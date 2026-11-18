#!/usr/bin/env python3
"""Pin how crew_decisions reads fictional member configs and applies their filters, marks and names.

Every case runs the real CLI over fixture transcripts in a temp dir, so no test reads a real one.
"""
import builtins
import errno
import hashlib
import io
import json
import os
import pty
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from contextlib import ExitStack, redirect_stderr, redirect_stdout
import unittest
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_fixtures import assert_cli_error, ask, reply, said, sandbox_usable  # noqa: E402

SCRIPTS = Path(__file__).resolve().parent.parent / "scripts"
CREW = SCRIPTS / "crew_decisions.py"


TEST_HOME_DIR = tempfile.TemporaryDirectory()
TEST_HOME = Path(TEST_HOME_DIR.name) / "home"
ENCODED_HOME = re.sub(r"[^A-Za-z0-9]", "-", str(TEST_HOME))
HOME_PATCH = patch.dict(os.environ, {"HOME": str(TEST_HOME), "CLAUDE_PROJECT_DIR": ""})
HOME_PATCH.start()


def tearDownModule():
    HOME_PATCH.stop()
    TEST_HOME_DIR.cleanup()


def marked(when, question, labels, answer):
    """A question whose labels already carry their gate marks."""
    return ask(when, question, labels, answer, recommended=None)


ADA = {"name": "Ada", "slug": "ada", "pronouns": "she/her", "principal": "Sam",
       "principal_slug": "sam", "principal_pronouns": "they/them", "state_dir": "STATE", "mark_word": "Ada",
       "filters": {"excluded_folders": r"garden-club|^-private-", "personal": r"\bmy therapist\b",
                   "secret": r"\b\d{9,}\b", "pasted_prefixes": ["You are "]}}

ARI = {"name": "Ari", "slug": "ari", "pronouns": "they/them", "principal": "Robin",
       "principal_slug": "robin", "state_dir": "STATE", "mark_word": "Ari",
       "filters": {"excluded_folders": r"hoa-board|^-private-", "personal": r"\bmy doctor\b",
                   "secret": r"\b\d{7,}\b|[\w.+-]+@[\w-]+\.[a-z]{2,}", "pasted_prefixes": ["Pasted: "]}}


class Harness:
    def __init__(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.projects = self.root / "projects"

    def session(self, folder, name, *records):
        path = self.projects / folder / f"{name}.jsonl"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("".join(json.dumps(r) + "\n" for r in records))

    def member(self, config, name="member.json"):
        config = json.loads(json.dumps(config).replace("STATE", str(self.root / "state")))
        (self.root / name).write_text(json.dumps(config))
        return str(self.root / name)

    def run(self, script, *args, env=None):
        return subprocess.run([sys.executable, str(script), *args], capture_output=True, text=True,
                              cwd=self.root, timeout=60, env={**os.environ, "HOME": str(TEST_HOME), **(env or {})})

    def crew(self, member, *args):
        return self.run(CREW, "--member", member, *args)

    def read(self, name):
        return (self.root / name).read_text()

    def write(self, name, value):
        (self.root / name).write_text(json.dumps(value))


def tripwire_crew(h, member, fences, *args):
    """Kill the child on any fence content or metadata access, including through links."""
    profile = '(version 1)(allow default)' + ''.join(
        '(deny file-read* file-write* file-read-metadata (subpath ' + json.dumps(str(p.resolve()), ensure_ascii=False) +
        ') (with send-signal SIGKILL))' for p in fences)
    return subprocess.run([shutil.which("sandbox-exec"), "-p", profile, sys.executable,
                           str(CREW), "--member", member, *args], capture_output=True, text=True,
                          cwd=h.root, env={**os.environ, "HOME": str(TEST_HOME)}, timeout=60)


def ada_fixture(h):
    h.session(ENCODED_HOME + "-projects-app", "s1",
              reply("2026-09-01T09:59:00Z", "Shall I keep two copies of the review queue?"),
              said("2026-09-01T10:00:00Z", "no, keep one source of truth for the review queue"),
              ask("2026-09-01T10:01:00Z", "Where should the review queue live?", ["Repo", "State"],
                  "Repo (Recommended · Ada)", member_pick=0),
              ask("2026-09-02T10:00:00Z", "Which review queue order?", ["Oldest", "Newest"], "Newest (Ada)",
                  member_pick=1),
              ask("2026-09-03T10:00:00Z", "Split the review queue?", ["Yes", "No"], "split it by repo instead"),
              said("2026-09-03T10:01:00Z", "You are a lane runner"),
              said("2026-09-03T10:02:00Z", "my therapist moved the review queue meeting"),
              said("2026-09-03T10:03:00Z", "the account number is 123456789"))
    h.session(ENCODED_HOME + "-projects-garden-club", "s2", said("2026-09-01T10:00:00Z", "grant call"))
    h.session("-private-scratch", "s3", said("2026-09-01T10:00:00Z", "budget call"))


class ExampleMember(unittest.TestCase):
    def test_mine_non_ascii_folder_has_zero_cwd_score(self):
        for cwd, folder in (("/projects/STRASSE", "-projects-Straße"),
                            ("/projects/K", "-projects-K")):
            with self.subTest(folder=folder):
                h = Harness()
                record = said("2026-09-01T00:00:00Z", "keep this review queue verdict", cwd=cwd)
                h.session("-aaa", "same", record)
                h.session(folder, "same", record)
                out = h.crew(h.member(ADA), "mine", "--projects", str(h.projects), "--out", "d.jsonl")
                self.assertEqual(out.returncode, 0, out.stderr)
                records = [json.loads(line) for line in h.read("d.jsonl").splitlines()]
                self.assertEqual([r["project"] for r in records], [".aaa"])
                self.assertIn(", 1 duplicate records", out.stdout)

    def test_mine_reads_a_list_answer(self):
        h = Harness()
        h.session(ENCODED_HOME + "-projects-app", "s1",
                  ask("2026-09-01T10:00:00Z", "List 0?", ["A", "A longer", "B"],
                      ["A longer", "B"], recommended=None, multi=True),
                  ask("2026-09-01T10:02:00Z", "List 2?", ["A", "B"],
                      ["A", "  own words, with a comma  "], recommended=None, multi=True),
                  ask("2026-09-01T10:04:00Z", "List 4?", ["A", "B"],
                      ["A (Recommended)"], recommended=0, multi=True),
                  ask("2026-09-01T10:07:00Z", "List 7?", ["A", "B"],
                      ["A", "   ", "A"], recommended=None, multi=True),
                  ask("2026-09-01T10:08:00Z", "List 8?", ["A", "B"],
                      ["A", "first own", "B", " second own "], recommended=None, multi=True))
        out = h.crew(h.member(ADA), "mine", "--projects", str(h.projects), "--out", "d.jsonl")
        self.assertEqual(out.returncode, 0, out.stderr)
        records = [json.loads(line) for line in h.read("d.jsonl").splitlines()]
        self.assertEqual([(r["picks"], r["own_words"], r["outcome"]) for r in records], [
            (["A longer", "B"], "", "no_rec"),
            (["A"], "own words, with a comma", "own_words"),
            (["A (Recommended)"], "", "took_rec"),
            (["A"], "", "no_rec"),
            (["A", "B"], "first own, second own", "own_words"),
        ])

    def test_mine_split_score_and_search(self):
        h = Harness()
        ada_fixture(h)
        member = h.member(ADA)
        out = h.crew(member, "mine", "--projects", str(h.projects), "--out", "d.jsonl", "--holdout", "1")
        self.assertEqual(out.returncode, 0, out.stderr)
        records = [json.loads(line) for line in h.read("d.jsonl").splitlines()]
        self.assertEqual(len(records), 4)
        self.assertEqual(sum(r["kind"] == "question" for r in records), 3)
        out = h.crew(member, "split", "--corpus", "d.jsonl", "--questions", "q.jsonl", "--answers", "a.jsonl")
        self.assertEqual(out.returncode, 0, out.stderr)
        questions = [json.loads(line) for line in h.read("q.jsonl").splitlines()]
        answers = [json.loads(line) for line in h.read("a.jsonl").splitlines()]
        self.assertEqual(len(answers), 3)
        self.assertEqual([q["id"] for q in questions], [a["id"] for a in answers])
        self.assertTrue(all("picks" not in q and "ada_pick" not in q for q in questions))
        found = h.crew(member, "similar", "--corpus", "d.jsonl", "--query",
                       "where should the review queue live", "--json")
        self.assertEqual(found.returncode, 0, found.stderr)
        self.assertEqual(json.loads(found.stdout), [
            {"kind": "message", "date": "2026-09-01", "project": "app",
             "text": "no, keep one source of truth for the review queue"}])
        h.write("labels.json", {a["id"]: {"category": "scope"} for a in answers})
        h.write("p.json", {a["id"]: {"pick": "Repo", "confidence": "high"} for a in answers})
        for fmt in ([], ["--json"]):
            scored = h.crew(member, "score", "--answers", "a.jsonl", "--labels", "labels.json",
                            "--predictions", "p.json", *fmt)
            self.assertEqual(scored.returncode, 0, scored.stderr)
            if not fmt:
                self.assertIn("Ada-led questions: 1 of 2 (50%); Sam took Ada's pick 100%", scored.stdout)
            else:
                self.assertEqual(json.loads(scored.stdout)["ada_led"],
                                 {"n": 2, "hits": 1, "accuracy": 0.5, "took_ada": 1.0})

    def test_record_uses_the_member_and_principal_keys(self):
        h = Harness()
        member = h.member(ADA)
        recorded = h.crew(member, "record", "--log", "live.jsonl", "--work", "plan.md", "--kind", "escalated",
                          "--category", "scope", "--tier", "low", "--question", "Q?", "--ada-pick", "A",
                          "--rule", "below-high", "--sam-answer", "B")
        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        line = json.loads(h.read("live.jsonl"))
        self.assertTrue(line.pop("timestamp"))
        self.assertEqual(list(line), ["work", "kind", "category", "tier", "question", "ada_pick", "rule",
                                     "sam_answer"])
        self.assertEqual(list(line.values()), ["plan.md", "escalated", "scope", "low", "Q?", "A", "below-high", "B"])


HOSTILE = "$(touch pwned-dollar) `touch pwned-tick` ${HOME} \"double\" 'single' back\\slash café\nEND_OF_TEXT\ntext"


class StdinTexts(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.member = self.h.member(ARI)
        self.values = {"work": HOSTILE, "kind": "escalated", "category": HOSTILE, "tier": "low",
                       "question": HOSTILE, "pick": HOSTILE}

    def stdin(self, command, payload, *flags):
        data = payload if isinstance(payload, bytes) else payload.encode("utf-8")
        return subprocess.run([sys.executable, str(CREW), "--member", self.member, command, *flags],
                              input=data, capture_output=True, cwd=self.h.root, timeout=60)

    def test_record_stdin_matches_flags_and_keeps_optional_nulls(self):
        for optional in ({}, {"rule": None, "answer": None}, {"rule": HOSTILE, "answer": HOSTILE}):
            with self.subTest(optional=optional):
                values = {**self.values, **optional}
                proc = self.stdin("record", json.dumps(values, ensure_ascii=False), "--log", "stdin.jsonl", "--stdin")
                self.assertEqual(proc.returncode, 0, proc.stderr)
                flags = ["--log", "flags.jsonl"]
                for key, value in values.items():
                    if value is not None:
                        flag = {"pick": "ari-pick", "answer": "robin-answer"}.get(key, key)
                        flags.extend(["--" + flag, value])
                flagged = self.h.crew(self.member, "record", *flags)
                self.assertEqual(flagged.returncode, 0, flagged.stderr)
                self.assertEqual(proc.stdout.decode(), flagged.stdout)
                actual = json.loads(self.h.read("stdin.jsonl").splitlines()[-1])
                expected = json.loads(self.h.read("flags.jsonl").splitlines()[-1])
                actual.pop("timestamp")
                expected.pop("timestamp")
                self.assertEqual(actual, expected)
                self.assertEqual(list(actual), ["work", "kind", "category", "tier", "question", "ari_pick",
                                                "rule", "robin_answer"])
                self.assertEqual(actual["question"], HOSTILE)

    def test_similar_stdin_matches_query_with_both_output_forms(self):
        ada_fixture(self.h)
        member = self.h.member(ADA)
        mined = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "d.jsonl")
        self.assertEqual(mined.returncode, 0, mined.stderr)
        self.member = member
        query = "review queue " + HOSTILE
        for fmt in ([], ["--json"]):
            proc = self.stdin("similar", " \n" + json.dumps({"query": query}) + "\n ",
                              "--corpus", "d.jsonl", "--stdin", *fmt)
            self.assertEqual(proc.returncode, 0, proc.stderr)
            flagged = self.h.crew(member, "similar", "--corpus", "d.jsonl", "--query", query, *fmt)
            self.assertEqual(flagged.returncode, 0, flagged.stderr)
            self.assertEqual(proc.stdout.decode(), flagged.stdout)
            self.assertIn("review queue", flagged.stdout)

    def test_stdin_refusals_are_usage_errors_before_reading_or_writing(self):
        for command, good in (("record", self.values), ("similar", {"query": HOSTILE})):
            key = "pick" if command == "record" else "query"
            cases = [(b"not json", "JSON object"), (b"[]", "JSON object"),
                     (json.dumps({**good, "unknown": "x"}).encode(), "unknown"),
                     ((json.dumps(good)[:-1] + ', "' + key + '": "again"}').encode(), key),
                     (json.dumps({k: v for k, v in good.items() if k != key}).encode(), key),
                     (json.dumps({**good, key: 7}).encode(), key),
                     (b" " * 65537, "64 KiB"), (b"\xff", "UTF-8"),
                     (json.dumps({**good, key: "\ud800"}).encode(), "JSON object")]
            if command == "record":
                cases += [(json.dumps({**good, "rule": 7}).encode(), "rule"),
                          (json.dumps({**good, "answer": 7}).encode(), "answer"),
                          (json.dumps({**good, "kind": "maybe"}).encode(), "kind"),
                          (json.dumps({**good, "tier": "top"}).encode(), "tier")]
            for payload, problem in cases:
                with self.subTest(command=command, problem=problem, payload=payload[:80]):
                    flags = ["--log", "live.jsonl"] if command == "record" else ["--corpus", "absent.jsonl"]
                    proc = self.stdin(command, payload, *flags, "--stdin")
                    self.assertEqual(proc.returncode, 2, proc.stderr)
                    self.assertIn(problem, proc.stderr.decode())
                    self.assertIn(command + ": error:", proc.stderr.decode())
                    self.assertEqual(proc.stdout, b"")
                    self.assertFalse((self.h.root / "live.jsonl").exists())

    def test_stdin_and_flags_are_either_or_and_flags_still_name_every_missing_argument(self):
        for flag in ("--work", "--kind", "--category", "--tier", "--question", "--ari-pick", "--rule", "--robin-answer"):
            value = {"--kind": "decided", "--tier": "high"}.get(flag, "text")
            proc = self.stdin("record", json.dumps(self.values), "--stdin", flag, value)
            self.assertEqual(proc.returncode, 2, proc.stderr)
            self.assertIn(b"give the texts either on stdin (--stdin) or as flags, not both", proc.stderr)
        proc = self.stdin("record", b"")
        self.assertEqual(proc.returncode, 2, proc.stderr)
        self.assertIn(b"the following arguments are required: --work, --kind, --category, --tier, --question, --ari-pick",
                      proc.stderr)
        proc = self.stdin("similar", '{"query": "text"}', "--stdin", "--query", "text")
        self.assertEqual(proc.returncode, 2, proc.stderr)
        proc = self.stdin("similar", b"")
        self.assertEqual(proc.returncode, 2, proc.stderr)
        self.assertIn(b"--query", proc.stderr)


class AnotherMember(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.member = self.h.member(ARI)

    def mine(self):
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return proc, [json.loads(line) for line in self.h.read("d.jsonl").splitlines()]

    def test_their_mark_word_records_their_pick_and_another_members_mark_is_not_theirs(self):
        self.h.session(ENCODED_HOME + "-projects-grants", "s1",
                       marked("2026-09-01T10:00:00Z", "Lead with aim 1?", ["Yes (Recommended · Ari)", "No"],
                              "Yes (Recommended · Ari)"),
                       marked("2026-09-02T10:00:00Z", "Cut the budget table?", ["Cut", "Keep (Ada)"], "Keep (Ada)"))
        _, records = self.mine()
        a, b = records
        self.assertEqual(a["ari_pick"], "Yes (Recommended · Ari)")
        self.assertEqual(a["outcome"], "took_rec")
        self.assertNotIn("ada_pick", a)
        self.assertIsNone(b["ari_pick"])

    def test_their_filters_replace_the_other_members(self):
        self.h.session(ENCODED_HOME + "-projects-lab-dashboard", "s1",
                       said("2026-09-01T10:00:00Z", "the grant call is due friday"),
                       said("2026-09-01T10:01:00Z", "my doctor moved it"),
                       said("2026-09-01T10:02:00Z", "student 12345678 asked for an extension"),
                       said("2026-09-01T10:03:00Z", "write to someone@example.edu"),
                       said("2026-09-01T10:04:00Z", "Pasted: a syllabus"),
                       said("2026-09-01T10:05:00Z", "the IRB amendment goes first"),
                       said("2026-09-01T10:06:00Z", "You are kept for this member"))
        self.h.session(ENCODED_HOME + "-projects-hoa-board", "s2", said("2026-09-01T10:00:00Z", "dues"))
        proc, records = self.mine()
        self.assertEqual([r["text"] for r in records],
                         ["the grant call is due friday", "the IRB amendment goes first",
                          "You are kept for this member"])
        self.assertIn("dropped 1 excluded folders, 1 personal records, 2 secret records", proc.stdout)

    def test_score_and_record_use_their_names(self):
        self.h.session(ENCODED_HOME + "-projects-grants", "s1",
                       marked("2026-09-01T10:00:00Z", "Lead with aim 1?", ["Yes (Recommended · Ari)", "No"],
                              "Yes (Recommended · Ari)"))
        self.mine()
        h = self.h
        self.assertEqual(h.crew(self.member, "split", "--corpus", "d.jsonl", "--questions", "q.jsonl",
                                "--answers", "a.jsonl").returncode, 0)
        [answer] = [json.loads(line) for line in h.read("a.jsonl").splitlines()]
        self.assertIn("ari_pick", answer)
        h.write("labels.json", {answer["id"]: {"category": "funder-strategy"}})
        h.write("p.json", {answer["id"]: {"pick": "Yes", "confidence": "high"}})
        scored = h.crew(self.member, "score", "--answers", "a.jsonl", "--labels", "labels.json", "--predictions",
                        "p.json", "--json")
        self.assertEqual(scored.returncode, 0, scored.stderr)
        result = json.loads(scored.stdout)
        self.assertEqual(result["ari_led"], {"n": 1, "hits": 1, "accuracy": 1.0, "took_ari": 1.0})
        text = h.crew(self.member, "score", "--answers", "a.jsonl", "--labels", "labels.json", "--predictions",
                      "p.json").stdout
        self.assertIn("Ari-led questions: 1 of 1 (100%); Robin took Ari's pick 100%", text)
        recorded = h.crew(self.member, "record", "--work", "w", "--kind", "decided", "--category", "scope",
                          "--tier", "high", "--question", "Q?", "--ari-pick", "A", "--robin-answer", "B")
        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        line = json.loads((h.root / "state" / "live.jsonl").read_text())
        self.assertEqual((line["ari_pick"], line["robin_answer"]), ("A", "B"))
        self.assertNotIn("ada_pick", line)

    def test_similar_defaults_to_the_corpus_in_their_state_dir(self):
        self.h.session(ENCODED_HOME + "-projects-grants", "s1",
                       said("2026-09-01T10:00:00Z", "always lead the aims page with the community partner need"))
        state = self.h.root / "state"
        state.mkdir()
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out",
                           str(state / "decisions.jsonl"), "--holdout", "0")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        found = self.h.crew(self.member, "similar", "--query", "aims page community partner", "--json")
        self.assertEqual(found.returncode, 0, found.stderr)
        self.assertEqual(len(json.loads(found.stdout)), 1)

    def test_the_env_var_names_the_member(self):
        self.h.session(ENCODED_HOME + "-projects-grants", "s1", said("2026-09-01T10:00:00Z", "kept"))
        proc = self.h.run(CREW, "mine", "--projects", str(self.h.projects), "--out", "d.jsonl",
                          env={"CREW_MEMBER": self.member})
        self.assertEqual(proc.returncode, 0, proc.stderr)


class PrivateOverlay(unittest.TestCase):
    """A gitignored private.json beside member.json holds what must not be committed: its filters
    extend the member's (both apply), and any other key it names replaces the member's."""

    def setUp(self):
        self.h = Harness()
        self.member = self.h.member(ARI)

    def overlay(self, value):
        (self.h.root / "private.json").write_text(value if isinstance(value, str) else json.dumps(value))

    def test_its_filters_extend_the_members(self):
        self.overlay({"filters": {"excluded_folders": "garden-club", "personal": r"\bmy cat\b",
                                  "secret": r"\bpin \d{4}\b", "pasted_prefixes": ["Quoted: "]}})
        self.h.session(ENCODED_HOME + "-projects-grants", "s1",
                       said("2026-09-01T10:00:00Z", "the grant call is due friday"),
                       said("2026-09-01T10:01:00Z", "my doctor moved it"),
                       said("2026-09-01T10:02:00Z", "my cat is at the vet"),
                       said("2026-09-01T10:03:00Z", "the door pin 1234 changed"),
                       said("2026-09-01T10:04:00Z", "Pasted: a syllabus"),
                       said("2026-09-01T10:05:00Z", "Quoted: an email"))
        self.h.session(ENCODED_HOME + "-projects-hoa-board", "s2", said("2026-09-01T10:00:00Z", "dues"))
        self.h.session(ENCODED_HOME + "-garden-club", "s3", said("2026-09-01T10:00:00Z", "the plot rota"))
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        texts = [json.loads(line)["text"] for line in self.h.read("d.jsonl").splitlines()]
        self.assertEqual(texts, ["the grant call is due friday"])
        self.assertIn("dropped 2 excluded folders, 2 personal records, 1 secret records", proc.stdout)

    def test_any_other_key_replaces_the_members(self):
        self.overlay({"principal_slug": "sam"})
        recorded = self.h.crew(self.member, "record", "--work", "w", "--kind", "decided", "--category", "scope",
                               "--tier", "high", "--question", "Q?", "--ari-pick", "A", "--sam-answer", "B")
        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        line = json.loads((self.h.root / "state" / "live.jsonl").read_text())
        self.assertEqual(line["sam_answer"], "B")

    def test_a_broken_overlay_is_named(self):
        self.overlay("{not json")
        proc = self.h.crew(self.member, "mine", "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("private.json is not JSON", proc.stderr)

    def test_an_overlay_of_the_wrong_shape_is_named(self):
        for value, named in (([1, 2], "private.json must be a JSON object"),
                             ({"filters": {"personal": ["a", "b"]}}, "private.json: filters.personal must be a string")):
            self.overlay(value)
            proc = self.h.crew(self.member, "mine", "--out", "d.jsonl")
            self.assertEqual(proc.returncode, 1, value)
            self.assertIn(named, proc.stderr)


FILLED = {"NAME": "Ari", "SLUG": "ari", "PRONOUNS": "they/them", "PRINCIPAL": "Robin", "PRINCIPAL_SLUG": "robin",
          "PRINCIPAL_PRONOUNS": "they/them", "REPO": "~/projects/voice", "DATE": "2026-10-22"}


def asking(when, tool_id, question, entrypoint):
    """An AskUserQuestion call still waiting for its answer, in a session opened from `entrypoint`."""
    return {"type": "assistant", "timestamp": when, "cwd": "/repo", "entrypoint": entrypoint,
            "message": {"role": "assistant", "content": [
                {"type": "tool_use", "id": tool_id, "name": "AskUserQuestion",
                 "input": {"questions": [{"question": question, "options": [{"label": "a"}, {"label": "b"}]}]}}]}}


class Pending(unittest.TestCase):
    def test_only_headless_sessions_are_reported(self):
        h = Harness()
        member = h.member(ADA)
        for name, entrypoint in [("desktop", "claude-desktop"), ("terminal", "cli"), ("job-run", "sdk-cli"),
                                 ("unknown", None)]:
            h.session(ENCODED_HOME + "-projects-app", name, asking("2026-10-25T10:00:00Z", "t1", "Which?", entrypoint))
        proc = h.crew(member, "pending", "--projects", str(h.projects), "--seen", str(h.root / "seen.json"))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual([json.loads(line)["session"] for line in proc.stdout.splitlines()], ["job-run"])

    def test_pending_without_compartments_preserves_output_bytes(self):
        h = Harness()
        self.addCleanup(h.dir.cleanup)
        h.session(ENCODED_HOME + "-projects-app", "s",
                  asking("2026-09-01T00:00:00Z", "t", "Which?", "sdk-cli"))
        folder = h.root / "queues" / "app"
        folder.mkdir(parents=True)
        (folder / "queue.md").write_text("🎛️ CONTROL-Q APP-1 · t · lane ALL\n")
        proc = h.crew(h.member(ADA), "pending", "--projects", str(h.projects),
                      "--queues", str(folder.parent), "--seen", "seen.json")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout.encode(), (
            '{"kind": "ask", "tool_id": "t", "when": "2026-09-01T00:00:00Z", '
            '"question": "Which?", "options": ["a", "b"], "session": "s", "project": "app", "id": "s:t"}\n'
            '{"kind": "control-q", "project": "app", "key": "APP-1", "line": 1, '
            '"question": "🎛️ CONTROL-Q APP-1 · t · lane ALL", "options": [], "body": "", "id": "app:APP-1:1"}\n'
        ).encode())
        self.assertEqual(json.loads(h.read("seen.json")), ["app:APP-1:1", "s:t"])


class PrivacyCheck(unittest.TestCase):
    """The step 11 gate over a crew folder made from the template the way install.sh makes it."""

    def setUp(self):
        self.h = Harness()
        self.dir = self.h.root / "crew"
        shutil.copytree(SCRIPTS.parent / "crew" / "_template", self.dir)
        (self.dir / "README.md").unlink()
        for path in self.dir.rglob("*"):
            text = path.read_text() if path.is_file() else None
            for key, value in FILLED.items():
                text = text and text.replace("{{" + key + "}}", value)
            if text is not None:
                path.write_text(text)

    def check(self):
        return self.h.crew(str(self.dir / "member.json"), "privacy-check", "--dir", str(self.dir))

    def append(self, name, line):
        path = self.dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text((path.read_text() if path.exists() else "") + line + "\n")
        return len(path.read_text().splitlines())

    def edit_member(self, change):
        config = json.loads((self.dir / "member.json").read_text())
        change(config)
        (self.dir / "member.json").write_text(json.dumps(config, indent=2))

    def test_the_template_is_clean_although_its_filters_match_themselves(self):
        proc = self.check()
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(proc.stdout, "clean\n")

    def test_markdown_hits_name_the_file_and_line_but_never_the_text(self):
        line = self.append("decisions.md", "We moved it because my doctor called.")
        self.append("briefs/extra.md", "Write to robin@example.org first.")
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn(f"decisions.md:{line} (personal)", proc.stdout)
        self.assertIn("briefs/extra.md:1 (secret)", proc.stdout)
        self.assertTrue(proc.stdout.endswith("2 hits\n"), proc.stdout)
        for text in ("doctor", "robin@"):
            self.assertNotIn(text, proc.stdout + proc.stderr)

    def test_member_json_is_scanned_outside_its_filters(self):
        self.edit_member(lambda c: c["codebook"]["domains"].append("robin@example.org"))
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member.json:codebook.domains[4] (secret)", proc.stdout)
        self.assertNotIn("robin@", proc.stdout)

    def test_filters_that_differ_from_the_template_are_hits(self):
        self.edit_member(lambda c: c["filters"].update(personal=c["filters"]["personal"] + r"|\bgarden club\b"))
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member.json:filters.personal (differs from crew/_template)", proc.stdout)
        self.assertIn("private.json", proc.stdout)
        self.assertNotIn("garden", proc.stdout)

    def test_private_json_patterns_apply_but_private_json_is_not_scanned(self):
        (self.dir / "private.json").write_text(json.dumps({"filters": {"personal": r"\bgarden club\b"}}))
        line = self.append("autonomy.md", "the garden club rota")
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn(f"autonomy.md:{line} (personal)", proc.stdout)
        self.assertNotIn("private.json:", proc.stdout)
        self.assertTrue(proc.stdout.endswith("1 hits\n"), proc.stdout)

    def test_every_committed_file_is_scanned_at_any_depth(self):
        (self.dir / "engineers").mkdir()
        self.append("engineers/example.md", "my doctor again")
        self.append("briefs/sub/deep.md", "my doctor again")
        self.append("report-template.html", "<p>my doctor again</p>")
        self.append("notes.txt", "my doctor again")
        self.append("extra.json", '{"note": "my doctor again"}')
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        for name in ("engineers/example.md:1", "briefs/sub/deep.md:1", "report-template.html:1", "notes.txt:1",
                     "extra.json:1"):
            self.assertIn(f"{name} (personal)", proc.stdout)
        self.assertTrue(proc.stdout.endswith("5 hits\n"), proc.stdout)

    def test_a_file_that_is_not_text_is_a_hit_by_name(self):
        (self.dir / "clip.bin").write_bytes(b"caf\xe9")
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn("clip.bin (not text)", proc.stdout)

    def test_a_key_that_trips_a_filter_is_named_by_its_parent_only(self):
        self.edit_member(lambda c: c["codebook"]["groups"].update({"robin@example.org": ["scope"]}))
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member.json:codebook.groups (a key, secret)", proc.stdout)
        self.assertNotIn("robin@", proc.stdout)

    def test_a_member_json_that_is_not_an_object_is_named(self):
        member = self.h.member(ARI)
        (self.dir / "member.json").write_text("[1, 2]")
        proc = self.h.crew(member, "privacy-check", "--dir", str(self.dir))
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member.json must be a JSON object", proc.stderr)
        assert_cli_error(self, proc.stderr, "crew_decisions:")

    def test_a_folder_without_member_json_is_named(self):
        member = self.h.member(ARI)
        empty = self.h.root / "empty"
        empty.mkdir()
        proc = self.h.crew(member, "privacy-check", "--dir", str(empty))
        self.assertEqual(proc.returncode, 1)
        self.assertIn(f"{empty}/member.json", proc.stderr)
        assert_cli_error(self, proc.stderr, "crew_decisions:")


class BadConfig(unittest.TestCase):
    def test_no_member_is_an_error(self):
        h = Harness()
        env = {k: v for k, v in os.environ.items() if k != "CREW_MEMBER"}
        proc = subprocess.run([sys.executable, str(CREW), "mine", "--out", "d.jsonl"], capture_output=True,
                              text=True, cwd=h.root, env=env, timeout=60)
        self.assertEqual(proc.returncode, 1)
        self.assertIn("crew_decisions: no member config (--member or CREW_MEMBER)", proc.stderr)

    def test_a_missing_key_is_named(self):
        h = Harness()
        member = h.member({k: v for k, v in ARI.items() if k != "slug"})
        proc = h.crew(member, "mine", "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member config: missing slug", proc.stderr)

    def test_a_bad_regex_is_named(self):
        h = Harness()
        member = h.member({**ARI, "filters": {**ARI["filters"], "personal": "(unclosed"}})
        proc = h.crew(member, "mine", "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("filters.personal", proc.stderr)


class Compartments(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.config = {"version": 1, "fenced": ["~/garden-club"],
                       "compartments": {"work": {"folders": ["~/projects/work", "~/.hub", "~/.other-hub"]},
                                        "client": {"folders": ["~/projects/work-site", "~/projects/client"]}},
                       "hub": {"paths": ["~/.hub/projects", "~/.other-hub/projects"], "routes": {"client-stack": "client"}}}
        self.h.write("compartments.json", self.config)
        self.member = self.h.member({**ADA, "compartments": "compartments.json"})
        sys.path.insert(0, str(SCRIPTS))
        import crew_decisions
        self.c = crew_decisions

    def garden_filters(self, overrides):
        self.config["compartments"]["garden"] = {
            "folders": ["~/projects/garden"], "filters": overrides}
        self.config["fenced"] = ["~/projects/garden/blocked"]
        self.h.write("compartments.json", self.config)
        return self.h.member({**ADA, "compartments": "compartments.json",
                              "filters": {**ADA["filters"], "excluded_folders": "garden",
                                          "personal": "garden"}})

    def test_mine_compartment_filters_replace_only_their_own_member_filter(self):
        for overrides, garden_kept in (({}, False), ({"excluded_folders": "(?!)"}, False),
                                       ({"personal": "(?!)"}, False),
                                       ({"excluded_folders": "(?!)", "personal": "(?!)"}, True)):
            with self.subTest(overrides=overrides):
                member = self.garden_filters(overrides)
                self.h.session(self.c.encode("~/projects/garden"), "garden",
                               said("2026-09-01T00:00:00Z", "keep the garden review queue",
                                    cwd="~/projects/garden"),
                               said("2026-09-01T00:01:00Z", "garden account 123456789"))
                cross = [said("2026-09-02T00:00:00Z", "keep this review queue"),
                         said("2026-09-02T00:01:00Z", "keep the garden review queue")]
                for record in cross:
                    record["cwd"] = str(TEST_HOME / "projects/garden")
                self.h.session(self.c.encode("~/projects/work"), "work", *cross,
                               said("2026-09-02T00:02:00Z", "garden stays private"))
                self.h.session(self.c.encode("~/projects/garden/blocked"), "fenced",
                               said("2026-09-03T00:00:00Z", "never admit fenced folder"))
                blocked = said("2026-09-04T00:00:00Z", "never admit fenced cwd")
                blocked["cwd"] = str(TEST_HOME / "projects/garden/blocked")
                self.h.session(self.c.encode("~/projects/garden"), "blocked-cwd", blocked)
                proc = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "out.jsonl")
                self.assertEqual(proc.returncode, 0, proc.stderr)
                rows = [json.loads(line) for line in self.h.read("out.jsonl").splitlines()]
                self.assertEqual([(r["compartment"], r["text"]) for r in rows],
                                 [("garden", "keep the garden review queue")] if garden_kept else [])
                excluded = 3 if "excluded_folders" in overrides else 2
                self.assertIn(f", {excluded} excluded records", proc.stdout)
                self.assertFalse(any("garden" in r["text"] for r in rows if r["compartment"] == "work"))
                # Merge retains the compartment filters for tagged records.
                proc = self.h.crew(member, "merge", "--in", "out.jsonl", "--out", "merged.jsonl")
                self.assertEqual(proc.returncode, 0, proc.stderr)
                merged = [json.loads(line) for line in self.h.read("merged.jsonl").splitlines()]
                self.assertEqual(merged, rows)

    def test_mine_hub_routing_does_not_borrow_compartment_filters(self):
        member = self.garden_filters({"excluded_folders": "(?!)", "personal": "(?!)"})
        config = json.loads(Path(member).read_text())
        config["filters"]["personal"] = r"garden (review|stays)"
        self.h.write("member.json", config)
        self.config["compartments"]["client"]["filters"] = {"personal": "(?!)"}
        self.config["hub"]["paths"].append("~/projects/garden/projects")
        self.config["hub"]["routes"].update({"garden-stack": "garden", "work-stack": "work"})
        self.h.write("compartments.json", self.config)
        for folder, targets, expected, excluded in (
                ("~/.hub", ["garden-stack"], "garden", 1),
                ("~/.hub", ["garden-stack", "client-stack"], "hub-mixed", 1),
                ("~/projects/garden", ["client-stack"], "client", 0),
                ("~/projects/garden", ["client-stack", "work-stack"], "hub-mixed", 0)):
            with self.subTest(folder=folder, compartment=expected):
                shutil.rmtree(self.h.projects, ignore_errors=True)
                tool = {"type": "assistant", "message": {"content": [
                    {"type": "tool_use", "name": "Read", "input": {
                        "file_path": folder + "/projects/" + target + "/queue.md"}}
                    for target in targets]}}
                self.h.session(self.c.encode(folder), "routed", tool,
                               said("2026-09-01T00:00:00Z", "keep the garden review queue",
                                    cwd="~/projects/garden"),
                               said("2026-09-01T00:01:00Z", "keep this review queue",
                                    cwd="~/projects/garden" if excluded else "~/projects/work"),
                               said("2026-09-01T00:02:00Z", "garden stays private",
                                    cwd="~/projects/work"),
                               said("2026-09-01T00:03:00Z", "keep every review queue verdict",
                                    cwd="~/projects/work"))
                proc = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "out.jsonl")
                self.assertEqual(proc.returncode, 0, proc.stderr)
                rows = [json.loads(line) for line in self.h.read("out.jsonl").splitlines()]
                self.assertEqual([(r["compartment"], r["text"]) for r in rows],
                                 [(expected, "keep every review queue verdict")] if excluded else [])
                if excluded:
                    self.assertIn(", 2 excluded records", proc.stdout)
                else:
                    self.assertIn(", 5 excluded records", proc.stdout)

    def test_admitted_hub_parent_cannot_route_into_another_compartment(self):
        member = self.garden_filters({"excluded_folders": "(?!)", "personal": "(?!)"})
        config = json.loads(Path(member).read_text())
        config["filters"]["personal"] = "garden review"
        self.h.write("member.json", config)
        self.config["hub"]["paths"].append("~/projects/garden/projects")
        self.h.write("compartments.json", self.config)
        self.h.session(self.c.encode("~/projects/garden"), "cross-route",
                       self.write_tool("~/projects/garden/projects/client-stack/queue.md"),
                       said("2026-09-01T00:00:00Z", "keep this review queue",
                            cwd="~/projects/garden"))
        proc = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "out.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(self.h.read("out.jsonl"), "")
        self.assertIn(", 2 excluded records", proc.stdout)

    def test_admitted_folder_rejects_other_and_unmapped_cwds(self):
        member = self.garden_filters({"excluded_folders": "(?!)", "personal": "(?!)"})
        for i, cwd in enumerate(("~/projects/work", "~/outside", "~/projects/garden", None)):
            record = said(f"2026-09-01T00:0{i}:00Z", "keep this review queue")
            if cwd is not None:
                record["cwd"] = cwd
            else:
                record.pop("cwd", None)
            self.h.session(self.c.encode("~/projects/garden"), f"cwd-{i}", record)
        proc = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "out.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        rows = [json.loads(line) for line in self.h.read("out.jsonl").splitlines()]
        self.assertEqual([(r["timestamp"], r["compartment"]) for r in rows],
                         [("2026-09-01T00:02:00Z", "garden"), ("2026-09-01T00:03:00Z", "garden")])
        self.assertIn(", 2 excluded records", proc.stdout)

    def test_filter_override_grid_only_adds_its_own_records(self):
        member = self.garden_filters({"excluded_folders": "(?!)", "personal": "(?!)"})
        folders = {"work": "~/projects/work", "garden": "~/projects/garden", "hub": "~/.hub"}
        self.config["hub"]["paths"] = [folder + "/projects" for folder in folders.values()]
        self.config["hub"]["routes"] = {"work-stack": "work", "garden-stack": "garden"}
        cases = {}
        for owner, folder in folders.items():
            for cwd_name, cwd in (("work", "~/projects/work"), ("garden", "~/projects/garden"),
                                  ("none", None), ("fenced", "~/projects/garden/blocked"),
                                  ("unmapped", "~/outside")):
                for route, targets in (("off", []), ("toA", ["work-stack"]),
                                       ("toB", ["garden-stack"]),
                                       ("mixed", ["work-stack", "garden-stack"])):
                    for mentions in (False, True):
                        name = f"2026-09-01T{len(cases) // 60:02d}:{len(cases) % 60:02d}:00Z"
                        text = "keep the garden review queue" if mentions else "keep this review queue"
                        record = said(name, text)
                        if cwd is None:
                            record.pop("cwd", None)
                        else:
                            record["cwd"] = cwd
                        tools = [self.write_tool(folder + "/projects/" + target + "/queue.md")
                                 for target in targets]
                        self.h.session(self.c.encode(folder), name, *tools, record)
                        routed = {"off": "work" if owner == "hub" else owner,
                                  "toA": "work", "toB": "garden", "mixed": "hub-mixed"}[route]
                        cases[name] = (owner, cwd_name, routed, mentions)
        # Exercise both newly admitted folders and personal-only overrides in member-kept folders.
        for excluded in ("garden", "(?!)"):
            with self.subTest(excluded=excluded):
                config = json.loads(Path(member).read_text())
                config["filters"]["excluded_folders"] = excluded
                config["filters"]["personal"] = "garden review"
                self.h.write("member.json", config)
                baseline = json.loads(json.dumps(self.config))
                del baseline["compartments"]["garden"]["filters"]
                outputs = []
                for c in (baseline, self.config):
                    self.h.write("compartments.json", c)
                    proc = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "out.jsonl")
                    self.assertEqual(proc.returncode, 0, proc.stderr)
                    outputs.append({r["timestamp"]: r for r in
                                    map(json.loads, self.h.read("out.jsonl").splitlines())})
                before, after = outputs
                eligible = {name for name, (owner, cwd, routed, mentions) in cases.items()
                            if owner == "garden" and routed == "garden" and cwd in ("garden", "none")}
                expected_new = {name for name in eligible if excluded == "garden" or cases[name][3]}
                self.assertEqual(set(after) - set(before), expected_new)
                for name in expected_new:
                    self.assertEqual(after[name]["compartment"], "garden", name)
                self.assertEqual({name: row for name, row in after.items() if name in before}, before)

    def test_compartment_filter_validation(self):
        for filters, suffix in ((None, ""), ([], ""), ({"secret": "x"}, ".secret"),
                                ({"personal": 1}, ".personal"), ({"personal": "("}, ".personal"),
                                ({"excluded_folders": False}, ".excluded_folders"),
                                ({"excluded_folders": "["}, ".excluded_folders"),
                                ({"personal": "x{99999999999999999999}"}, ".personal")):
            with self.subTest(filters=filters):
                self.garden_filters(filters)
                proc = self.h.crew(self.member, "where")
                self.assertEqual(proc.returncode, 2, proc.stderr)
                self.assertIn("compartments.garden.filters" + suffix, proc.stderr)

    def test_config_and_resolvers(self):
        m = self.c.load_member(self.c.read_member(self.member))
        c = m.compartments
        home = str(Path.home())
        for path, expected in (("~/projects/work", "work"), ("~/projects/work-site", "client"),
                               ("~/other", "p:other")):
            name = self.c.encode(path)
            self.assertEqual(self.c.folder_compartment(c, name), expected)
            self.assertEqual(self.c.folder_compartment(c, name + "--claude-worktrees-x"), expected)
        fenced = self.c.encode("~/garden-club")
        self.assertEqual(self.c.folder_compartment(c, "-private-tmp-" + fenced + "-id-scratchpad"), "fenced")
        for path, cwd, expected in (("notes/x.md", home + "/projects/work", "work"),
                                    ("~/.hub/projects/client-stack/x.md", home, "client"),
                                    ("~/.hub/projects/unknown/x.md", home, "work"),
                                    ("~/.other-hub/projects/client-stack/x.md", home, "client"),
                                    ("~/garden-club/sub/x.md", home, "fenced")):
            self.assertEqual(self.c.path_compartment(c, path, cwd), expected)
        repo = self.h.root / "repo"
        (repo / ".git").mkdir(parents=True)
        (repo / "sub").mkdir()
        expected = "p:" + self.c.project_name(self.c.encode(repo.resolve()))
        self.assertEqual(self.c.path_compartment(c, repo / "sub/file.md", home), expected)
        wt = repo / ".claude/worktrees/branch"
        (wt / ".git").mkdir(parents=True)
        self.assertEqual(self.c.path_compartment(c, wt / "file.md", home), expected)
        link = self.h.root / "link"
        link.symlink_to(Path("~/garden-club").expanduser())
        self.assertEqual(self.c.path_compartment(c, link / "x", home), "fenced")
        for project, expected in (("work", "work"), ("work-site", "client"),
                                  ("garden-club", "fenced"), ("unknown", "p:unknown")):
            self.assertEqual(self.c.record_compartment(c, {"project": project}), expected)
        self.assertEqual(self.c.record_compartment(c, {"compartment": "hub-mixed"}), "hub-mixed")

    def test_fallback_names_with_symlink_home_preserve_fences(self):
        home = self.h.root.resolve() / "home"
        home.mkdir()
        linked_home = self.h.root / "linked-home"
        linked_home.symlink_to(home, target_is_directory=True)
        project = home / "projects/queues-app"
        project.mkdir(parents=True)
        repo = home / "projects/repo"
        (repo / ".git").mkdir(parents=True)
        worktree = repo / ".claude/worktrees/branch"
        (worktree / ".git").mkdir(parents=True)
        fenced = home / "garden-club"
        fenced.mkdir()
        (project / "blocked").symlink_to(fenced, target_is_directory=True)
        cases = (("projects/queues-app", True, "p:queues-app"),
                 ("projects/queues-app/queue.md", False, "p:queues-app"),
                 ("projects/repo/queue.md", True, "p:repo"),
                 ("projects/repo/.claude/worktrees/branch/queue.md", True, "p:repo"))
        for spelling in (home, linked_home):
            with self.subTest(home=spelling), patch.dict(os.environ, {"HOME": str(spelling)}):
                c = self.c.check_compartments({"version": 1, "fenced": ["~/garden-club"],
                                               "compartments": {}})
                for relative, resolve, expected in cases:
                    with self.subTest(relative=relative, resolve=resolve):
                        self.assertEqual(self.c.path_compartment(c, spelling / relative, spelling,
                                                                resolve=resolve), expected)
                outside = self.h.root.resolve()
                expected = "p:" + self.c.project_name(self.c.encode(outside))
                self.assertEqual(self.c.path_compartment(c, outside, spelling), expected)
                lstat = os.lstat
                def guarded(path, *args, **kwargs):
                    self.assertFalse(self.c.under(Path(path), fenced), "probed fence")
                    return lstat(path, *args, **kwargs)
                with patch.object(os, "lstat", side_effect=guarded):
                    self.assertEqual(self.c.path_compartment(c, project / "blocked/queue.md", spelling),
                                     "fenced")

    def test_hub_paths_validation(self):
        for hub, error in (({"path": "~/.hub/projects", "routes": {}}, "hub.path"),
                           ({"paths": [], "routes": {}}, "hub.paths"),
                           ({"paths": ["relative"], "routes": {}}, "hub.paths"),
                           ({"paths": "~/.hub/projects", "routes": {}}, "hub.paths")):
            with self.subTest(hub=hub):
                with self.assertRaisesRegex(self.c.Failed, error):
                    self.c.check_compartments({**self.config, "hub": hub})

    def test_hub_projects_require_a_route_or_real_directory(self):
        hub = TEST_HOME / ".hub/projects"
        garden = hub / "garden"
        garden.mkdir(parents=True)
        self.addCleanup(shutil.rmtree, hub.parent)
        for name in ("*", "client-*", "{a,b}", "$p", ".git", ".flightdeck", "missing"):
            # Even an on-disk directory or route cannot admit invalid syntax.
            if name != "missing":
                (hub / name).mkdir()
                self.config["hub"]["routes"][name] = "client"
            with self.subTest(name=name):
                target = "~/.hub/projects/" + name + "/queue.md"
                self.assertEqual(self.c.hub_compartment(self.config, ENCODED_HOME + "--hub", "work", [self.write_tool(target)]), "work")
                self.assertEqual(self.c.path_compartment(self.config, target, TEST_HOME), "work")
                self.assertEqual(self.c.folder_compartment(self.config, self.c.encode("~/.hub/projects") + "-" + name), "work")
        for route, expected in ((None, "p:garden"), ("p:garden-site", "p:garden-site")):
            if route:
                self.config["hub"]["routes"]["GARDEN"] = route
            with self.subTest(route=route):
                target = "~/.hub/projects/garden/queue.md"
                self.assertEqual(self.c.hub_compartment(self.config, ENCODED_HOME + "--hub", "work", [self.write_tool(target)]), expected)
                self.assertEqual(self.c.path_compartment(self.config, target, TEST_HOME), expected)
                self.assertEqual(self.c.folder_compartment(self.config, self.c.encode("~/.hub/projects/garden")), expected)
        # Existence in a different hub does not make this hub's project real.
        target = "~/.other-hub/projects/garden/queue.md"
        self.config["hub"]["routes"].pop("GARDEN")
        self.assertEqual(self.c.hub_compartment(self.config, ENCODED_HOME + "--hub", "work", [self.write_tool(target)]), "work")
        self.assertEqual(self.c.path_compartment(self.config, target, TEST_HOME), "work")

    def test_hub_projects_never_probe_a_fenced_directory(self):
        target = TEST_HOME / ".hub/projects/blocked"
        self.config["fenced"].append(str(target))
        self.config["hub"]["routes"]["blocked"] = "client"
        isdir = os.path.isdir
        def guarded(path):
            self.assertFalse(self.c.under(Path(path), target), "probed fenced project")
            return isdir(path)
        with patch.object(self.c.os.path, "isdir", side_effect=guarded):
            self.assertEqual(self.c.hub_compartment(self.config, ENCODED_HOME + "--hub", "work", [self.write_tool(target / "queue.md")]), "work")
            self.assertEqual(self.c.path_compartment(self.config, target / "queue.md", TEST_HOME), "fenced")
            self.assertEqual(self.c.folder_compartment(self.config, self.c.encode(target)), "fenced")

    def test_hub_route_targets(self):
        for target in ("work", "p:garden-site", "p:.site", "p:Garden_2.0"):
            with self.subTest(target=target):
                self.c.check_compartments({**self.config, "hub": {**self.config["hub"], "routes": {"garden": target}}})
        for target in ("p:", "p:a/b", "p:*", "p:-site", "absent", None):
            with self.subTest(target=target):
                with self.assertRaisesRegex(self.c.Failed, "hub.routes.garden"):
                    self.c.check_compartments({**self.config, "hub": {**self.config["hub"], "routes": {"garden": target}}})

    def test_load_member_compartments_path(self):
        from unittest.mock import patch
        with self.assertRaisesRegex(self.c.Failed,
                                    "compartments: a path given to load_member must be absolute or start with ~/"):
            self.c.load_member({**ADA, "compartments": "compartments.json"})
        for path in (str(self.h.root / "compartments.json"), "~/compartments.json"):
            with patch.dict(os.environ, {"HOME": str(self.h.root)}):
                member = self.c.load_member({**ADA, "compartments": path})
            self.assertEqual(member.compartments, self.config)
        self.assertEqual(self.c.load_member(self.c.read_member(self.member)).compartments, self.config)

    def test_hub_routing_from_each_folder(self):
        self.config["compartments"]["client"]["folders"].append("~/.client-hub")
        self.config["hub"]["paths"].append("~/.client-hub/projects")
        record = {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "input": {"file_path": "~/.other-hub/projects/client-stack/x.md"}}]}}
        self.assertEqual(self.c.hub_compartment(self.config, self.c.encode("~/.hub"), "work", [record]), "client")
        self.assertEqual(self.c.hub_compartment(self.config, self.c.encode("~/.client-hub"), "client", [record]), "client")
        self.config["hub"]["routes"]["work-stack"] = "work"
        record["message"]["content"][0]["input"]["other"] = "~/.client-hub/projects/work-stack/x.md"
        self.assertEqual(self.c.hub_compartment(self.config, self.c.encode("~/.hub"), "work", [record]), "client")
        self.assertEqual(self.c.hub_compartment(self.config, self.c.encode("~/.client-hub"), "client", [record]), "work")
        self.assertEqual(self.c.hub_compartment(self.config, self.c.encode("~/outside"), "p:outside", [record]), "p:outside")

    def test_where_similar_and_record(self):
        rows = [{"id": str(i), "kind": "message", "timestamp": "2026-09-01T00:00:00Z",
                                      "project": project, "text": "keep every review queue verdict for later scoring", "context": "", "split": "train",
                                      "compartment": name}
                                     for i, (project, name) in enumerate((("work", "work"), ("client", "client"),
                                                                         ("private", "fenced"), ("hub", "hub-mixed")))]
        (self.h.root / "corpus.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
        for cwd, name, code in ((str(TEST_HOME / "projects/work"), "work", 0), (str(TEST_HOME / "garden-club"), "fenced", 4)):
            proc = self.h.crew(self.member, "where", "--cwd", cwd, "--json")
            self.assertEqual(proc.returncode, code, proc.stderr)
            self.assertEqual(json.loads(proc.stdout), {"compartment": name, "fenced": code == 4})
        for flags, names in ((("--compartment", "work"), ["work"]),
                             (("--cwd", str(TEST_HOME / "projects/client")), ["client"]),
                             (("--compartment", "all"), ["work", "client", "hub"])):
            proc = self.h.crew(self.member, "similar", "--corpus", "corpus.jsonl", "--query", "review queue",
                               "--json", *flags)
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertEqual({r["project"] for r in json.loads(proc.stdout)}, set(names))
        proc = self.h.crew(self.member, "similar", "--corpus", "missing", "--query", "q",
                           "--cwd", str(TEST_HOME / "garden-club"))
        self.assertEqual(proc.returncode, 4, proc.stderr)
        self.assertIn("refusing: this folder is fenced", proc.stderr)
        common = ("--log", "live.jsonl", "--kind", "decided", "--category", "scope", "--tier", "high",
                  "--question", "Queue?", "--ada-pick", "Keep")
        for cwd, work in ((str(TEST_HOME / "garden-club"), "x.md"), (str(TEST_HOME / "projects/work"), str(TEST_HOME / "garden-club/x"))):
            proc = self.h.crew(self.member, "record", *common, "--cwd", cwd, "--work", work)
            self.assertEqual(proc.returncode, 4, proc.stderr)
            self.assertIn("refusing: fenced", proc.stderr)
            self.assertFalse((self.h.root / "live.jsonl").exists())
        proc = self.h.crew(self.member, "record", *common, "--cwd", str(TEST_HOME / "projects/work"), "--work", "notes/x")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(json.loads(self.h.read("live.jsonl"))["compartment"], "work")

    def test_validation_and_no_config(self):
        import copy
        for key, value in (("version", 2), ("unknown", []), ("fenced", ["relative"]),
                           ("compartments", {"all": {"folders": []}}),
                           ("hub", {"paths": ["~/.hub"], "routes": {"x": "absent"}})):
            config = copy.deepcopy(self.config)
            config[key] = value
            self.h.write("compartments.json", config)
            proc = self.h.crew(self.member, "where")
            self.assertEqual(proc.returncode, 2, proc.stderr)
            assert_cli_error(self, proc.stderr, "crew_decisions:")
            self.assertIn(key, proc.stderr)
        member = self.h.member(ADA)
        proc = self.h.crew(member, "where")
        self.assertEqual((proc.returncode, proc.stdout), (0, "all\n"), proc.stderr)
        self.h.session(ENCODED_HOME + "-projects-work", "s", said("2026-09-01T00:00:00Z", "keep the review queue"))
        proc = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "out.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertNotIn("compartment", json.loads(self.h.read("out.jsonl")))

    def test_hub_routing_only_from_hub_folders(self):
        record = self.write_tool("~/.hub/projects/client-stack/x.md")
        for folder, compartment, expected in (
                ("~/projects/work", "work", "work"),
                ("~/projects/work--claude-worktrees-x", "work", "work"),
                ("~/.hub", "work", "client"),
                ("~/.hub--claude-worktrees-x", "work", "client"),
                ("~/.other-hub", "work", "client"),
                ("~/.hub/projects/client-stack", "client", "client"),
                ("~/projects/elsewhere", "p:elsewhere", "p:elsewhere")):
            with self.subTest(folder=folder):
                self.assertEqual(self.c.hub_compartment(self.config, self.c.encode(folder), compartment, [record]), expected)
        self.assertEqual(self.c.hub_compartment(self.config, self.c.encode("~/.HUB").upper(), "work", [record]), "client")

    def test_hub_routing_requires_the_matching_parents_owner(self):
        self.config["hub"]["paths"].append("~/projects")
        self.config["compartments"]["work"]["folders"].append("~/work")
        record = self.write_tool("~/projects/client-stack/x.md")
        for folder, compartment in (("~/work", "work"), ("~/anything/else", "p:else")):
            with self.subTest(folder=folder):
                self.assertEqual(self.c.hub_compartment(self.config, self.c.encode(folder), compartment, [record]), compartment)

    def test_mine_keeps_non_hub_session_compartment(self):
        for folder, session in ((ENCODED_HOME + "-projects-work", "work"), (ENCODED_HOME + "--hub", "hub")):
            self.h.session(folder, session, self.write_tool("~/.hub/projects/client-stack/x.md"),
                           said("2026-09-01T00:00:00Z", "keep every review queue verdict for later scoring"))
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "mined.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        rows = [json.loads(line) for line in self.h.read("mined.jsonl").splitlines()]
        self.assertEqual({r["project"]: r["compartment"] for r in rows}, {"work": "work", ".hub": "client"})
        self.assertEqual(len({r["id"] for r in rows}), len(rows))

    def test_hub_project_folder_keeps_its_compartment(self):
        self.config["hub"]["routes"].update({"garden": "p:garden", "orchard": "p:orchard"})
        for hub in self.config["hub"]["paths"]:
            for suffix in ("", "--claude-worktrees-x"):
                for targets in (("garden",), ("garden", "orchard")):
                    with self.subTest(hub=hub, suffix=suffix, targets=targets):
                        records = [self.write_tool(hub + "/" + target + "/queue.md") for target in targets]
                        folder = self.c.encode(hub + "/client-stack") + suffix
                        self.assertEqual(self.c.hub_compartment(self.config, folder, "client", records), "client")
                        self.assertEqual(self.c.hub_compartment(self.config, folder.upper(), "client", records), "client")
        record = self.write_tool("~/.hub/projects/garden/queue.md")
        for folder in ("~/.hub/projects", "~/.hub", "~/.hub-x", "~/.hub/notes"):
            with self.subTest(folder=folder):
                self.assertEqual(self.c.hub_compartment(self.config, self.c.encode(folder), "work", [record]), "p:garden")

    def test_harvest_skips_repeated_user_uuid(self):
        m = self.c.load_member(self.c.read_member(self.member))
        record = said("2026-09-01T00:00:00Z", "keep every review queue verdict for later scoring", uuid="u1")
        folder = self.c.encode("~/projects/work")
        self.h.session(folder, "resumed", record, reply("2026-09-02T00:00:00Z", "Review it again?"), record)
        path = self.h.projects / folder / "resumed.jsonl"
        self.assertEqual(len(self.c.harvest(m, path, "work")), 1)
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "mined.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        rows = [json.loads(line) for line in self.h.read("mined.jsonl").splitlines()]
        self.assertEqual(len(rows), 1)
        self.assertEqual(len({r["id"] for r in rows}), len(rows))

    def test_mine_deduplicates_ids_with_distinct_or_missing_uuids(self):
        m = self.c.load_member(self.c.read_member(self.member))
        for uuids in (("u1", "u2"), (None, None), (1, 1)):
            with self.subTest(uuids=uuids):
                records = [said("2026-09-01T00:00:00Z", "keep every review queue verdict for later scoring",
                                **({"uuid": uuid} if uuid else {})) for uuid in uuids]
                folder = self.c.encode("~/projects/work")
                self.h.session(folder, "resumed", *records)
                path = self.h.projects / folder / "resumed.jsonl"
                self.assertEqual(len(self.c.harvest(m, path, "work")), 2)
                proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "mined.jsonl")
                self.assertEqual(proc.returncode, 0, proc.stderr)
                rows = [json.loads(line) for line in self.h.read("mined.jsonl").splitlines()]
                self.assertEqual(len({r["id"] for r in rows}), len(rows))
                self.assertEqual(len(rows), 1)
                self.assertIn(", 1 duplicate records", proc.stdout)

    def test_mine_prefers_record_cwd_folder_for_duplicate_ids(self):
        self.config["compartments"]["work"]["folders"].append("~/projects/a")
        self.config["compartments"]["client"]["folders"].append("~/projects/b")
        self.h.write("compartments.json", self.config)
        for cwd, expected in (("~/projects/b--claude-worktrees-x", "b"),
                              ("~/projects/B--claude-worktrees-X", "b"),
                              ("~/projects/b", "b"), ("~/projects/b/sub", "b"),
                              (None, "a"), ("~/projects/a", "a")):
            with self.subTest(cwd=cwd):
                record = said("2026-09-01T00:00:00Z", "keep every review queue verdict for later scoring")
                record.pop("cwd")
                if cwd is not None:
                    record["cwd"] = str(TEST_HOME / cwd[2:])
                for folder in ("~/projects/a", "~/projects/b--claude-worktrees-x"):
                    self.h.session(self.c.encode(folder), "resumed", record)
                proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "mined.jsonl")
                self.assertEqual(proc.returncode, 0, proc.stderr)
                rows = [json.loads(line) for line in self.h.read("mined.jsonl").splitlines()]
                self.assertEqual(len({r["id"] for r in rows}), len(rows))
                self.assertEqual(len(rows), 1)
                self.assertEqual(rows[0]["project"], expected)
                self.assertEqual(rows[0]["compartment"], "client" if expected == "b" else "work")
                self.assertEqual(set(rows[0]), {"id", "kind", "timestamp", "project", "text", "context", "compartment", "split"})
                self.assertIn(", 1 duplicate records", proc.stdout)

    def test_mine_drops_records_with_fenced_cwd_without_probing(self):
        fenced = self.h.root / "private"
        fenced.mkdir()
        fenced.chmod(0)
        self.addCleanup(fenced.chmod, 0o700)
        self.config["fenced"] = [str(fenced)]
        self.h.write("compartments.json", self.config)
        m = self.c.load_member(self.c.read_member(self.member))
        for index, cwd in enumerate((str(fenced / "sub"), self.c.encode(fenced) + "-sub",
                                     str(self.h.root / "outside/../private/sub"))):
            self.h.session(self.c.encode("~/projects/work"), str(index),
                           said("2026-09-01T00:00:00Z", "keep every review queue verdict " + str(index), cwd=cwd))
        args = self.c.parser_for(m, "test").parse_args([
            "mine", "--projects", str(self.h.projects), "--out", str(self.h.root / "mined.jsonl")])
        output = io.StringIO()
        with ExitStack() as stack:
            for owner, method in ((Path, "stat"), (Path, "open"), (builtins, "open"),
                                  (os, "open"), (os, "stat"), (os, "lstat")):
                original = getattr(owner, method)
                def guarded(path, *args, _original=original, **kwargs):
                    if isinstance(path, (str, bytes, os.PathLike)):
                        target = Path(os.path.abspath(os.fsdecode(path)))
                        self.assertFalse(self.c.under(target, fenced), "probed fenced cwd: " + str(target))
                    return _original(path, *args, **kwargs)
                stack.enter_context(patch.object(owner, method, guarded))
            with redirect_stdout(output):
                self.assertEqual(self.c.cmd_mine(m, args), 0)
        self.assertEqual(self.h.read("mined.jsonl"), "")
        self.assertIn(", 3 fenced records", output.getvalue())

    def test_mine_drops_records_with_excluded_cwd(self):
        member = self.h.member({**ADA, "filters": {**ADA["filters"], "excluded_folders": "excluded-project"}})
        self.h.session(self.c.encode("~/projects/work"), "resumed",
                       said("2026-09-01T00:00:00Z", "keep every review queue verdict", cwd="~/excluded-project/sub"))
        proc = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "mined.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(self.h.read("mined.jsonl"), "")
        self.assertIn(", 1 excluded records", proc.stdout)

    def test_mine_filters_transcript_cwds_before_context_and_hub_routing(self):
        for reason, cwd in (("fenced", "~/garden-club/case"),
                            ("excluded", "~/excluded-project/case")):
            with self.subTest(reason=reason):
                member = self.h.member({**ADA, "compartments": "compartments.json",
                                       "filters": {**ADA["filters"], "excluded_folders": "excluded-project"}})
                marker = reason.upper() + "-REPLY-TEXT"
                assistant = reply("2026-09-01T00:01:00Z", marker)
                assistant["cwd"] = cwd
                tool = {"type": "assistant", "cwd": cwd, "message": {"content": [
                    {"type": "tool_use", "name": "Read",
                     "input": {"file_path": "~/.hub/projects/client-stack/x.md"}}]}}
                for folder in ("~/projects/work", "~/.hub"):
                    self.h.session(self.c.encode(folder), "resumed",
                                   said("2026-09-01T00:00:00Z", "drop this review queue verdict", cwd=cwd),
                                   assistant, tool,
                                   said("2026-09-01T00:02:00Z", "keep every review queue verdict " + folder,
                                        cwd="~/projects/work"))
                proc = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "mined.jsonl")
                self.assertEqual(proc.returncode, 0, proc.stderr)
                rows = [json.loads(line) for line in self.h.read("mined.jsonl").splitlines()]
                self.assertEqual(len(rows), 2)
                for row in rows:
                    with self.subTest(project=row["project"], field="context"):
                        self.assertNotIn(marker, row["context"])
                    with self.subTest(project=row["project"], field="text"):
                        self.assertNotIn(marker, row["text"])
                    with self.subTest(project=row["project"], field="compartment"):
                        self.assertEqual(row["compartment"], "work")
                self.assertNotIn(marker, proc.stdout)
                self.assertIn(", 6 " + reason + " records", proc.stdout)
                other = "excluded" if reason == "fenced" else "fenced"
                self.assertNotIn(other + " records", proc.stdout)

    def test_fenced_scans_and_hub_sessions(self):
        if os.geteuid() == 0:
            self.skipTest("mode 000 requires an unprivileged user")
        fenced = self.h.projects / (ENCODED_HOME + "-garden-club")
        fenced.mkdir(parents=True)
        fenced.chmod(0)
        self.addCleanup(fenced.chmod, 0o700)
        other = TEST_HOME / ".hub/projects/other"
        other.mkdir(parents=True)
        self.addCleanup(shutil.rmtree, other.parent.parent)
        for session, targets in (("one", ["~/.hub/projects/client-stack/x.md"]),
                                 ("mixed", [str(TEST_HOME / ".hub/projects/client-stack"), "~/.hub/projects/other/x.md"]),
                                 ("second", ["~/.other-hub/projects/client-stack/x.md"]),
                                 ("second-absolute", [str(TEST_HOME / ".other-hub/projects/client-stack/x.md")]),
                                 ("none", [])):
            tool = {"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Read",
                    "input": {"nested": [{"file_path": target} for target in targets]}}]}}
            self.h.session(ENCODED_HOME + "--hub", session, tool,
                           said("2026-09-01T00:00:00Z", "keep every review queue verdict for later scoring " + session))
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "mined.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("1 fenced folders", proc.stdout)
        rows = [json.loads(line) for line in self.h.read("mined.jsonl").splitlines()]
        self.assertEqual({r["text"].split()[-1]: r["compartment"] for r in rows},
                         {"one": "client", "mixed": "hub-mixed", "second": "client",
                          "second-absolute": "client", "none": "work"})
        for command in (("pending", "--seen", "seen.json"), ("folders",), ("folders", "--json")):
            proc = self.h.crew(self.member, *command, "--projects", str(self.h.projects))
            self.assertEqual(proc.returncode, 0, proc.stderr)
            if command == ("folders", "--json"):
                row = next(r for r in json.loads(proc.stdout) if r["state"] == "fenced")
                self.assertIsNone(row["sessions"])
                self.assertEqual(row["compartment"], "fenced")
            elif command == ("folders",):
                self.assertIn("fenced\t-\tgarden-club", proc.stdout)
                self.assertIn(", 1 fenced", proc.stdout)
        proc = self.h.crew(self.member, "similar", "--corpus", "mined.jsonl", "--query", "review queue",
                           "--compartment", "all")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        root = self.h.root / "docs"
        root.mkdir()
        self.h.write("private.json", {"sources": [{"name": "notes", "kind": "folder", "path": str(root),
                                                  "consent": "2026-09-01"}]})
        proc = self.h.crew(self.member, "sources", "review", "--name", "notes", "--projects", str(self.h.projects))
        self.assertEqual(proc.returncode, 0, proc.stderr)

    def test_fenced_document_roots(self):
        root = self.h.root / "docs"
        root.mkdir()
        self.config["fenced"] = [str(root / "private")]
        self.h.write("compartments.json", self.config)
        for path in (root, root / "private", root / "private/sub"):
            proc = self.h.crew(self.member, "documents", "--dir", str(path), "--out-dir", "out")
            self.assertNotEqual(proc.returncode, 0)
            self.assertIn("fenced", proc.stderr)
            self.h.write("private.json", {"sources": [{"name": "notes", "kind": "folder", "path": str(path),
                                                      "consent": "2026-09-01"}]})
            proc = self.h.crew(self.member, "sources", "review", "--name", "notes", "--projects", "absent")
            self.assertNotEqual(proc.returncode, 0)
            self.assertIn("fenced", proc.stderr)

    def test_alias_into_fenced_is_never_scanned(self):
        from unittest.mock import patch
        secret = self.h.root / "secret"
        secret.mkdir()
        self.config["fenced"] = [str(secret)]
        self.h.write("compartments.json", self.config)
        self.h.projects.mkdir()
        (self.h.projects / "alias").symlink_to(secret)
        m = self.c.load_member(self.c.read_member(self.member))
        original = Path.rglob
        def guarded(path, pattern):
            self.assertNotEqual(path, self.h.projects, "must not recursively scan projects root")
            self.assertNotEqual(path.resolve(), secret, "must not traverse a fenced alias")
            return original(path, pattern)
        with patch.object(Path, "rglob", guarded):
            self.c.claude_touched(m, self.h.projects)
        for command in (("mine", "--out", "alias.jsonl"), ("pending",), ("folders", "--json")):
            proc = self.h.crew(self.member, *command, "--projects", str(self.h.projects))
            self.assertEqual(proc.returncode, 0, proc.stderr)
            if command[0] == "folders":
                self.assertEqual(json.loads(proc.stdout)[0]["state"], "fenced")

    def test_pending_compartment_refusals_do_not_touch_seen(self):
        for configured, scope, error in (
                (True, "fenced", "pending: refusing compartment fenced"),
                (False, "X", "pending: --compartment requires compartments configuration")):
            member = self.member if configured else self.h.member(ADA, "unscoped.json")
            for existing in (False, True):
                with self.subTest(configured=configured, existing=existing):
                    seen = self.h.root / "seen.json"
                    seen.unlink(missing_ok=True)
                    if existing:
                        seen.write_text('[ "already-seen" ]\n')
                    before = seen.read_bytes() if existing else None
                    proc = self.h.crew(member, "pending", "--compartment", scope,
                                       "--projects", str(self.h.projects), "--seen", str(seen))
                    self.assertNotEqual(proc.returncode, 0)
                    self.assertEqual(proc.stdout, "")
                    self.assertEqual(proc.stderr, "crew_decisions: " + error + "\n")
                    self.assertEqual(seen.read_bytes() if seen.exists() else None, before)

    def test_pending_compartment_leaves_other_events_unseen(self):
        for folder, session in (("work", "a"), ("client", "b")):
            self.h.session(self.c.encode("~/projects/" + folder), session,
                           asking("2026-09-01T00:00:00Z", "t", "Which queue?", "sdk-cli"))
        queues = self.h.root / "queues"
        folder = queues / "archive-stack"
        folder.mkdir(parents=True)
        (folder / "queue.md").write_text("🎛️ CONTROL-Q APP-1 · t · lane ALL\n")
        self.config["compartments"]["archive"] = {"folders": []}
        self.config["hub"]["paths"].append(str(queues))
        self.config["hub"]["routes"]["archive-stack"] = "archive"
        self.h.write("compartments.json", self.config)
        flags = ("pending", "--projects", str(self.h.projects), "--queues", str(queues),
                 "--seen", "seen.json")
        for scope, ids, seen in (
                (("--compartment", "work"), ["a:t"], ["a:t"]),
                (("--compartment", "work"), [], ["a:t"]),
                (("--compartment", "archive"), ["archive-stack:APP-1:1"],
                 ["a:t", "archive-stack:APP-1:1"]),
                ((), ["b:t"], ["a:t", "archive-stack:APP-1:1", "b:t"])):
            with self.subTest(scope=scope, ids=ids):
                proc = self.h.crew(self.member, *flags, *scope)
                self.assertEqual(proc.returncode, 0, proc.stderr)
                rows = [json.loads(line) for line in proc.stdout.splitlines()]
                self.assertEqual([r["id"] for r in rows], ids)
                self.assertEqual(json.loads(self.h.read("seen.json")), seen)
                if ids == ["archive-stack:APP-1:1"]:
                    self.assertEqual(rows[0]["compartment"], "archive")

    def test_pending_queue_uses_project_folder_for_questions_and_answers(self):
        queues = self.h.root / "queues"
        folder = queues / "app"
        folder.mkdir(parents=True)
        target = self.h.root / "other" / "queue.md"
        target.parent.mkdir()
        target.write_text("🎛️ CONTROL-Q APP-1 · t · lane ALL\n"
                          "🎛️ CONTROL-Q APP-2 · t · session s (Autonomous on)\n"
                          "🎛️ CONTROL-A APP-2 · t · Ari · answered by Robin\n")
        (folder / "queue.md").symlink_to(target)
        self.config["compartments"]["client"]["folders"].append(str(folder))
        self.config["compartments"]["work"]["folders"].append(str(target.parent))
        self.h.write("compartments.json", self.config)
        member = self.h.member({**ADA, "compartments": "compartments.json", "queue_answerer": "Ari"})
        for scope in ((), ("--compartment", "client")):
            with self.subTest(scope=scope):
                proc = self.h.crew(member, "pending", "--projects", str(self.h.projects),
                                   "--queues", str(queues), "--seen", "seen.json", *scope)
                self.assertEqual(proc.returncode, 0, proc.stderr)
                rows = [json.loads(line) for line in proc.stdout.splitlines()]
                self.assertEqual([(r["kind"], r["compartment"]) for r in rows],
                                 [("control-q", "client"), ("control-a", "client")])
                self.assertEqual(json.loads(self.h.read("seen.json")), ["app:APP-1:1", "app:APP-2:A:3"])
                (self.h.root / "seen.json").unlink()

    def test_pending_compartments_and_fenced_queues(self):
        self.h.session(ENCODED_HOME + "-projects-work", "s", {
            "type": "assistant", "timestamp": "2026-09-01T00:00:00Z", "entrypoint": "sdk-cli",
            "message": {"content": [{"type": "tool_use", "id": "t", "name": "AskUserQuestion",
                        "input": {"questions": [{"question": "Which queue?", "options": [{"label": "Keep"}]}]}}]}})
        queues = self.h.root / "queues"
        for project in ("client-stack", "other", "blocked"):
            folder = queues / project
            folder.mkdir(parents=True)
            (folder / "queue.md").write_text("🎛️ CONTROL-Q APP-1 · t · lane ALL\n")
        self.config["hub"]["paths"].append(str(queues))
        self.config["fenced"].append(str(queues / "blocked"))
        self.h.write("compartments.json", self.config)
        blocked = queues / "blocked" / "queue.md"
        blocked.write_text("🎛️ CONTROL-Q APP-2 · t · session s (Autonomous on)\n"
                           "🎛️ CONTROL-A APP-2 · t · Ari · answered by Robin\n")
        blocked.chmod(0)
        self.addCleanup(blocked.chmod, 0o600)
        member = self.h.member({**ADA, "compartments": "compartments.json", "queue_answerer": "Ari"})
        for scope, expected in (((), {"work": "work", "client-stack": "client", "other": "p:other"}),
                                (("--compartment", "client"), {"client-stack": "client"})):
            with self.subTest(scope=scope):
                (self.h.root / "seen.json").unlink(missing_ok=True)
                proc = self.h.crew(member, "pending", "--projects", str(self.h.projects),
                                   "--queues", str(queues), "--seen", "seen.json", *scope)
                self.assertEqual(proc.returncode, 0, proc.stderr)
                rows = [json.loads(line) for line in proc.stdout.splitlines()]
                self.assertEqual({r["project"]: r["compartment"] for r in rows}, expected)
                self.assertEqual(json.loads(self.h.read("seen.json")), sorted(r["id"] for r in rows))

    def test_stdin_fence_and_cwd_precedence(self):
        from argparse import Namespace
        from unittest.mock import patch
        with patch.dict(os.environ, {"CLAUDE_PROJECT_DIR": "/env"}):
            self.assertEqual(self.c.cwd_of(Namespace(cwd="/flag")), "/flag")
            self.assertEqual(self.c.cwd_of(Namespace(cwd=None)), "/env")
        with patch.dict(os.environ, {"CLAUDE_PROJECT_DIR": ""}):
            self.assertEqual(self.c.cwd_of(Namespace()), os.getcwd())
        for cwd, work, code in ((str(TEST_HOME / "garden-club"), "notes", 4),
                                (str(TEST_HOME / "projects/work"), str(TEST_HOME / "garden-club/x"), 4),
                                (str(TEST_HOME / "projects/work"), "notes", 0)):
            data = {"work": work, "kind": "decided", "category": "scope", "tier": "high",
                    "question": "Which queue?", "pick": "Keep"}
            proc = subprocess.run([sys.executable, str(CREW), "--member", self.member, "record", "--stdin",
                                   "--cwd", cwd, "--log", "stdin.jsonl"], input=json.dumps(data), text=True,
                                  capture_output=True, cwd=self.h.root, env={**os.environ, "HOME": str(TEST_HOME)})
            self.assertEqual(proc.returncode, code, proc.stderr)
            if code == 4:
                self.assertFalse((self.h.root / "stdin.jsonl").exists())
        self.assertEqual(json.loads(self.h.read("stdin.jsonl"))["compartment"], "work")

    def test_fenced_transcript_file_alias(self):
        secret = self.h.root / "secret"
        secret.mkdir()
        target = secret / "session.jsonl"
        target.write_text(json.dumps(said("2026-09-01T00:00:00Z", "keep the queue")) + "\n")
        self.config["fenced"] = [str(secret)]
        self.h.write("compartments.json", self.config)
        folder = self.h.projects / (ENCODED_HOME + "-projects-work")
        folder.mkdir(parents=True)
        (folder / "alias.jsonl").symlink_to(target)
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "out.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(self.h.read("out.jsonl"), "")

    def test_scans_never_probe_fenced_entries(self):
        from contextlib import redirect_stdout
        from io import StringIO
        from unittest.mock import patch
        fenced = self.h.projects / self.c.encode("~/garden-club")
        fenced.mkdir(parents=True)
        m = self.c.load_member(self.c.read_member(self.member))
        for method in ("stat", "glob", "rglob", "iterdir", "read_text", "read_bytes"):
            original = getattr(Path, method)
            def guarded(path, *args, _original=original, **kwargs):
                self.assertFalse(self.c.under(path, fenced), "probed fenced entry: " + str(path))
                return _original(path, *args, **kwargs)
            with patch.object(Path, method, guarded), redirect_stdout(StringIO()):
                for command in (("mine", "--out", str(self.h.root / "out.jsonl")), ("pending",),
                                ("folders",), ("folders", "--json")):
                    args = self.c.parser_for(m, "test").parse_args([*command, "--projects", str(self.h.projects)])
                    self.assertEqual(self.c.COMMANDS[args.cmd](m, args), 0)
                self.c.claude_touched(m, self.h.projects)

    def test_loaded_config_is_validated_and_file_read_once(self):
        from unittest.mock import patch
        raw = self.c.read_member(self.member)
        raw["compartments"]["version"] = 2
        with self.assertRaises(self.c.Failed):
            self.c.load_member(raw)
        original = Path.read_text
        reads = []
        def counted(path, *args, **kwargs):
            if path.name == "compartments.json":
                reads.append(path)
            return original(path, *args, **kwargs)
        with patch.object(Path, "read_text", counted):
            config = self.c.read_member(self.member)
            self.c.load_member(config)
            self.c.load_member(config)
        self.assertEqual(len(reads), 1)
        self.h.write("override.json", {**self.config, "fenced": []})
        self.h.write("private.json", {"compartments": "override.json"})
        member = self.c.load_member(self.c.read_member(self.member))
        self.assertEqual(member.compartments["fenced"], [])

    def test_review_never_walks_a_nested_fence(self):
        from unittest.mock import patch
        folder = self.h.projects / (ENCODED_HOME + "-projects-work")
        fenced = folder / "private"
        fenced.mkdir(parents=True)
        self.config["fenced"] = [str(fenced)]
        self.h.write("compartments.json", self.config)
        m = self.c.load_member(self.c.read_member(self.member))
        original = os.scandir
        def guarded(path):
            self.assertNotEqual(Path(path), fenced, "walked a nested fence")
            return original(path)
        with patch.object(os, "scandir", guarded):
            self.assertEqual(self.c.claude_touched(m, self.h.projects), {})


    def test_fenced_projects_roots_are_refused_before_probing(self):
        from unittest.mock import patch
        root = self.h.root / "secret"
        root.mkdir()
        alias = self.h.root / "alias"
        alias.symlink_to(root)
        self.config["fenced"] = [str(root)]
        self.h.write("compartments.json", self.config)
        m = self.c.load_member(self.c.read_member(self.member))
        for path in (root, root / "sub", alias):
            for method in ("stat", "iterdir", "glob", "rglob"):
                original = getattr(Path, method)
                def guarded(target, *args, _original=original, **kwargs):
                    self.assertFalse(target == path or self.c.under(target, root), "probed fenced root")
                    return _original(target, *args, **kwargs)
                with patch.object(Path, method, guarded):
                    for command in (("mine", "--out", str(self.h.root / "out.jsonl")), ("pending",), ("folders",)):
                        args = self.c.parser_for(m, "test").parse_args([*command, "--projects", str(path)])
                        with self.assertRaises(self.c.Fenced):
                            self.c.COMMANDS[args.cmd](m, args)
                    with self.assertRaises(self.c.Fenced):
                        self.c.claude_touched(m, path)


    def test_hub_routing_uses_input_values_only(self):
        m = self.c.load_member(self.c.read_member(self.member))
        record = {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "input": {"~/.hub/projects/client-stack/x.md": "nothing"}}]}}
        self.assertEqual(self.c.hub_compartment(m.compartments, ENCODED_HOME + "--hub", "work", [record]), "work")

    def save_config(self):
        self.h.write("compartments.json", self.config)
        return self.c.load_member(self.c.read_member(self.member))

    def test_case_and_unicode_fence_cli_and_mine(self):
        self.config["fenced"] = ["~/Garden Club", "~/Café"]
        m = self.save_config()
        commands = (("where", "--cwd", "~/garden club"),
                    ("record", "--work", "~/GARDEN CLUB/x.md", "--log", "live.jsonl"),
                    ("documents", "--dir", "~/garden club", "--out-dir", "out"),
                    ("where", "--cwd", "~/Cafe\u0301"))
        for command in commands:
            with self.subTest(command=command):
                proc = self.h.crew(self.member, *command)
                self.assertEqual(proc.returncode, 4, proc.stderr)
        self.assertFalse((self.h.root / "live.jsonl").exists())
        folder = self.c.encode("~/Garden Club").lower()
        self.h.session(folder, "s", said("2026-09-01T00:00:00Z", "never read this"))
        with patch.object(self.c, "read_session", side_effect=AssertionError("read fenced transcript")):
            args = self.c.parser_for(m, "test").parse_args([
                "mine", "--projects", str(self.h.projects), "--out", str(self.h.root / "mine.jsonl")])
            self.c.cmd_mine(m, args)
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "out.jsonl")
        self.assertIn("1 fenced folders", proc.stdout)
        self.assertEqual(self.h.read("out.jsonl"), "")

    def test_folded_comparisons_and_original_names(self):
        self.config["fenced"] = ["~/Cafe\u0301"]
        m = self.save_config()
        c = m.compartments
        self.assertEqual(self.c.path_compartment(c, "~/Café/x", TEST_HOME), "fenced")
        self.assertTrue(self.c.under(Path("/A/Café/x"), Path("/a/Cafe\u0301")))
        self.assertEqual(self.c.folder_compartment(c, self.c.encode("~/PROJECTS/WORK")), "work")
        self.assertEqual(self.c.path_compartment(c, "~/PROJECTS/WORK/x", TEST_HOME), "work")
        self.assertEqual(self.c.record_compartment(c, {"project": "WORK"}), "work")
        self.assertEqual(self.c.path_compartment(c, "~/.HUB/PROJECTS/CLIENT-STACK/x", TEST_HOME), "client")
        self.assertEqual(self.c.path_compartment(c, "~/.HUB/PROJECTS/New-App/x", TEST_HOME), "work")
        self.assertTrue(self.c.overlaps_fenced(m, str(TEST_HOME).upper()))
        self.assertEqual(self.c.queue_compartment(m,
                         Path("~/.HUB/PROJECTS/CLIENT-STACK/queue.md")), {"compartment": "client"})
        self.assertEqual(self.c.path_compartment(c, self.c.encode("~/Cafe\u0301").upper(), TEST_HOME), "fenced")

    def test_similar_override_cannot_bypass_fence(self):
        for compartment in ("all", "work", "p:other", "auto"):
            proc = self.h.crew(self.member, "similar", "--compartment", compartment,
                               "--cwd", "~/garden-club", "--corpus", "missing", "--query", "q")
            self.assertEqual(proc.returncode, 4, proc.stderr)

    def test_similar_rejects_unknown_compartment_one_line(self):
        for compartment in ("typo", "fenced", "hub-mixed", "p:"):
            proc = self.h.crew(self.member, "similar", "--compartment", compartment,
                               "--corpus", "missing", "--query", "q")
            self.assertEqual(proc.returncode, 2, proc.stderr)
            self.assertEqual(len(proc.stderr.splitlines()), 1)

    def test_legacy_scratchpad_project_fence_substring(self):
        project = "private-tmp-" + self.c.project_name(self.c.encode("~/garden-club")).upper() + "-id-scratchpad"
        self.assertEqual(self.c.record_compartment(self.config, {"project": project}), "fenced")

    def test_hub_shell_boundaries_and_home_forms(self):
        absolute = str(TEST_HOME / ".hub/projects")
        for hub in (absolute, "~/.hub/projects", "$HOME/.hub/projects", "${HOME}/.hub/projects"):
            for suffix in ("", "/file", " && x", '"', "'", "`", ";x", "&x", "|x", ")", ">x", "<x"):
                with self.subTest(hub=hub, suffix=suffix):
                    text = 'cd "' + hub.upper().replace("$HOME", "$HOME") + "/CLIENT-STACK" + suffix
                    record = {"type": "assistant", "message": {"content": [
                        {"type": "tool_use", "input": {"command": text}}]}}
                    self.assertEqual(self.c.hub_compartment(self.config, ENCODED_HOME + "--hub", "work", [record]), "client")

    def test_hub_unicode_prefix_and_route_casefold(self):
        self.config["hub"]["paths"] = ["~/Café/hub"]
        self.config["compartments"]["work"]["folders"].append("~/Café")
        self.config["hub"]["routes"]["Straße"] = "client"
        record = {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "input": {"command": "ls ~/Cafe\u0301/HUB/STRASSE && x"}}]}}
        self.assertEqual(self.c.hub_compartment(self.config, self.c.encode("~/Café"), "work", [record]), "client")

    def test_hub_routing_megabyte_strings_are_linear(self):
        mention = "~/.hub/projects/client-stack/queue.md "
        values = (("a/" * (1024 * 512), "work"),
                  ((mention * (1024 * 1024 // len(mention) + 1))[:1024 * 1024], "client"))
        def deadline(signum, frame):
            self.fail("hub routing exceeded 2 seconds of CPU time")
        old_handler = signal.signal(signal.SIGPROF, deadline)
        try:
            for value, expected in values:
                with self.subTest(expected=expected):
                    signal.setitimer(signal.ITIMER_PROF, 2)
                    start = time.process_time()
                    actual = self.c.hub_compartment(self.config, ENCODED_HOME + "--hub", "work", [self.write_tool(value)])
                    elapsed = time.process_time() - start
                    signal.setitimer(signal.ITIMER_PROF, 0)
                    self.assertEqual(actual, expected)
                    self.assertLess(elapsed, 2)
        finally:
            signal.setitimer(signal.ITIMER_PROF, 0)
            signal.signal(signal.SIGPROF, old_handler)

    def test_hub_session_folder_longest_route(self):
        self.config["hub"]["routes"]["client"] = "work"
        for suffix, expected in (("client-stack", "client"), ("client-stack--claude-worktrees-one", "client"),
                                 ("Other-App", "work"), ("clientele", "work")):
            folder = self.c.encode("~/.HUB/PROJECTS") + "-" + suffix
            self.assertEqual(self.c.folder_compartment(self.config, folder), expected)

    @staticmethod
    def write_tool(target):
        return {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "name": "Write", "input": {"file_path": str(target)}}]}}

    def test_touched_skips_fenced_target_before_realpath(self):
        fenced = self.h.root / "fenced"
        fenced.mkdir()
        outside = self.h.root / "outside.md"
        outside.write_text("outside")
        target = fenced / "link"
        target.symlink_to(outside)
        self.config["fenced"] = [str(fenced)]
        m = self.save_config()
        self.h.session("allowed", "s", self.write_tool(target))
        realpath = os.path.realpath
        def guarded(path, *args, **kwargs):
            self.assertFalse(Path(path) != fenced and self.c.under(Path(path), fenced), "realpath saw fenced argument")
            return realpath(path, *args, **kwargs)
        # Prime root forms separately: only the tool target must never be resolved.
        self.c.path_compartment(m.compartments, self.h.projects, self.h.root)
        with patch.object(os.path, "realpath", guarded):
            self.assertEqual(self.c.claude_touched(m, self.h.projects), {})

    def test_link_into_fence_never_probes_fence_or_target(self):
        fenced = self.h.root.resolve() / "fenced"
        fenced.mkdir()
        (self.h.root / "lnk").symlink_to(fenced, target_is_directory=True)
        self.config["fenced"] = [str(fenced)]
        m = self.save_config()
        self.h.session("allowed", "s", self.write_tool("~/lnk/c.md"))

        def guard(original):
            def checked(path, *args, **kwargs):
                if not isinstance(path, int):
                    self.assertFalse(self.c.under(Path(path), fenced), "probed fence: " + str(path))
                return original(path, *args, **kwargs)
            return checked

        with ExitStack() as stack:
            stack.enter_context(patch.dict(os.environ, {"HOME": str(self.h.root)}))
            for owner, name in ((os, "lstat"), (os, "readlink"), (os, "stat"),
                                (os.path, "realpath"), (Path, "resolve")):
                stack.enter_context(patch.object(owner, name, guard(getattr(owner, name))))
            with patch.object(os, "lstat", side_effect=AssertionError("lexical fence probed")):
                self.assertEqual(self.c.safe_resolve(m.compartments, fenced / "c.md", self.h.root),
                                 (fenced / "c.md", True))
            # No priming: resolving the fence root itself must also avoid probing it.
            self.assertIn(fenced, self.c.fenced_forms(m.compartments, self.h.root))
            self.assertEqual(self.c.claude_touched(m, self.h.projects), {})
            self.assertEqual(self.c.path_compartment(m.compartments, "~/lnk/c.md", self.h.root), "fenced")
            with redirect_stderr(io.StringIO()), redirect_stdout(io.StringIO()):
                self.assertEqual(self.c.main(["--member", self.member, "record", "--work", "~/lnk/c.md",
                    "--kind", "decided", "--category", "scope", "--tier", "high", "--question", "q",
                    "--ada-pick", "keep"]), 4)

    def test_parent_components_after_links_never_probe_fence(self):
        base = self.h.root.resolve()
        fenced = base / "elsewhere/a/fence"
        (base / "elsewhere/a/b").mkdir(parents=True)
        fenced.mkdir()
        (base / "deep").symlink_to("elsewhere/a/b", target_is_directory=True)
        (base / "lnk").symlink_to("deep/../fence", target_is_directory=True)
        # Climb twice from the link's real target, then return to its parent.
        (base / "climb").symlink_to("deep/../../a/fence", target_is_directory=True)
        self.config["fenced"] = [str(fenced)]
        self.save_config()

        def guard(original):
            def checked(path, *args, **kwargs):
                if not isinstance(path, int):
                    self.assertFalse(self.c.under(Path(path), fenced), "probed fence: " + str(path))
                return original(path, *args, **kwargs)
            return checked

        with ExitStack() as stack:
            stack.enter_context(patch.dict(os.environ, {"HOME": str(base)}))
            for owner, name in ((os, "lstat"), (os, "stat"), (os, "readlink"),
                                (os.path, "isdir"), (os.path, "lexists"), (os.path, "realpath"),
                                (Path, "resolve"), (io, "open"), (builtins, "open")):
                stack.enter_context(patch.object(owner, name, guard(getattr(owner, name))))
            for target in ("~/lnk", "~/deep/../fence", "~/climb"):
                for entry in ("path", "work", "log", "cwd", "touched"):
                    with self.subTest(target=target, entry=entry):
                        m = self.c.load_member(self.c.read_member(self.member))
                        if entry == "path":
                            self.assertEqual(self.c.path_compartment(m.compartments, target + "/c.md", base), "fenced")
                        elif entry == "touched":
                            self.h.session("allowed", "s", self.write_tool(target + "/c.md"))
                            self.assertEqual(self.c.claude_touched(m, self.h.projects), {})
                        else:
                            command = (["similar", "--cwd", target, "--query", "q"] if entry == "cwd" else
                                       ["record", "--" + entry, target + ("/l.jsonl" if entry == "log" else "/c.md"),
                                        "--kind", "decided", "--category", "scope", "--tier", "high",
                                        "--question", "q", "--ada-pick", "keep"])
                            if entry == "log":
                                command.extend(["--work", str(base / "allowed")])
                            with redirect_stderr(io.StringIO()), redirect_stdout(io.StringIO()):
                                self.assertEqual(self.c.main(["--member", self.member, *command]), 4)
        self.assertEqual(list(fenced.iterdir()), [])

    def test_parent_at_root_stays_at_root(self):
        self.assertEqual(self.c.walk_resolve(Path("/../.."), ()), (Path("/"), False))

    def test_fence_parent_link_and_link_hop_limit(self):
        base = self.h.root.resolve()
        real = base / "real"
        real.mkdir()
        alias = base / "alias"
        alias.symlink_to(real, target_is_directory=True)
        self.config["fenced"] = [str(alias / "private")]
        m = self.save_config()
        with patch.object(os, "lstat", wraps=os.lstat) as calls:
            self.assertEqual(self.c.path_compartment(m.compartments, real / "private/c.md", self.h.root), "fenced")
            self.assertFalse(any(self.c.under(Path(call.args[0]), real / "private") or
                                 self.c.under(Path(call.args[0]), alias / "private")
                                 for call in calls.call_args_list))
        for i in range(42):
            (base / f"link-{i}").symlink_to(f"link-{i + 1}" if i < 41 else "missing")
        self.assertEqual(self.c.path_compartment(m.compartments, base / "link-0/c.md", self.h.root), "fenced")
        # Exactly 40 hops are permitted; missing suffixes are appended lexically.
        self.assertEqual(self.c.safe_resolve(m.compartments, base / "link-2/c.md", self.h.root),
                         (base / "missing/c.md", False))

    def test_explicit_fenced_paths_refused_before_io(self):
        fenced = self.h.root / "fenced"
        self.config["fenced"] = [str(fenced)]
        self.save_config()
        for command in (("record", "--log", str(fenced / "live.jsonl")),
                        ("similar", "--corpus", str(fenced / "c.jsonl"), "--query", "q")):
            proc = self.h.crew(self.member, *command)
            self.assertEqual(proc.returncode, 4, proc.stderr)
        self.assertFalse(fenced.exists())
        with patch.object(self.c, "read_jsonl", side_effect=AssertionError("read fenced corpus")):
            self.assertEqual(self.c.main(["--member", self.member, "similar", "--query", "q",
                                         "--corpus", str(fenced / "c.jsonl")]), 4)

    def test_parser_path_dest_inventory(self):
        import argparse
        expected = {"member", "cwd", "work", "projects", "queues", "corpus", "answers", "labels", "predictions",
                    "log", "out", "seen", "dir", "out_dir", "questions", "export", "review", "inputs",
                    "rounds", "train", "pdftotext", "chunk", "doc", "corpora", "file", "block_list", "from_file"}
        found = set()
        def visit(parser):
            for action in parser._actions:
                if isinstance(action, argparse._SubParsersAction):
                    for sub in action.choices.values():
                        visit(sub)
                elif action.dest in expected:
                    found.add(action.dest)
        visit(self.c.parser_for(None, "test"))
        self.assertEqual(found, expected)
        self.assertEqual(set(self.c.PATH_ARGUMENTS), found)

    def test_bad_compartments_recursion_unicode_and_controls(self):
        for data in (b"[" * 2000, b"\xff"):
            (self.h.root / "compartments.json").write_bytes(data)
            proc = self.h.crew(self.member, "where")
            self.assertEqual(proc.returncode, 2, proc.stderr)
            self.assertEqual(len(proc.stderr.splitlines()), 1)
        for bad in ("\x00", "\n"):
            for location in ("fenced", "folder", "hub", "route"):
                c = json.loads(json.dumps(self.config))
                if location == "fenced": c["fenced"] = ["~/bad" + bad]
                if location == "folder": c["compartments"]["work"]["folders"] = ["~/bad" + bad]
                if location == "hub": c["hub"]["paths"] = ["~/bad" + bad]
                if location == "route": c["hub"]["routes"] = {"bad" + bad: "work"}
                self.h.write("compartments.json", c)
                with self.subTest(location=location, bad=bad):
                    proc = self.h.crew(self.member, "where")
                    self.assertEqual(proc.returncode, 2, proc.stderr)
                    self.assertEqual(len(proc.stderr.splitlines()), 1)

    def test_member_and_private_bad_encoding_or_nesting(self):
        for name in ("member.json", "private.json"):
            for data in (b"[" * 10000 + b"0" + b"]" * 10000, b"\xff"):
                with self.subTest(name=name, data=data[:1]):
                    self.h.member(ADA)
                    path = self.h.root / name
                    path.write_bytes(data)
                    errors = io.StringIO()
                    with redirect_stderr(errors):
                        self.assertEqual(self.c.main(["--member", self.member, "where"]), 2)
                    self.assertEqual(len(errors.getvalue().splitlines()), 1)
                    self.assertIn("member config:", errors.getvalue())
                    path.unlink()

    def test_touched_no_config_base_parity(self):
        m = self.c.load_member(ADA)
        targets = [self.h.root / "excluded.md", self.h.root / "top.md"]
        self.h.session("-private-excluded", "s", self.write_tool(targets[0]))
        (self.h.projects / "top.jsonl").write_text(json.dumps(self.write_tool(targets[1])) + "\n")
        expected = {key: "wrote" for target in targets
                    for key in (os.path.normpath(target), os.path.realpath(target))}
        self.assertEqual(self.c.claude_touched(m, self.h.projects), expected)

    def test_all_cwd_sources_enforce_fence(self):
        from contextlib import redirect_stdout, redirect_stderr
        import io
        m = self.save_config()
        commands = (("where", "--cwd", "~/projects/work"),
                    ("similar", "--cwd", "~/projects/work", "--compartment", "all", "--query", "q"),
                    ("record", "--cwd", "~/projects/work", "--work", "x", "--kind", "decided",
                     "--category", "scope", "--tier", "high", "--question", "q", "--ada-pick", "keep"))
        for env, cwd in (("~/garden-club", str(self.h.root)), ("~/projects/work", str(TEST_HOME / "garden-club"))):
            for command in commands:
                with self.subTest(command=command, env=env, cwd=cwd), patch.dict(os.environ, {"CLAUDE_PROJECT_DIR": env}), \
                     patch.object(os, "getcwd", return_value=cwd), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                    self.assertEqual(self.c.main(list(command), member={**ADA, "compartments": self.config}), 4)

    def fenced_source_fixture(self):
        fence = self.h.root / "fence"
        fence.mkdir()
        (fence / "mail.mbox").write_text("From sender\n")
        (fence / "live.jsonl").write_text("")
        (fence / "conversations.json").write_text("[]")
        link = self.h.root / "link"
        link.symlink_to(fence, target_is_directory=True)
        self.config["fenced"] = [str(fence)]
        self.save_config()
        return fence, (fence, link, fence.with_name("FENCE"))

    def trace_fence(self, fence):
        stack = ExitStack()
        roots = (fence, fence.resolve())
        for owner, name in ((os, "lstat"), (os, "stat"), (os, "readlink"), (os, "scandir"),
                            (os, "open"), (builtins, "open")):
            original = getattr(owner, name)
            def guarded(path, *args, _original=original, **kwargs):
                if isinstance(path, (str, bytes, os.PathLike)):
                    self.assertFalse(any(self.c.under(Path(os.fsdecode(path)), root) for root in roots),
                                     "touched fence")
                return _original(path, *args, **kwargs)
            stack.enter_context(patch.object(owner, name, guarded))
        return stack

    def test_source_openers_refuse_before_io(self):
        fence, paths = self.fenced_source_fixture()
        m = self.c.load_member(self.c.read_member(self.member))
        for root in paths:
            for kind, filename in (("mbox", "mail.mbox"), ("live", "live.jsonl")):
                source = {"name": "fixture", "kind": kind, "path": str(root / filename),
                          "own_addresses": ["sam@example.com"]}
                with self.subTest(root=root, kind=kind), self.trace_fence(fence), \
                        patch.object(os.path, "lexists", side_effect=AssertionError("lexists before refusal")), \
                        patch.object(os, "open", side_effect=AssertionError("opened source")):
                    with self.assertRaises(self.c.Fenced):
                        {"mbox": self.c.mine_mbox, "live": self.c.mine_live}[kind](m, source)

    def test_source_openers_cli_exit_four(self):
        fence, paths = self.fenced_source_fixture()
        for root in paths:
            for kind, filename in (("mbox", "mail.mbox"), ("live", "live.jsonl")):
                self.h.write("private.json", {"sources": [{"name": "fixture", "kind": kind,
                    "path": str(root / filename), "consent": "2026-11-01",
                    **({"own_addresses": ["sam@example.com"]} if kind == "mbox" else {})}]})
                with self.subTest(root=root, kind=kind):
                    proc = self.h.crew(self.member, "sources", "mine", "--name", "fixture")
                    self.assertEqual(proc.returncode, 4, proc.stderr)
                    self.assertEqual(proc.stderr, "crew_decisions: refusing: fenced\n")

    @unittest.skipUnless(sandbox_usable(), "sandbox-exec cannot apply a profile here")
    def test_source_openers_kernel_tripwire(self):
        fence, paths = self.fenced_source_fixture()
        for root in paths:
            for kind, filename in (("mbox", "mail.mbox"), ("live", "live.jsonl")):
                self.h.write("private.json", {"sources": [{"name": "fixture", "kind": kind,
                    "path": str(root / filename), "consent": "2026-11-01",
                    **({"own_addresses": ["sam@example.com"]} if kind == "mbox" else {})}]})
                with self.subTest(root=root, kind=kind):
                    proc = tripwire_crew(self.h, self.member, [fence], "sources", "mine", "--name", "fixture")
                    self.assertEqual(proc.returncode, 4, proc.stderr)

    def test_export_opener_refuses_before_io(self):
        fence, paths = self.fenced_source_fixture()
        m = self.c.load_member(self.c.read_member(self.member))
        for root in paths:
            with self.subTest(root=root), self.trace_fence(fence), \
                    patch.object(Path, "is_dir", side_effect=AssertionError("stat export")):
                with self.assertRaises(self.c.Fenced):
                    self.c.cmd_claudeai_inspect(m, self.c.argparse.Namespace(export=str(root / "conversations.json")))

    def test_default_state_paths_refuse_before_commands(self):
        fence, paths = self.fenced_source_fixture()
        for root in paths:
            for command in (("record",), ("similar", "--query", "q"), ("pending",)):
                with self.subTest(root=root, command=command), self.trace_fence(fence), \
                        patch.dict(self.c.COMMANDS, {command[0]: lambda *args: self.fail("command ran")}):
                    with redirect_stderr(io.StringIO()):
                        rc = self.c.main(list(command), member={**ADA, "state_dir": str(root),
                                                               "compartments": self.config})
                    self.assertEqual(rc, 4)

    @unittest.skipUnless(sandbox_usable(), "sandbox-exec cannot apply a profile here")
    def test_default_state_paths_kernel_tripwire(self):
        fence, paths = self.fenced_source_fixture()
        for root in paths:
            member = self.h.member({**ADA, "state_dir": str(root), "compartments": "compartments.json"})
            for command in (("record",), ("similar", "--query", "q"), ("pending",)):
                with self.subTest(root=root, command=command):
                    proc = tripwire_crew(self.h, member, [fence], *command)
                    self.assertEqual(proc.returncode, 4, proc.stderr)

    def test_hub_children_and_resolutions_cached_per_member(self):
        hub = self.h.root / "hub"
        hub.mkdir()
        (hub / "garden").mkdir()
        (hub / "file").write_text("fixture")
        fence = self.h.root / "fence"
        fence.mkdir()
        (hub / "linked").symlink_to(fence)
        self.config["fenced"] = [str(fence)]
        self.config["hub"]["paths"] = [str(hub)]
        m = self.c.load_member({**ADA, "compartments": self.config})
        with patch.object(self.c, "safe_resolve", wraps=self.c.safe_resolve) as resolve, \
                patch.object(os, "scandir", wraps=os.scandir) as scan:
            with patch.object(self.c, "safe_resolve", side_effect=AssertionError("route key resolved")):
                self.assertEqual(self.c.hub_project_compartment(m.compartments, hub, "CLIENT-STACK"), "client")
            for _ in range(2):
                for name, expected in (("garden", "p:garden"), ("linked", None), ("file", None), ("absent", None)):
                    self.assertEqual(self.c.hub_project_compartment(m.compartments, hub, name), expected)
            self.assertEqual(scan.call_count, 1)
            self.assertEqual([call.args[1] for call in resolve.call_args_list],
                             [hub, hub / "garden", hub / "linked"])
        other = self.c.load_member({**ADA, "compartments": self.config})
        with patch.object(os, "scandir", wraps=os.scandir) as scan:
            self.assertEqual(self.c.hub_project_compartment(other.compartments, hub, "garden"), "p:garden")
            self.assertEqual(scan.call_count, 1)

    def test_folded_fence_comparisons_prepared_once(self):
        m = self.c.load_member({**ADA, "compartments": self.config})
        lexical = self.c.lexical_fences(m.compartments, self.h.root)
        resolved = self.c.fenced_forms(m.compartments, self.h.root)
        original = self.c.encode
        def encode(path):
            self.assertNotIn(Path(path), (*lexical, *resolved), "re-encoded a fence root")
            return original(path)
        with patch.object(self.c, "encode", side_effect=encode):
            for roots in (lexical, resolved):
                for _ in range(2):
                    self.assertTrue(self.c.matches_fence(roots[0] / "child", roots))
                    self.assertFalse(self.c.matches_fence(self.h.root / "allowed", roots))

    def test_hub_distinct_megabyte_names_with_two_fences(self):
        self.config["fenced"].append("~/other-fence")
        m = self.c.load_member({**ADA, "compartments": self.config})
        value = " ".join("~/.hub/projects/unknown-%05d/file" % i for i in range(30000))
        self.assertGreater(len(value), 1024 * 1024)
        def deadline(signum, frame):
            self.fail("distinct hub routing exceeded 15 seconds of CPU time")
        old = signal.signal(signal.SIGPROF, deadline)
        try:
            # CPU time, not wall time: a busy Mac runs this on slow cores; a quadratic matcher takes minutes.
            signal.setitimer(signal.ITIMER_PROF, 15)
            start = time.process_time()
            actual = self.c.hub_compartment(m.compartments, ENCODED_HOME + "--hub", "work", [self.write_tool(value)])
            elapsed = time.process_time() - start
            self.assertEqual(actual, "work")
            self.assertLess(elapsed, 15)
        finally:
            signal.setitimer(signal.ITIMER_PROF, 0)
            signal.signal(signal.SIGPROF, old)

    def test_fenced_root_forms_cached_per_member(self):
        m = self.save_config()
        with patch.object(os, "lstat", wraps=os.lstat) as calls:
            first = self.c.fenced_forms(m.compartments, TEST_HOME)
            count = calls.call_count
            self.assertGreater(count, 0)
            for _ in range(3):
                self.assertIs(self.c.fenced_forms(m.compartments, TEST_HOME), first)
            self.assertEqual(calls.call_count, count)


class Core(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        sys.path.insert(0, str(SCRIPTS))
        import crew_decisions
        self.c = crew_decisions
        self.agent_vars = ("CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT", "CODEX_SANDBOX", "CODEX_THREAD_ID")
        clean_env = {key: value for key, value in os.environ.items() if key not in self.agent_vars}
        self.env_patch = patch.dict(os.environ, clean_env, clear=True)
        self.env_patch.start()
        self.addCleanup(self.env_patch.stop)
        self.overlay = self.h.root.resolve() / "state/core/patterns.md"
        self.config = {"version": 1, "fenced": [str(self.h.root / "fence")],
                       "compartments": {"client": {"folders": []}},
                       "core": {"files": ["a.md", "b.md"],
                                "markers": {"client": ["clientco", "client-stack"], "p:garden": ["garden site"]},
                                "patterns": [r"slack\.com/archives"], "overlay": str(self.overlay)}}
        self.member = self.h.member({**ADA, "compartments": "compartments.json"})
        self.save()
        for name in ("a.md", "b.md", "input.md"):
            (self.h.root / name).write_text("Keep one source.\n")

    def save(self):
        self.h.write("compartments.json", self.config)

    def test_core_config_validation(self):
        for key, values in {"core": [None, [], {"unknown": []}],
                            "files": [None, [], "a.md", [""], [1], ["/a"], ["~a"], ["a/../b"], ["a\0"], ["a\nb"]],
                            "markers": [None, [], {1: ["x"]}, {"unknown": ["x"]}, {"p:": ["x"]}, {"client": []},
                                        {"client": [""]}, {"client": [1]}, {"client": ["x\ny"]}],
                            "patterns": [None, "x", [1], ["["]],
                            "overlay": [None, "relative", "~other/a", "/a\0", "/a\nb"]}.items():
            for value in values:
                c = json.loads(json.dumps(self.config))
                if key == "core": c[key] = value
                else: c["core"][key] = value
                with self.subTest(key=key, value=value), self.assertRaisesRegex(
                        self.c.CompartmentConfigError, "invalid " + (key if key == "core" else "core." + key)):
                    self.c.check_compartments(c)
        loaded = self.c.read_compartments("compartments.json", self.h.root)
        self.assertEqual(loaded, self.config)
        self.assertEqual(loaded.base, self.h.root)
        self.assertEqual(self.c.check_compartments({**self.config, "core": {}})["core"], {})

    def test_core_validation_edge_cases(self):
        cases = {"markers": [{"client": [term]} for term in (" ", "\t", "x\0y", *(
                    "x" + separator + "y" for separator in "\r\n\u2028\u2029\x0b\x0c\x1c\x1d\x1e\x85"), "x\r", *("x" + char + "y" for char in
                    "\t\x1b\x7f\u200b\u200c\u200d\u2060\ufeff"))],
                 "patterns": [[pattern] for pattern in ("", "a*", "^", "(?:x)?")],
                 "files": [["."], ["a/"], ["a//b"], ["a", "./a"], ["a/./b", "a/b"]],
                 "overlay": ["/", "~/", "/tmp/core/"]}
        for key, values in cases.items():
            for value in values:
                config = {**self.config, "core": {**self.config["core"], key: value}}
                with self.subTest(key=key, value=value), self.assertRaisesRegex(
                        self.c.CompartmentConfigError, "invalid core." + key):
                    self.c.check_compartments(config)

    def test_core_check_clean_override_overlay_and_missing(self):
        proc = self.h.crew(self.member, "core-check")
        self.assertEqual((proc.returncode, proc.stdout), (0, "core-check: clean, 2 files\n"), proc.stderr)
        self.overlay.parent.mkdir(parents=True)
        self.overlay.write_text("ClientCo.\n")
        proc = self.h.crew(self.member, "core-check")
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(proc.stdout, f"{self.overlay}:1: marker client\n")
        proc = self.h.crew(self.member, "core-check", "--file", "b.md", "--file", "input.md")
        self.assertEqual((proc.returncode, proc.stdout), (0, "core-check: clean, 2 files\n"), proc.stderr)
        (self.h.root / "a.md").unlink()
        proc = self.h.crew(self.member, "core-check")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("a.md", proc.stderr)

    def test_core_check_hits_are_sorted_and_redacted(self):
        (self.h.root / "a.md").write_text("clientcoast\nClientCo.\nclient-stack/\nSLACK.com/archives/x\nmy therapist\n123456789\nquiet-codename\ngarden site\n")
        (self.h.root / "b.md").write_text("clientco client-stack\n")
        (self.h.root / "blocks").write_text("\n# ignored\nquiet-codename\n")
        proc = self.h.crew(self.member, "core-check", "--file", "b.md", "--file", "a.md", "--block-list", "blocks")
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(proc.stdout.splitlines(), [f"a.md:{n}: {kind}" for n, kind in
                         [(2, "marker client"), (3, "marker client"), (4, "pattern 0"), (5, "personal"),
                          (6, "secret"), (7, "block-list"), (8, "marker p:garden")]] + ["b.md:1: marker client"])
        self.assertIn("core-check: 8 hits in 2 files", proc.stderr)
        for term in ("clientco", "client-stack", "quiet-codename", "my therapist", "123456789", "garden site"):
            self.assertNotIn(term, (proc.stdout + proc.stderr).lower())

    def test_core_requires_config_and_base(self):
        del self.config["core"]
        self.save()
        proc = self.h.crew(self.member, "core-check")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("core-check: no core in compartments.json", proc.stderr)
        self.config["core"] = {"files": ["a.md"]}
        with redirect_stderr(io.StringIO()) as err:
            self.assertEqual(self.c.main(["core-check"], member={**ADA, "compartments": self.config}), 1)
        self.assertIn("base", err.getvalue())

    def test_core_fences_refused_before_open(self):
        fence = self.h.root / "fence"
        fence.mkdir()
        target = fence / "blocked"
        target.write_text("private\n")
        target.chmod(0)
        link = self.h.root / "link"
        link.symlink_to(fence, target_is_directory=True)
        original_open, original_io_open, original_os_open = builtins.open, io.open, os.open
        def guard(fn):
            def call(path, *args, **kwargs):
                if isinstance(path, (str, bytes, os.PathLike)):
                    self.assertFalse(self.c.under(Path(os.fsdecode(path)), fence), "opened fence")
                return fn(path, *args, **kwargs)
            return call
        for path in (target, link / "blocked"):
            for args in (("core-check", "--file", str(path)),
                         ("core-check", "--block-list", str(path))):
                with self.subTest(args=args), patch.object(builtins, "open", guard(original_open)), \
                        patch.object(io, "open", guard(original_io_open)), \
                        patch.object(os, "open", guard(original_os_open)), redirect_stderr(io.StringIO()) as err:
                    self.assertNotEqual(self.c.main(["--member", self.member, *args]), 0)
                self.assertIn("fenced", err.getvalue())

    def terminal(self, *args, answer="y", stdin_tty=True, stdout_tty=True):
        previous = Path.cwd()
        try:
            os.chdir(self.h.root)
            with patch.object(os, "isatty", side_effect=lambda fd: stdin_tty if fd == 0 else stdout_tty), \
                    patch.object(self.c, "confirm_import", return_value=answer.strip().lower() in ("y", "yes")), \
                    patch.object(sys, "stdin", io.StringIO("y\n")), \
                    redirect_stdout(io.StringIO()) as out, redirect_stderr(io.StringIO()) as err:
                code = self.c.main(["--member", self.member, "core-import", *args])
            return code, out.getvalue(), err.getvalue()
        finally:
            os.chdir(previous)

    def test_core_import_fenced_block_list_refused_before_any_open(self):
        self.assert_fenced_import_refused("--block-list")

    def test_core_block_list_ignores_indented_comments(self):
        (self.h.root / "blocks").write_text("  # ignored\n\t# also ignored\n")
        (self.h.root / "input.md").write_text("# ignored\n# also ignored\n")
        proc = self.h.crew(self.member, "core-check", "--file", "input.md", "--block-list", "blocks")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        code, out, err = self.terminal("--from", "input.md", "--block-list", "blocks")
        self.assertEqual(code, 0, err)
        self.assertEqual(self.overlay.read_text(), "# ignored\n# also ignored\n")

    def test_core_non_utf8_inputs_fail_cleanly_without_writes(self):
        for name, command, label in (("input.md", "core-import", "--from"),
                                      ("blocks", "core-import", "--block-list"),
                                      ("blocks", "core-check", "--block-list"),
                                      ("a.md", "core-check", "core file")):
            with self.subTest(command=command, label=label):
                path = self.h.root / name
                path.write_bytes(b"private-\xff")
                if command == "core-import":
                    extra = ("--block-list", "blocks") if label == "--block-list" else ()
                    code, out, err = self.terminal("--from", "input.md", *extra)
                else:
                    extra = ("--block-list", "blocks") if label == "--block-list" else ()
                    proc = self.h.crew(self.member, command, *extra)
                    code, out, err = proc.returncode, proc.stdout, proc.stderr
                self.assertEqual(code, 1)
                self.assertIn(f"{command}: {label} is not UTF-8 text", err)
                self.assertNotIn("Traceback", out + err)
                self.assertNotIn("private-", out + err)
                self.assertIn(out, ("", "y\n") if command == "core-import" else ("",))
                self.assertFalse(self.overlay.parent.exists())
                path.write_text("Keep one source.\n")

    def test_core_non_utf8_overlay_stays_unchanged(self):
        self.overlay.parent.mkdir(parents=True)
        self.overlay.write_bytes(b"private-\xff")
        for command in ("core-check", "core-import"):
            with self.subTest(command=command):
                if command == "core-import":
                    code, out, err = self.terminal("--from", "input.md")
                else:
                    proc = self.h.crew(self.member, command)
                    code, out, err = proc.returncode, proc.stdout, proc.stderr
                self.assertEqual(code, 1)
                self.assertIn(f"{command}: core file is not UTF-8 text", err)
                self.assertNotIn("Traceback", out + err)
                self.assertNotIn("private-", out + err)
                self.assertEqual(self.overlay.read_bytes(), b"private-\xff")
                self.assertEqual(list(self.overlay.parent.iterdir()), [self.overlay])

    def test_core_import_requires_ttys_and_overlay(self):
        for stdin_tty, stdout_tty in ((False, True), (True, False)):
            code, out, err = self.terminal("--from", "input.md", stdin_tty=stdin_tty, stdout_tty=stdout_tty)
            self.assertEqual(code, 1)
            self.assertIn("core-import: run this yourself in a terminal", err)
            self.assertFalse(self.overlay.exists())
        del self.config["core"]["overlay"]
        self.save()
        code, out, err = self.terminal("--from", "input.md")
        self.assertEqual(code, 1)
        self.assertIn("overlay", err)

    def test_core_import_hits_never_write(self):
        for text, kind in (("ClientCo.", "marker client"), ("slack.com/archives", "pattern 0"),
                           ("my therapist", "personal"), ("123456789", "secret"), ("quiet-codename", "block-list")):
            (self.h.root / "input.md").write_text(text + "\n")
            (self.h.root / "blocks").write_text("quiet-codename\n")
            code, out, err = self.terminal("--from", "input.md", "--block-list", "blocks")
            self.assertEqual(code, 1)
            self.assertIn("input.md:1: " + kind, out)
            self.assertNotIn(text, out + err)
            self.assertFalse(self.overlay.exists())

    def test_core_import_confirmation_metadata_and_modes(self):
        source = self.h.root / "chosen.md"
        planted = "unique-import-content-uvwx\n"
        old = "unique-overlay-content-abcd\n"
        source.write_text(planted)
        self.overlay.parent.mkdir(parents=True)
        self.overlay.parent.chmod(0o755)
        self.overlay.write_text(old)
        for answer, expected in (("n", 1), ("y", 0)):
            with self.subTest(answer=answer):
                code, out, err = self.terminal("--from", str(source), answer=answer)
                self.assertEqual(code, expected, err)
                self.assertNotIn(planted.strip(), out + err)
                self.assertNotIn(old.strip(), out + err)
                self.assertIn(f"source: 1 lines, {len(planted.encode())} bytes, sha256 {hashlib.sha256(planted.encode()).hexdigest()[:12]}", out)
                self.assertIn(f"overlay: 1 lines, sha256 {hashlib.sha256(old.encode()).hexdigest()[:12]}", out)
                self.assertIn(str(self.overlay), out)
                self.assertNotIn("[y/N]", out)
                if expected:
                    self.assertIn("core-import: not imported", err)
                    self.assertEqual(self.overlay.read_text(), old)
                    self.assertEqual(self.overlay.parent.stat().st_mode & 0o777, 0o755)
        self.assertEqual(self.overlay.read_text(), planted)
        self.assertEqual(self.overlay.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.overlay.parent.stat().st_mode & 0o777, 0o700)
        self.assertEqual(list(self.overlay.parent.iterdir()), [self.overlay])

    def test_core_import_dialog_text_and_absent_overlay(self):
        source = self.h.root / "input.md"
        source.write_text("Keep one source.\nKeep it local.\n")
        with patch.object(os, "isatty", return_value=True), \
                patch.object(self.c, "confirm_import", return_value=False) as confirm, \
                patch.object(sys, "stdin") as stdin, \
                redirect_stdout(io.StringIO()) as out, redirect_stderr(io.StringIO()) as err:
            self.assertEqual(self.c.main(["--member", self.member, "core-import", "--from", str(source)]), 1)
        confirm.assert_called_once_with(f"Import 2 lines into {self.overlay}?")
        stdin.readline.assert_not_called()
        self.assertIn("overlay: none\n", out.getvalue())
        self.assertIn("core-import: not imported", err.getvalue())
        self.assertNotIn("Keep one source.", out.getvalue() + err.getvalue())
        self.assertNotIn("Keep it local.", out.getvalue() + err.getvalue())
        self.assertFalse(self.overlay.exists())

    def test_core_import_agent_pty_refused_before_any_open(self):
        for variable in self.agent_vars:
            with self.subTest(variable=variable):
                with tempfile.TemporaryFile() as errors:
                    pid, master = pty.fork()
                    if pid == 0:
                        try:
                            os.environ[variable] = "1"
                            assert os.isatty(0) and os.isatty(1)
                            with patch.object(os, "open", side_effect=AssertionError("opened before refusal")), \
                                    patch.object(builtins, "open", side_effect=AssertionError("opened config")), \
                                    patch.object(io, "open", side_effect=AssertionError("opened content")), \
                                    redirect_stderr(io.StringIO()) as err:
                                code = self.c.main(["--member", self.member, "core-import", "--from", "input.md"])
                            os.write(errors.fileno(), err.getvalue().encode())
                            os._exit(code)
                        except BaseException:
                            os._exit(99)
                    try:
                        try:
                            os.write(master, b"y\n")
                        except OSError as error:
                            if error.errno != errno.EIO:
                                raise
                        _, status = os.waitpid(pid, 0)
                        self.assertEqual(os.waitstatus_to_exitcode(status), 1)
                        errors.seek(0)
                        self.assertIn("core-import: run this yourself in a terminal, not from an agent",
                                      errors.read().decode())
                    finally:
                        os.close(master)
                self.assertFalse(self.overlay.exists())

    def test_core_import_keyboard_interrupt_is_clean(self):
        for seam in ("core_import_file", "confirm_import", "load_member"):
            with self.subTest(seam=seam), patch.object(self.c, seam, side_effect=KeyboardInterrupt):
                try:
                    code, out, err = self.terminal("--from", "input.md") if seam != "confirm_import" else self.interrupted_dialog()
                except KeyboardInterrupt:
                    self.fail("core-import leaked KeyboardInterrupt")
                self.assertEqual(code, 1)
                self.assertIn("core-import: not imported", err)
                self.assertNotIn("Traceback", out + err)
                self.assertFalse(self.overlay.exists())

    def interrupted_dialog(self):
        with patch.object(os, "isatty", return_value=True), \
                patch.object(self.c, "confirm_import", side_effect=KeyboardInterrupt), \
                redirect_stdout(io.StringIO()) as out, redirect_stderr(io.StringIO()) as err:
            code = self.c.main(["--member", self.member, "core-import", "--from", str(self.h.root / "input.md")])
        return code, out.getvalue(), err.getvalue()

    def test_confirm_import_dialog(self):
        self.assertTrue(callable(getattr(self.c, "confirm_import", None)), "missing macOS confirmation")
        text = 'Import 1 lines into /state/"chosen".md?'
        script = 'display dialog "Import 1 lines into /state/\\"chosen\\".md?" buttons {"Cancel", "Import"} default button "Cancel" cancel button "Cancel" with icon caution'
        fake_bin = self.h.root / "fake-bin"
        fake_bin.mkdir()
        fake = fake_bin / "osascript"
        fake.write_text("#!/bin/sh\nprintf 'button returned:Import\\n'\n")
        fake.chmod(0o755)
        trusted = os.stat_result((stat.S_IFREG | 0o755, 0, 0, 1, 0, 0, 0, 0, 0, 0))
        with patch.object(sys, "platform", "darwin"), \
                patch.object(Path, "lstat", autospec=True, return_value=trusted) as check, \
                patch.dict(os.environ, {"PATH": str(self.h.root / "fake-bin")}), \
                patch.object(subprocess, "run") as run:
            for status, output, expected in ((0, "button returned:Import\n", True), (1, "button returned:Import", False),
                                             (0, "button returned:Cancel", False), (0, "", False)):
                run.return_value = subprocess.CompletedProcess([], status, output, "")
                self.assertEqual(self.c.confirm_import(text), expected)
                check.assert_called_with(Path("/usr/bin/osascript"))
                run.assert_called_with(["/usr/bin/osascript", "-e", script], capture_output=True, text=True,
                                       timeout=600, env={"PATH": "/usr/bin:/bin", "HOME": str(TEST_HOME)})
            for error in (subprocess.TimeoutExpired("osascript", 600), OSError("failed")):
                run.side_effect = error
                self.assertFalse(self.c.confirm_import(text))
        with patch.object(sys, "platform", "linux"), patch.object(subprocess, "run") as run:
            self.assertFalse(self.c.confirm_import(text))
            run.assert_not_called()

    def test_confirm_import_refuses_untrusted_executable(self):
        for mode, uid in ((stat.S_IFREG | 0o755, 501), (stat.S_IFDIR | 0o755, 0),
                          (stat.S_IFLNK | 0o777, 0)):
            with self.subTest(mode=mode, uid=uid), patch.object(sys, "platform", "darwin"), \
                    patch.object(Path, "lstat", return_value=os.stat_result((mode, 0, 0, 1, uid, 0, 0, 0, 0, 0))), \
                    patch.object(subprocess, "run") as run:
                self.assertFalse(self.c.confirm_import("Import?"))
                run.assert_not_called()
        with patch.object(sys, "platform", "darwin"), patch.object(Path, "lstat", side_effect=OSError("missing")), \
                patch.object(subprocess, "run") as run:
            self.assertFalse(self.c.confirm_import("Import?"))
            run.assert_not_called()

    def test_core_import_overlay_stays_under_state(self):
        outside = self.h.root / "outside"
        outside.mkdir(mode=0o755)
        link = self.h.root / "state/link"
        link.parent.mkdir()
        link.symlink_to(outside, target_is_directory=True)
        for overlay in (outside / "patterns.md", link / "patterns.md", self.h.root / "state", self.h.root / "state/core"):
            if overlay.name == "core":
                overlay.mkdir()
            self.config["core"]["overlay"] = str(overlay)
            self.save()
            with self.subTest(overlay=overlay):
                code, out, err = self.terminal("--from", "input.md")
                self.assertEqual(code, 1)
                self.assertIn("overlay must be a file inside state_dir", err)
                self.assertEqual(outside.stat().st_mode & 0o777, 0o755)
                self.assertFalse((outside / "patterns.md").exists())

    def test_core_import_rejects_git_and_fenced_overlays(self):
        checkout = self.h.root / "state/checkout"
        checkout.mkdir(parents=True)
        for git_dir in (False, True):
            entry = checkout / ".git"
            if git_dir: entry.mkdir()
            else: entry.write_text("gitdir: elsewhere\n")
            self.config["core"]["overlay"] = str(checkout / "nested/patterns.md")
            self.save()
            code, out, err = self.terminal("--from", "input.md")
            self.assertEqual(code, 1)
            self.assertIn("the overlay must stay out of git and out of fences", err)
            self.assertFalse((checkout / "nested").exists())
            if git_dir: entry.rmdir()
            else: entry.unlink()
        self.config["core"]["overlay"] = str(self.h.root / "fence/patterns.md")
        self.save()
        code, out, err = self.terminal("--from", "input.md")
        self.assertEqual(code, 1)
        self.assertIn("the overlay must stay out of git and out of fences", err)
        self.assertFalse((self.h.root / "fence").exists())

    def test_core_import_single_regular_file(self):
        link = self.h.root / "link"
        link.symlink_to(self.h.root / "input.md")
        large = self.h.root / "large"
        large.write_bytes(b"x" * (256 * 1024 + 1))
        for path in (link, self.h.root, large):
            for option in ("--from", "--block-list"):
                with self.subTest(path=path.name, option=option):
                    args = ("--from", "input.md") if option == "--block-list" else ()
                    code, out, err = self.terminal(*args, option, str(path))
                    self.assertEqual(code, 1)
                    self.assertFalse(self.overlay.exists())

    def test_core_import_fenced_source_refused_before_any_open(self):
        self.assert_fenced_import_refused("--from")

    def assert_fenced_import_refused(self, option):
        fence = self.h.root / "fence"
        fence.mkdir()
        source = fence / "chosen.md"
        source.write_text("A distilled pattern.\n")
        resolved_fence = fence.resolve()
        m = self.c.load_member(self.c.read_member(self.member))
        args = [option, str(source)]
        if option == "--block-list":
            args = ["--from", str(self.h.root / "input.md"), *args]

        def guard(fn):
            def call(path, *args, **kwargs):
                if isinstance(path, (str, bytes, os.PathLike)):
                    path = Path(os.fsdecode(path))
                    self.assertFalse(self.c.under(path, fence) or self.c.under(path, resolved_fence),
                                     "touched a fenced path before refusal")
                return fn(path, *args, **kwargs)
            return call

        with ExitStack() as stack:
            for module, name in ((os, "open"), (os, "stat"), (os, "lstat"), (os, "scandir"),
                                 (os, "listdir"), (builtins, "open"), (io, "open")):
                stack.enter_context(patch.object(module, name, guard(getattr(module, name))))
            stack.enter_context(patch.object(os, "isatty", return_value=True))
            confirm = stack.enter_context(patch.object(self.c, "confirm_import", return_value=True))
            out = stack.enter_context(redirect_stdout(io.StringIO()))
            err = stack.enter_context(redirect_stderr(io.StringIO()))
            self.assertEqual(self.c.main(["--member", self.member, "core-import", *args]), 4)
            self.assertIn("fenced", err.getvalue())
            parsed = self.c.parser_for(m, "test").parse_args(["core-import", *args])
            with self.assertRaisesRegex(self.c.Failed, "fenced"):
                self.c.cmd_core_import(m, parsed)
            confirm.assert_not_called()
        self.assertEqual(out.getvalue(), "")
        self.assertFalse(self.overlay.exists())


class CompartmentFilterCommands(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.config = {"version": 1, "fenced": [], "compartments": {
            "noor": {"folders": ["~/p/noor"], "filters": {
                "personal": "zzoverride", "excluded_folders": "zzexcluded"}},
            "fleet": {"folders": ["~/p/fleet"]}}}
        self.h.write("compartments.json", self.config)
        self.member = self.h.member({**ADA, "compartments": "compartments.json", "filters": {
            **ADA["filters"], "personal": "noor|zzmember", "excluded_folders": "noor|fleet"}})
        sys.path.insert(0, str(SCRIPTS))
        import crew_decisions
        self.c = crew_decisions

    def rows(self, name):
        return [json.loads(line) for line in self.h.read(name).splitlines()]

    def ok(self, *args):
        proc = self.h.crew(self.member, *args)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return proc

    def merge_records(self, records, out="merged.jsonl"):
        (self.h.root / "input.jsonl").write_text("".join(json.dumps(r) + "\n" for r in records))
        return self.ok("merge", "--in", "input.jsonl", "--out", out)

    def record(self, name, text, **extra):
        return {"id": name, "kind": "message", "timestamp": "2026-09-01T00:00:00Z",
                "project": "p-noor", "text": text, **extra}

    def test_merge_uses_record_compartment_and_always_drops_secrets(self):
        records = [self.record("own", "noor review", compartment="noor"),
                   self.record("unscoped", "noor review"),
                   self.record("other", "noor review", compartment="fleet"),
                   self.record("unknown", "noor review", compartment="unknown"),
                   self.record("invalid", "noor review", compartment=[]),
                   self.record("override", "zzoverride", compartment="noor")]
        records += [self.record("secret-" + str(i), "account 123456789", **extra)
                    for i, extra in enumerate(({}, {"compartment": "noor"}, {"compartment": "fleet"}))]
        proc = self.merge_records(records)
        self.assertEqual([r["id"] for r in self.rows("merged.jsonl")], ["own"])
        self.assertIn("dropped 5 personal records, 3 secret records", proc.stdout)
        config = json.loads(Path(self.member).read_text())
        del config["compartments"]
        self.h.write("member.json", config)
        self.merge_records(records[:1], out="legacy.jsonl")
        self.assertEqual(self.rows("legacy.jsonl"), [])

    def test_private_personal_addition_reaches_mine_override(self):
        self.h.session(self.c.encode("~/p/noor"), "own",
                       said("2026-09-01T00:00:00Z", "noor zzprivate", cwd="~/p/noor"),
                       said("2026-09-01T00:01:00Z", "noor zzoverride", cwd="~/p/noor"),
                       said("2026-09-01T00:02:00Z", "noor keep this queue", cwd="~/p/noor"))
        self.ok("mine", "--projects", str(self.h.projects), "--out", "baseline.jsonl")
        self.assertEqual([r["text"] for r in self.rows("baseline.jsonl")],
                         ["noor zzprivate", "noor keep this queue"])
        self.h.write("private.json", {"filters": {"personal": "zzprivate"}})
        self.ok("mine", "--projects", str(self.h.projects), "--out", "private.jsonl")
        self.assertEqual([r["text"] for r in self.rows("private.jsonl")], ["noor keep this queue"])

    def test_private_personal_addition_reaches_merge_override(self):
        records = [self.record("private", "noor zzprivate", compartment="noor"),
                   self.record("override", "zzoverride", compartment="noor"),
                   self.record("safe", "noor keep this queue", compartment="noor")]
        self.merge_records(records, out="baseline.jsonl")
        self.assertEqual([r["id"] for r in self.rows("baseline.jsonl")], ["private", "safe"])
        self.h.write("private.json", {"filters": {"personal": "zzprivate"}})
        self.merge_records(records)
        self.assertEqual([r["id"] for r in self.rows("merged.jsonl")], ["safe"])

    def test_private_excluded_addition_reaches_mine_override(self):
        for suffix in ("", "/zzprivate", "/zzexcluded"):
            self.h.session(self.c.encode("~/p/noor" + suffix), "own" + suffix.replace("/", "-"),
                           said("2026-09-01T00:00:00Z", "noor keep this queue", cwd="~/p/noor"))
        self.ok("mine", "--projects", str(self.h.projects), "--out", "baseline.jsonl")
        self.assertEqual([r["project"] for r in self.rows("baseline.jsonl")], ["p-noor", "p-noor-zzprivate"])
        self.h.write("private.json", {"filters": {"excluded_folders": "zzprivate"}})
        self.ok("mine", "--projects", str(self.h.projects), "--out", "private.jsonl")
        self.assertEqual([r["project"] for r in self.rows("private.jsonl")], ["p-noor"])

    def test_folder_compartment_distinguishes_siblings_from_descendants(self):
        home = self.h.root / "home"
        root = home / "p/noor"
        (root / "sub").mkdir(parents=True)
        (home / "p/noor-x").mkdir()
        (home / "p/noor-note").touch()
        (home / "p/noor-link").symlink_to(root / "sub", target_is_directory=True)
        with patch.dict(os.environ, {"HOME": str(home)}):
            compartments = self.c.read_compartments("compartments.json", self.h.root)
            for suffix, expected in (("", "noor"), ("/sub", "noor"), ("-x", "p:p-noor-x"),
                                     ("-x/sub", "p:p-noor-x-sub"), ("-note", "noor"),
                                     ("-link", "noor"), ("/.claude/worktrees/a", "noor"),
                                     ("-x/.claude/worktrees/a", "p:p-noor-x")):
                with self.subTest(suffix=suffix):
                    self.assertEqual(self.c.folder_compartment(compartments, self.c.encode(str(root) + suffix)),
                                     expected)
            compartments["compartments"]["fleet"]["folders"] = ["~/p"]
            self.assertEqual(self.c.folder_compartment(compartments, self.c.encode(home / "p/noor-x")), "fleet")

    def test_folder_compartment_skips_unreadable_parent(self):
        with patch.object(os, "scandir", side_effect=PermissionError):
            compartments = self.c.read_compartments("compartments.json", self.h.root)
            self.assertEqual(self.c.folder_compartment(compartments, self.c.encode("~/p/noor/sub")), "noor")

    def test_folders_text_and_json_use_the_same_exclusion_as_mine(self):
        noor = self.c.encode("~/p/noor")
        fleet = self.c.encode("~/p/fleet")
        self.h.session(noor, "own", said("2026-09-01T00:00:00Z", "noor keep this queue", cwd="~/p/noor"))
        self.h.session(fleet, "other", said("2026-09-01T00:00:00Z", "keep this queue", cwd="~/p/fleet"))
        self.ok("mine", "--projects", str(self.h.projects), "--out", "mined.jsonl")
        self.assertEqual([r["compartment"] for r in self.rows("mined.jsonl")], ["noor"])
        for private, noor_state in ((False, "mined"), (True, "excluded")):
            with self.subTest(private=private):
                if private:
                    self.h.write("private.json", {"filters": {"excluded_folders": "noor"}})
                text = self.ok("folders", "--projects", str(self.h.projects)).stdout
                self.assertIn(f"{noor_state}\t1\tp-noor\t{noor}\n", text)
                self.assertIn(f"excluded\t1\tp-fleet\t{fleet}\n", text)
                rows = json.loads(self.ok("folders", "--projects", str(self.h.projects), "--json").stdout)
                self.assertEqual({r["compartment"]: r["state"] for r in rows},
                                 {"noor": noor_state, "fleet": "excluded"})

    def test_round_keeps_member_filter_with_compartment_override_loaded(self):
        self.h.write("questions.json", {"questions": [
            {"n": n, "question": "Which review order?", "options": [{"label": "Oldest"}, {"label": "Newest"}]}
            for n in (1, 2)]})
        self.h.write("answers.json", {"answered": "2026-09-01", "answers": {
            "1": {"own_words": "noor review"}, "2": {"own_words": "keep one queue"}}})
        proc = self.ok("round", "--name", "sample", "--questions", "questions.json",
                       "--answers", "answers.json", "--out", "round.jsonl")
        self.assertEqual([r["own_words"] for r in self.rows("round.jsonl")], ["keep one queue"])
        self.assertEqual(self.rows("round.jsonl")[0]["project"], "round-sample")
        self.assertNotIn("compartment", self.rows("round.jsonl")[0])
        self.assertIn("dropped 1 personal records, 0 secret records", proc.stdout)


if __name__ == "__main__":
    unittest.main()

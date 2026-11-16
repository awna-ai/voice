mod common;
use common::*;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard},
};
use voice_agent::{
    claude::Claude,
    guard::Guard,
    ledger::{Cap, Session},
};
use voice_core::{ceilings::Ceilings, error::Result};
use voice_grow::{
    apply,
    backtest::{self, Backtested},
    diff,
    keep::keep,
    record::*,
    refresh,
    store::Store,
};
use voice_ingest::{filters::Filters, private::write_private_at};

static SUBPROCESS_TEST: Mutex<()> = Mutex::new(());
const HEADING: &str = "## Calibration (the blind backtest)";

struct Setup {
    f: Fixture,
    dir: PathBuf,
    stub: PathBuf,
    _serial: MutexGuard<'static, ()>,
}
impl Setup {
    fn new() -> Self {
        let serial = SUBPROCESS_TEST.lock().unwrap_or_else(|p| p.into_inner());
        let f = Fixture::new();
        let dir = f.home.join("member");
        fs::create_dir(&dir).unwrap();
        fs::write(
            dir.join("decisions.md"),
            voice_build::draft::filled_template(&f.member),
        )
        .unwrap();
        let stub = f.home.join("stub");
        fs::create_dir(&stub).unwrap();
        fs::create_dir(stub.join("profile")).unwrap();
        fs::set_permissions(stub.join("profile"), fs::Permissions::from_mode(0o700)).unwrap();
        let python = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        fs::write(
            stub.join("claude"),
            format!(
                "#!{} -I\n{}",
                String::from_utf8(python.stdout).unwrap().trim(),
                include_str!("fixtures/backtest_stub.py")
            ),
        )
        .unwrap();
        fs::set_permissions(stub.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            f,
            dir,
            stub,
            _serial: serial,
        }
    }
    fn store(&self) -> Store {
        Store::open(&self.f.home, &self.f.member).unwrap()
    }
    fn filters(&self) -> Filters {
        Filters::compile(&self.f.member.filters).unwrap()
    }
    /// `count` invented tests on `day`, one a minute; every fourth is one the stub answers wrong.
    /// Even ones are about communication (medium confidence), odd ones about scope (high).
    fn keep_tests(&self, day: &str, count: usize) {
        for i in 0..count {
            let input = json!({
                "kind": "correction",
                "area": if i % 2 == 0 { "communication" } else { "scope" },
                "situation": format!(
                    "Test {i} on {day}: a student asks to move a quiz{}.",
                    if i % 4 == 0 { " WRONG" } else { "" }
                ),
                "wanted": format!("EXPECTED-{i} offer a smaller make-up slot in the same week."),
                "source": "review-report"
            });
            keep(
                &self.store(),
                &self.f.member,
                &self.filters(),
                &serde_json::to_vec(&input).unwrap(),
                &format!("{day}T10:{i:02}:00Z"),
            )
            .unwrap();
        }
    }
    fn cases(&self) -> Vec<Case> {
        self.store().cases().unwrap().iter().cloned().collect()
    }
    fn guard(&self) -> Guard<'static> {
        let claude: &'static Claude = Box::leak(Box::new(Claude::at(
            self.stub.join("claude"),
            self.stub.join("profile"),
        )));
        Guard::new(vec![claude], Ceilings::defaults())
    }
    fn backtest(&self) -> Result<Backtested> {
        let session = Session::open(&self.f.home, &self.f.member.slug, Cap::default()).unwrap();
        backtest::backtest(
            &self.guard(),
            &session,
            &self.f.home,
            &self.store(),
            &self.f.member,
            &self.dir,
            &self.filters(),
            "sonnet",
            NOW,
            &mut |_| {},
        )
    }
    fn drafted(&self) -> Proposal {
        match self.backtest().unwrap() {
            Backtested::Drafted(p) => p,
            other => panic!("expected a change, got {other:?}"),
        }
    }
    fn apply(&self, p: &Proposal) -> apply::Finished {
        refresh::answer(
            &self.store(),
            &self.dir,
            &p.id,
            Decision::Approved,
            None,
            NOW,
        )
        .unwrap();
        let session = Session::open(&self.f.home, &self.f.member.slug, Cap::default()).unwrap();
        let measured = apply::measure(
            &self.guard(),
            &session,
            &self.f.home,
            &self.store(),
            &self.f.member,
            &self.dir,
            &self.filters(),
            "sonnet",
            &p.id,
            &|| NOW.into(),
            &mut |_| {},
        )
        .unwrap();
        apply::finish(
            &self.store(),
            &self.f.member,
            &self.dir,
            &self.filters(),
            &measured.proposal,
            &measured.measurement,
        )
        .unwrap()
    }
    fn plan(&self, v: Value) {
        fs::write(self.stub.join("plan.json"), v.to_string()).unwrap();
    }
    fn prompts(&self) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(text) = fs::read_to_string(self.stub.join(format!("stdin{}", out.len()))) {
            out.push(text);
        }
        out
    }
    fn decisions(&self) -> String {
        fs::read_to_string(self.dir.join("decisions.md")).unwrap()
    }
}

fn section(decisions: &str) -> String {
    let start = decisions.find(HEADING).unwrap();
    let rest = &decisions[start + HEADING.len()..];
    let end = rest.find("\n## ").map_or(rest.len(), |at| at + 1);
    format!("{HEADING}{}", &rest[..end])
}

#[test]
fn too_few_tests_draft_nothing_and_ask_nothing() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 29);
    assert!(matches!(s.backtest().unwrap(), Backtested::TooFew(29)));
    assert!(s.prompts().is_empty());
    assert!(s.store().proposals().unwrap().is_empty());
}

#[test]
fn drafts_a_calibration_only_change_naming_the_ten_newest_right_tests() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    let before = s.decisions();
    let p = s.drafted();
    assert_eq!(p.changes.len(), 1);
    let after = diff::apply(&before, &p.changes[0].diff, "decisions.md").unwrap();
    // Only the Calibration section moved.
    assert_eq!(
        before.replace(&section(&before), ""),
        after.replace(&section(&after), "")
    );
    let new = section(&after);
    assert!(new.contains("Measured on 32 growth tests (2026-03-04 → 2026-03-04)"));
    assert!(new.contains("Overall 24 of 32 (75%)"));
    assert!(new.contains("90% right: scope."));
    assert!(new.contains("| Category | High | Medium | Low | Live |\n|---|---|---|---|---|\n"));
    assert!(
        new.contains("| scope | 16/16 | — | — | yes |\n| communication | — | 8/16 | — | no |\n")
    );
    assert!(
        !new.contains('<'),
        "the template's placeholder is gone: {new}"
    );
    // The ten newest the backtest got right: every fourth (0, 4, 8, ...) was answered wrong.
    let created: Vec<_> = p
        .cases
        .iter()
        .map(|id| {
            let case = s.cases().into_iter().find(|c| &c.id == id).unwrap();
            case.created[11..16].to_owned()
        })
        .collect();
    assert_eq!(
        created,
        [
            "10:31", "10:30", "10:29", "10:27", "10:26", "10:25", "10:23", "10:22", "10:21",
            "10:19"
        ]
    );
    assert!(refresh::show(&s.store(), &s.dir, None).unwrap().is_some());
}

#[test]
fn the_backtest_is_blind() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    s.drafted();
    let prompts = s.prompts();
    let asked: Vec<_> = prompts
        .iter()
        .filter(|p| p.starts_with("Answer each test"))
        .collect();
    assert!(!asked.is_empty());
    for prompt in asked {
        assert!(prompt.lines().next().unwrap().contains("how sure"));
        assert!(!prompt.contains("EXPECTED-"), "{prompt}");
    }
}

#[test]
fn applying_it_keeps_it_and_turns_only_the_named_tests_in_use() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    let p = s.drafted();
    let finished = s.apply(&p);
    assert_eq!(finished.outcome, KeepOrRevert::Keep);
    let active: Vec<_> = s
        .cases()
        .into_iter()
        .filter(|c| c.status == Status::Active)
        .map(|c| c.id)
        .collect();
    let mut named = p.cases.clone();
    named.sort();
    assert_eq!(active, named);
    assert!(section(&s.decisions()).contains("Measured on 32 growth tests"));
}

#[test]
fn only_tests_after_the_last_kept_change_count() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    let p = s.drafted();
    s.apply(&p);
    s.keep_tests("2026-03-05", 30);
    let next = s.drafted();
    let before = s.decisions();
    let after = diff::apply(&before, &next.changes[0].diff, "decisions.md").unwrap();
    assert!(section(&after).contains("Measured on 30 growth tests (2026-03-05 → 2026-03-05)"));
    assert!(next.cases.iter().all(|id| {
        s.cases()
            .iter()
            .any(|c| &c.id == id && c.created.starts_with("2026-03-05"))
    }));
}

#[test]
fn a_waiting_change_refuses_before_any_call() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    let p = s.drafted();
    let calls = s.prompts().len();
    let err = s.backtest().unwrap_err().to_string();
    assert!(err.contains(&p.id), "{err}");
    assert_eq!(s.prompts().len(), calls);
}

#[test]
fn no_calibration_section_refuses_before_any_call() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    let without = s.decisions().replace(HEADING, "## Calibration");
    fs::write(s.dir.join("decisions.md"), without).unwrap();
    let err = s.backtest().unwrap_err().to_string();
    assert!(err.contains("Calibration"), "{err}");
    assert!(s.prompts().is_empty());
}

#[test]
fn a_bad_confidence_is_asked_again_then_refused_plainly() {
    {
        let s = Setup::new();
        s.keep_tests("2026-03-04", 32);
        s.plan(json!({"0": "bad_confidence"}));
        s.drafted();
    }
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    s.plan(json!({"0": "bad_confidence", "1": "bad_confidence"}));
    let err = s.backtest().unwrap_err().to_string();
    assert!(err.contains("voice refresh backtest"), "{err}");
    assert!(s.store().proposals().unwrap().is_empty());
}

#[test]
fn an_unknown_id_is_never_repeated_back() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 1);
    let case = s.cases().remove(0);
    let reply = json!({"answers": [
        {"id": "INJECTED words", "answer": "Offer a smaller first step.", "confidence": "sure"}
    ]});
    let misfit = backtest::check_backtest_answers(&[&case], &reply).unwrap_err();
    assert!(!misfit.told.contains("INJECTED"), "{}", misfit.told);
}

fn rewrite_cases(s: &Setup, edit: impl Fn(&mut Vec<Case>)) {
    let mut cases = s.cases();
    edit(&mut cases);
    let text: String = cases.iter().map(to_line).collect();
    write_private_at(
        &s.f.home,
        &s.f.member.slug,
        Some("growth"),
        "cases.jsonl",
        text.as_bytes(),
    )
    .unwrap();
}

#[test]
fn a_rejected_change_is_not_drafted_again() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    let p = s.drafted();
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Rejected, None, NOW).unwrap();
    assert!(matches!(s.backtest().unwrap(), Backtested::Declined));
    assert_eq!(s.store().proposals().unwrap().len(), 1);
}

#[test]
fn an_approved_change_not_yet_applied_refuses_before_any_call() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    let p = s.drafted();
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Approved, None, NOW).unwrap();
    let calls = s.prompts().len();
    let err = s.backtest().unwrap_err().to_string();
    assert!(err.contains(&p.id), "{err}");
    assert_eq!(s.prompts().len(), calls);
}

#[test]
fn a_section_already_up_to_date_drafts_nothing() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    let p = s.drafted();
    let before = s.decisions();
    let after = diff::apply(&before, &p.changes[0].diff, "decisions.md").unwrap();
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Rejected, None, NOW).unwrap();
    fs::write(s.dir.join("decisions.md"), after).unwrap();
    assert!(matches!(s.backtest().unwrap(), Backtested::Unchanged));
    assert_eq!(s.store().proposals().unwrap().len(), 1);
}

#[test]
fn none_right_drafts_nothing() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    rewrite_cases(&s, |cases| {
        for case in cases {
            case.situation.push_str(" WRONG");
        }
    });
    assert!(matches!(s.backtest().unwrap(), Backtested::NoneRight));
    assert!(s.store().proposals().unwrap().is_empty());
}

#[test]
fn over_forty_tests_are_asked_in_batches_and_all_scored() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 41);
    let p = s.drafted();
    let asked = s
        .prompts()
        .iter()
        .filter(|p| p.starts_with("Answer each test"))
        .count();
    assert!(asked >= 2, "{asked}");
    let after = diff::apply(&s.decisions(), &p.changes[0].diff, "decisions.md").unwrap();
    assert!(section(&after).contains("Measured on 41 growth tests"));
}

#[test]
fn a_renamed_area_is_backtested_as_its_own_row() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    rewrite_cases(&s, |cases| cases[1].area = "scope of work".into());
    let p = s.drafted();
    let after = diff::apply(&s.decisions(), &p.changes[0].diff, "decisions.md").unwrap();
    let new = section(&after);
    // The stub is only sure of "scope" itself.
    assert!(
        new.contains("| scope of work | — | 1/1 | — | no |"),
        "{new}"
    );
}

#[test]
fn an_area_that_breaks_a_line_or_cell_refuses_naming_the_test() {
    for bad in ["scope\n## Injected", "scope\r", "scope | yes"] {
        let s = Setup::new();
        s.keep_tests("2026-03-04", 32);
        rewrite_cases(&s, |cases| cases[3].area = bad.into());
        let id = s.cases()[3].id.clone();
        let err = s.backtest().unwrap_err().to_string();
        assert!(err.contains(&id), "{err}");
        assert!(s.prompts().is_empty());
    }
}

#[test]
fn a_line_too_long_for_a_change_refuses_before_any_call() {
    let s = Setup::new();
    s.keep_tests("2026-03-04", 32);
    rewrite_cases(&s, |cases| cases[1].area = "x".repeat(600));
    let err = s.backtest().unwrap_err().to_string();
    assert!(err.contains("too long"), "{err}");
    assert!(s.prompts().is_empty());
}

#[test]
fn replace_calibration_handles_the_edges() {
    let new = format!("{HEADING}\n\nnew\n");
    assert_eq!(
        backtest::replace_calibration(&format!("# A\n\n{HEADING}\n\nold\n"), &new).unwrap(),
        format!("# A\n\n{new}")
    );
    assert_eq!(
        backtest::replace_calibration(&format!("{HEADING}\nold\n\n## Next\nkeep\n"), &new).unwrap(),
        format!("{new}\n## Next\nkeep\n")
    );
    assert!(
        backtest::replace_calibration(&format!("{HEADING}\r\nold\r\n## Next\r\n"), &new)
            .unwrap()
            .ends_with("\n## Next\r\n")
    );
    assert!(backtest::replace_calibration("# A\n## Calibration\nold\n", &new).is_none());
    assert!(backtest::replace_calibration(&format!("x{HEADING}\n"), &new).is_none());
}

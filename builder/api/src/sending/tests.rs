use super::*;
use crate::radio::test_support::{FakeRadio, Scratch, link_hello};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

fn assistant(home: &Path, member: bool) -> PathBuf {
    voice_core::assistants::make(
        home,
        &voice_core::assistants::New {
            name: "Wren",
            pronouns: "she/her",
            principal: "Robin",
            principal_pronouns: "they/them",
        },
        "2026-11-17",
        "voice",
    )
    .unwrap();
    let state = home.join(".claude/state/wren");
    fs::create_dir_all(&state).unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
    if member {
        fs::write(state.join("save.json"), br#"{"version":1,"mode":"github","owner":"example-owner","name":"wren","branch":"main","role":"member"}"#).unwrap();
    }
    state
}
fn record(id: &str, text: &str) -> Value {
    json!({"id":id,"timestamp":"2026-11-17","kind":"message","text":text})
}
fn lines(records: &[Value]) -> Vec<u8> {
    records
        .iter()
        .map(|v| format!("{v}\n"))
        .collect::<String>()
        .into_bytes()
}
fn private(path: &Path) {
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
fn setup() -> Vec<Value> {
    vec![
        link_hello(),
        json!({"ok":true,"port":7341,"addresses":["10.0.0.5"]}),
        json!({"ok":true}),
    ]
}
fn computer(id: &str, state: &str, inviter: Option<&str>) -> Value {
    json!({"id":id,"name":"Studio","state":state,"invited_by":inviter,"trusted":false})
}
fn send_answers(computers: Vec<Value>, answer: Value) -> Vec<Value> {
    let mut a = setup();
    a.push(json!({"ok":true,"computers":computers}));
    if !answer.is_null() {
        a.push(answer);
    }
    a
}
#[test]
fn status_without_files_is_empty() {
    let h = Scratch::new();
    assistant(&h.0, true);
    let s = history_status(&h.0, "wren").unwrap();
    assert!(!s.pending);
    assert!(s.last_sent.is_none());
    assert!(s.computers.is_empty());
}
#[test]
fn status_reads_both_files_and_pending() {
    let h = Scratch::new();
    let s = assistant(&h.0, true);
    fs::write(s.join("pending-send.jsonl"), b"").unwrap();
    fs::write(
        s.join("last-sent.json"),
        br#"{"at":12,"to":"Studio","bytes":321}"#,
    )
    .unwrap();
    fs::write(
        s.join("deliveries-taken.json"),
        json!({"after":7,"computers":{"a".repeat(64):{"name":"Laptop","at":11,"records":4}}})
            .to_string(),
    )
    .unwrap();
    let answer = history_status(&h.0, "wren").unwrap();
    assert!(answer.pending);
    let sent = answer.last_sent.unwrap();
    assert_eq!(sent.to, "Studio");
    assert_eq!(sent.at, 12.0);
    assert_eq!(sent.bytes, 321);
    assert_eq!(answer.computers[0].name, "Laptop");
    assert_eq!(answer.computers[0].records, 4);
}
#[test]
fn local_refusals_happen_before_radio() {
    for (member, expected) in [
        (
            false,
            "This computer builds Wren. Your other computers send their history here.",
        ),
        (
            true,
            "Read this computer's Claude Code history for Wren first, on the Your history step.",
        ),
    ] {
        let h = Scratch::new();
        assistant(&h.0, member);
        let radio = Radio::at(h.0.join("absent.sock"));
        assert_eq!(
            send_with(&h.0, "wren", &radio).unwrap_err().message,
            expected
        );
    }
    let h = Scratch::new();
    assistant(&h.0, true);
    assert_eq!(
        take_with(&h.0, "wren", &Radio::at(h.0.join("absent.sock")))
            .unwrap_err()
            .message,
        "This computer sends its history to the computer that builds Wren; take deliveries there."
    );
}
#[test]
fn sends_pending_copy_to_accepted_founder_by_id() {
    let h = Scratch::new();
    let s = assistant(&h.0, true);
    let bytes = lines(&[record("one", "Prefer a small change.")]);
    fs::write(s.join("decisions-claude-code.jsonl"), &bytes).unwrap();
    fs::write(s.join("pending-send.jsonl"), b"older").unwrap();
    let expected_pending = fs::canonicalize(s.join("pending-send.jsonl")).unwrap();
    let fake = FakeRadio::answering(&send_answers(
        vec![
            computer("removed", "removed", None),
            computer("founder", "accepted", None),
            computer("other", "accepted", Some("founder")),
        ],
        json!({"ok":true,"size":bytes.len()}),
    ));
    let sent = send_with(&h.0, "wren", &fake.radio()).unwrap();
    assert_eq!(sent.to, "Studio");
    assert_eq!(sent.bytes, bytes.len() as u64);
    assert!(sent.at > 0.0);
    assert!(!s.join("pending-send.jsonl").exists());
    private(&s.join("last-sent.json"));
    let requests = fake.requests();
    assert_eq!(
        requests
            .iter()
            .map(|r| r["op"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "hello",
            "link_address_list",
            "register",
            "computer_list",
            "send"
        ]
    );
    assert_eq!(requests[4]["computer"], "founder");
    assert_eq!(requests[4]["file"], expected_pending.to_str().unwrap());
    let name = requests[4]["name"].as_str().unwrap();
    let parts: Vec<_> = name
        .strip_prefix("history-")
        .unwrap()
        .strip_suffix(".jsonl")
        .unwrap()
        .split('-')
        .collect();
    assert_eq!(parts.len(), 2);
    for (part, max) in parts.iter().zip([20, 10]) {
        assert!(!part.is_empty() && part.len() <= max && part.bytes().all(|b| b.is_ascii_digit()));
    }
}
#[test]
fn send_refusals_keep_private_pending_copy() {
    for (phrase, sentence) in [
        (
            "the computer could not be reached",
            "Voice couldn't reach the computer that builds this assistant. Check that both are on the VPN and Voice is open there. This computer sends it again next time.",
        ),
        (
            "the computer did not answer",
            "Voice couldn't reach the computer that builds this assistant. Check that both are on the VPN and Voice is open there. This computer sends it again next time.",
        ),
        (
            "that app takes no files on that computer",
            "The computer that builds this assistant isn't ready to take history yet. Make a join code there once, then try again.",
        ),
        (
            "that computer is busy; try again",
            "The computer that builds this assistant is busy. Try again in a moment.",
        ),
        (
            "a file with that name is already there",
            "The computer that builds this assistant is busy. Try again in a moment.",
        ),
        ("the link stopped", "Radio said: the link stopped."),
    ] {
        let h = Scratch::new();
        let s = assistant(&h.0, true);
        let bytes = lines(&[record("one", "Keep the tests.")]);
        fs::write(s.join("decisions-claude-code.jsonl"), &bytes).unwrap();
        let fake = FakeRadio::answering(&send_answers(
            vec![computer("founder", "accepted", None)],
            json!({"ok":false,"error":phrase}),
        ));
        assert_eq!(
            send_with(&h.0, "wren", &fake.radio()).unwrap_err().message,
            sentence
        );
        assert_eq!(fs::read(s.join("pending-send.jsonl")).unwrap(), bytes);
        private(&s.join("pending-send.jsonl"));
        assert!(!s.join("last-sent.json").exists());
        fake.requests();
    }
}
#[test]
fn no_or_multiple_founders_send_nothing() {
    for computers in [
        vec![],
        vec![computer("gone", "removed", None)],
        vec![
            computer("one", "accepted", None),
            computer("two", "accepted", None),
        ],
    ] {
        let h = Scratch::new();
        let s = assistant(&h.0, true);
        fs::write(s.join("decisions-claude-code.jsonl"), b"").unwrap();
        let fake = FakeRadio::answering(&send_answers(computers, Value::Null));
        assert_eq!(
            send_with(&h.0, "wren", &fake.radio()).unwrap_err().message,
            "Voice can't tell which of your computers builds this assistant. Link this computer again with a new join code from that computer."
        );
        assert_eq!(fake.requests().len(), 4);
        assert!(!s.join("pending-send.jsonl").exists());
    }
}
fn delivery(id: i64, key: char, name: &str) -> Value {
    json!({"id":id,"computer":format!("Laptop {key}"),"key":key.to_string().repeat(64),"name":name,"size":1,"at":100+id})
}
#[test]
fn takes_two_peers_and_never_shrinks_then_pages_from_saved_cursor() {
    let h = Scratch::new();
    let s = assistant(&h.0, false);
    let folder = s.join("deliveries");
    fs::create_dir(&folder).unwrap();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        folder.join("history-1-1.jsonl"),
        lines(&[
            record("one", "Keep it small."),
            record("two", "Run the checks."),
        ]),
    )
    .unwrap();
    fs::write(
        folder.join("history-2-1.jsonl"),
        lines(&[record("other", "Use the existing style.")]),
    )
    .unwrap();
    fs::write(
        folder.join("history-3-1.jsonl"),
        lines(&[
            record("three", "Read the brief."),
            record("one", "Duplicate."),
            record("private", "my therapist"),
            json!({"kind":"message","text":"No id."}),
        ]),
    )
    .unwrap();
    let mut answers = setup();
    answers.extend([json!({"ok":true,"deliveries":[delivery(1,'a',"history-1-1.jsonl"),delivery(2,'b',"history-2-1.jsonl")]}),json!({"ok":true,"deliveries":[delivery(3,'a',"history-3-1.jsonl")]}),json!({"ok":true,"deliveries":[]})]);
    let fake = FakeRadio::answering(&answers);
    let answer = take_with(&h.0, "wren", &fake.radio()).unwrap();
    assert_eq!(answer.taken, 3);
    assert_eq!(answer.refused, 0);
    let records = voice_ingest::corpus::read_records(
        "history",
        &fs::read(s.join("decisions-from-aaaaaaaaaaaaaaaa.jsonl")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        records
            .iter()
            .map(|(_, r)| r["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["one", "three", "two"]
    );
    private(&s.join("decisions-from-aaaaaaaaaaaaaaaa.jsonl"));
    private(&s.join("decisions-from-bbbbbbbbbbbbbbbb.jsonl"));
    private(&s.join("deliveries-taken.json"));
    assert_eq!(
        answer
            .computers
            .iter()
            .map(|c| c.records)
            .collect::<Vec<_>>(),
        [3, 1]
    );
    assert!(fs::read_dir(&folder).unwrap().next().is_none());
    let requests = fake.requests();
    assert_eq!(requests[3]["after"], 0);
    assert_eq!(requests[3]["limit"], 500);
    assert_eq!(requests[4]["after"], 2);
    assert_eq!(requests[5]["after"], 3);
    let mut answers = setup();
    answers.push(json!({"ok":true,"deliveries":[]}));
    let fake = FakeRadio::answering(&answers);
    assert_eq!(take_with(&h.0, "wren", &fake.radio()).unwrap().taken, 0);
    assert_eq!(fake.requests()[3]["after"], 3);
}
#[test]
fn refuses_bad_files_deletes_them_and_advances_cursor() {
    let h = Scratch::new();
    let s = assistant(&h.0, false);
    let folder = s.join("deliveries");
    fs::create_dir(&folder).unwrap();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();
    let outside = h.0.join("outside");
    fs::write(&outside, lines(&[record("outside", "Keep this file.")])).unwrap();
    fs::write(folder.join("other.txt"), b"not history").unwrap();
    std::os::unix::fs::symlink(&outside, folder.join("history-2-1.jsonl")).unwrap();
    fs::create_dir(folder.join("history-3-1.jsonl")).unwrap();
    fs::File::create(folder.join("history-4-1.jsonl"))
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    fs::write(folder.join("history-5-1.jsonl"), b"not json").unwrap();
    let names = [
        "other.txt",
        "history-2-1.jsonl",
        "history-3-1.jsonl",
        "history-4-1.jsonl",
        "history-5-1.jsonl",
        "history-6-1.jsonl",
    ];
    let mut answers = setup();
    answers.extend([json!({"ok":true,"deliveries":names.iter().enumerate().map(|(i,n)|delivery(i as i64+1,'a',n)).collect::<Vec<_>>()}),json!({"ok":true,"deliveries":[]})]);
    let fake = FakeRadio::answering(&answers);
    let answer = take_with(&h.0, "wren", &fake.radio()).unwrap();
    assert_eq!(answer.refused, 5);
    assert_eq!(answer.taken, 0);
    assert!(answer.computers.is_empty());
    assert!(outside.exists());
    assert!(fs::read_dir(folder).unwrap().next().is_none());
    assert_eq!(fake.requests()[4]["after"], 6);
    let saved: Value =
        serde_json::from_slice(&fs::read(s.join("deliveries-taken.json")).unwrap()).unwrap();
    assert_eq!(saved["after"], 6);
}
#[test]
fn forget_removes_the_whole_state_folder_including_sent_and_received_history() {
    let h = Scratch::new();
    let s = assistant(&h.0, false);
    fs::create_dir(s.join("deliveries")).unwrap();
    for name in [
        "pending-send.jsonl",
        "last-sent.json",
        "deliveries-taken.json",
        "decisions-from-aaaaaaaaaaaaaaaa.jsonl",
        "deliveries/history-1-1.jsonl",
    ] {
        fs::write(s.join(name), b"invented").unwrap();
    }
    let files = crate::use_it::forget_preview_in(
        &h.0,
        dto::SlugRequest {
            slug: "wren".into(),
        },
    )
    .unwrap()
    .files;
    crate::use_it::forget_in(
        &h.0,
        dto::ForgetRequest {
            slug: "wren".into(),
            files,
        },
    )
    .unwrap();
    assert!(!s.exists());
    assert!(h.0.join(".claude/crew/wren/member.json").exists());
}

// File-level checks exercise the same delivery processor without requiring a socket.
fn empty_taken() -> dto::Taken {
    dto::Taken {
        taken: 0,
        refused: 0,
        computers: vec![],
    }
}
fn filters(home: &Path) -> Filters {
    Filters::compile(
        &crate::assistants::listed(home, "wren")
            .unwrap()
            .member
            .filters,
    )
    .unwrap()
}
#[test]
fn founder_rule_ignores_removed_and_invited_computers() {
    let candidates = vec![
        computer("gone", "removed", None),
        computer("member", "accepted", Some("builder")),
        computer("builder", "accepted", None),
    ];
    let candidates = serde_json::from_value(json!(candidates)).unwrap();
    assert_eq!(founder(candidates).unwrap().id, "builder");
    assert!(founder(vec![]).is_err());
    assert!(
        founder(
            serde_json::from_value(json!([
                computer("a", "accepted", None),
                computer("b", "accepted", None)
            ]))
            .unwrap()
        )
        .is_err()
    );
}
#[test]
fn fold_keeps_old_ids_and_deduplicates_a_shorter_read() {
    let h = Scratch::new();
    assistant(&h.0, false);
    let old = lines(&[record("a", "Keep this."), record("c", "Keep that.")]);
    let new = lines(&[
        record("b", "A new thought."),
        record("a", "A changed duplicate."),
        json!({"kind":"message","text":"No id."}),
    ]);
    let bytes = fold(&filters(&h.0), &old, &new).unwrap();
    let records = corpus::read_records("history", &bytes).unwrap();
    assert_eq!(
        records
            .iter()
            .map(|(_, r)| r["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(records[0].1["text"], "Keep this.");
}
#[test]
fn fold_reruns_the_members_personal_and_secret_filters() {
    let h = Scratch::new();
    assistant(&h.0, false);
    let input = lines(&[
        record("ok", "Keep the change small."),
        record("private", "my therapist"),
        record("secret", "Contact example@example.test"),
    ]);
    let bytes = fold(&filters(&h.0), b"", &input).unwrap();
    let records = corpus::read_records("history", &bytes).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].1["id"], "ok");
}
#[test]
fn failed_send_keeps_the_pending_file_and_success_clears_it() {
    let h = Scratch::new();
    assistant(&h.0, true);
    let pending = private::write_private(&h.0, "wren", "pending-send.jsonl", b"invented").unwrap();
    assert!(
        finish_send(
            &h.0,
            "wren",
            "Studio".into(),
            Err(AskError::Refused("the computer did not answer".into()))
        )
        .is_err()
    );
    assert_eq!(fs::read(&pending).unwrap(), b"invented");
    private(&pending);
    let sent = finish_send(&h.0, "wren", "Studio".into(), Ok(8)).unwrap();
    assert_eq!(sent.bytes, 8);
    assert_eq!(
        history_status(&h.0, "wren")
            .unwrap()
            .last_sent
            .unwrap()
            .bytes,
        8
    );
    assert!(!pending.exists());
    private(&h.0.join(".claude/state/wren/last-sent.json"));
}
fn prepare_delivery() -> (Scratch, PathBuf) {
    let h = Scratch::new();
    let s = assistant(&h.0, false);
    private::private_folder(&h.0, "wren", "deliveries").unwrap();
    (h, s)
}
fn process(h: &Scratch, d: Value) -> (Received, dto::Taken) {
    let mut saved = Received::default();
    let mut answer = empty_taken();
    take_one(
        &h.0,
        "wren",
        &filters(&h.0),
        &mut saved,
        &mut answer,
        &serde_json::from_value(d).unwrap(),
    )
    .unwrap();
    (saved, answer)
}
#[test]
fn delivery_symlink_is_refused_without_reading_its_target() {
    let (h, s) = prepare_delivery();
    let target = h.0.join("outside.jsonl");
    fs::write(&target, lines(&[record("outside", "Never import this.")])).unwrap();
    let path = s.join("deliveries/history-1-1.jsonl");
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let (saved, answer) = process(&h, delivery(1, 'a', "history-1-1.jsonl"));
    assert_eq!(answer.refused, 1);
    assert_eq!(answer.taken, 0);
    assert_eq!(saved.after, 1);
    assert!(target.exists());
    assert!(!path.symlink_metadata().is_ok());
    assert!(!s.join("decisions-from-aaaaaaaaaaaaaaaa.jsonl").exists());
}
#[test]
fn delivery_size_cap_checks_opened_file_not_radios_size() {
    let (h, s) = prepare_delivery();
    let path = s.join("deliveries/history-1-1.jsonl");
    let mut file = fs::File::create(&path).unwrap();
    use std::io::Write;
    // Valid JSON followed by whitespace: without the cap this parses and would be taken.
    file.write_all(&lines(&[record("large", "Keep it small.")]))
        .unwrap();
    let padding = vec![b' '; 1024 * 1024];
    for _ in 0..64 {
        file.write_all(&padding).unwrap();
    }
    drop(file);
    let (saved, answer) = process(&h, delivery(1, 'a', "history-1-1.jsonl"));
    assert_eq!(answer.refused, 1);
    assert_eq!(answer.taken, 0);
    assert_eq!(saved.after, 1);
    assert!(!path.exists());
}
#[test]
fn refused_delivery_moves_the_saved_cursor_and_missing_is_skipped() {
    let (h, s) = prepare_delivery();
    fs::write(s.join("deliveries/history-7-1.jsonl"), b"not json").unwrap();
    let (saved, answer) = process(&h, delivery(7, 'a', "history-7-1.jsonl"));
    assert_eq!(answer.refused, 1);
    assert_eq!(saved.after, 7);
    let on_disk: Value =
        serde_json::from_slice(&fs::read(s.join("deliveries-taken.json")).unwrap()).unwrap();
    assert_eq!(on_disk["after"], 7);
    let (saved, answer) = process(&h, delivery(8, 'a', "history-8-1.jsonl"));
    assert_eq!(answer.taken, 0);
    assert_eq!(answer.refused, 0);
    assert_eq!(saved.after, 8);
}
#[test]
fn file_processor_merges_two_peers_and_repeated_delivery_is_harmless() {
    let (h, s) = prepare_delivery();
    let mut saved = Received::default();
    let mut answer = empty_taken();
    for (id, key, records) in [
        (
            1,
            'a',
            vec![record("a", "Keep it small."), record("b", "Test it.")],
        ),
        (2, 'b', vec![record("c", "Read it.")]),
        (
            3,
            'a',
            vec![record("a", "Duplicate."), record("d", "Another thought.")],
        ),
    ] {
        let name = format!("history-{id}-1.jsonl");
        fs::write(s.join("deliveries").join(&name), lines(&records)).unwrap();
        take_one(
            &h.0,
            "wren",
            &filters(&h.0),
            &mut saved,
            &mut answer,
            &serde_json::from_value(delivery(id, key, &name)).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(answer.taken, 3);
    assert_eq!(saved.computers[&"a".repeat(64)].records, 3);
    assert_eq!(saved.computers[&"b".repeat(64)].records, 1);
    private(&s.join("decisions-from-aaaaaaaaaaaaaaaa.jsonl"));
    private(&s.join("decisions-from-bbbbbbbbbbbbbbbb.jsonl"));
    private(&s.join("deliveries-taken.json"));
    assert!(fs::read_dir(s.join("deliveries")).unwrap().next().is_none());
}
#[test]
fn invalid_delivery_names_nonregular_files_and_bad_shapes_are_refused() {
    for (name, contents) in [
        ("other.txt", b"wrong".as_slice()),
        ("history-1-1.jsonl", b"[]"),
        ("history-123456789012345678901-1.jsonl", b"{}"),
        ("history-1-12345678901.jsonl", b"{}"),
        (
            "history-1-1.jsonl",
            br#"{"id":"bad","kind":"message","timestamp":"now"}"#,
        ),
    ] {
        let (h, s) = prepare_delivery();
        fs::write(s.join("deliveries").join(name), contents).unwrap();
        let (saved, answer) = process(&h, delivery(1, 'a', name));
        assert_eq!(answer.refused, 1);
        assert_eq!(saved.after, 1);
        assert!(fs::read_dir(s.join("deliveries")).unwrap().next().is_none());
    }
    let (h, s) = prepare_delivery();
    fs::create_dir(s.join("deliveries/history-1-1.jsonl")).unwrap();
    let (_, answer) = process(&h, delivery(1, 'a', "history-1-1.jsonl"));
    assert_eq!(answer.refused, 1);
    assert!(fs::read_dir(s.join("deliveries")).unwrap().next().is_none());
}

#[test]
fn nonempty_delivery_directory_is_removed_without_following_nested_links() {
    let (h, s) = prepare_delivery();
    let outside = h.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), b"keep this").unwrap();
    let rejected = s.join("deliveries/history-1-1.jsonl");
    fs::create_dir_all(rejected.join("nested")).unwrap();
    fs::write(rejected.join("nested/file"), b"invented").unwrap();
    std::os::unix::fs::symlink(&outside, rejected.join("link")).unwrap();
    let (saved, answer) = process(&h, delivery(1, 'a', "history-1-1.jsonl"));
    assert_eq!(answer.refused, 1);
    assert_eq!(saved.after, 1);
    assert!(!rejected.exists());
    assert_eq!(fs::read(outside.join("keep")).unwrap(), b"keep this");
}

#[test]
fn exported_counts_match_the_json_numbers_the_bridge_receives() {
    assert!(<dto::Sent as ts_rs::TS>::decl(&ts_rs::Config::default()).contains("bytes: number"));
    assert!(
        <dto::SentFrom as ts_rs::TS>::decl(&ts_rs::Config::default()).contains("records: number")
    );
}

#[test]
fn delivery_missing_kind_is_refused_without_changing_peer_history() {
    let (h, s) = prepare_delivery();
    let peer = s.join("decisions-from-aaaaaaaaaaaaaaaa.jsonl");
    let old = lines(&[record("old", "Keep the checks.")]);
    fs::write(&peer, &old).unwrap();
    let incoming = s.join("deliveries/history-1-1.jsonl");
    fs::write(
        &incoming,
        lines(&[json!({"id":"new","timestamp":"2026-11-17","text":"Keep it small."})]),
    )
    .unwrap();
    let (saved, answer) = process(&h, delivery(1, 'a', "history-1-1.jsonl"));
    assert_eq!(answer.refused, 1);
    assert_eq!(answer.taken, 0);
    assert_eq!(saved.after, 1);
    assert_eq!(fs::read(&peer).unwrap(), old);
    assert!(!incoming.exists());
    assert!(saved.computers.is_empty());
}

#[test]
fn taken_peer_history_passes_the_builds_corpus_merge() {
    let (h, s) = prepare_delivery();
    fs::write(
        s.join("deliveries/history-1-1.jsonl"),
        lines(&[record("one", "Keep it small.")]),
    )
    .unwrap();
    let (_, answer) = process(&h, delivery(1, 'a', "history-1-1.jsonl"));
    assert_eq!(answer.taken, 1);
    let bytes = fs::read(s.join("decisions-from-aaaaaaaaaaaaaaaa.jsonl")).unwrap();
    corpus::merge(
        &filters(&h.0),
        &[corpus::Input {
            name: "peer history",
            mode: corpus::Mode::History,
            bytes: &bytes,
        }],
        &voice_ingest::mine::Split::Holdout(voice_ingest::mine::DEFAULT_HOLDOUT),
    )
    .unwrap();
}

#[test]
fn fold_sorts_old_records_without_required_keys_without_panicking() {
    let h = Scratch::new();
    assistant(&h.0, false);
    let old = lines(&[
        json!({"text":"Old incomplete record."}),
        json!({"timestamp":"2026-11-17","text":"Old record without an id."}),
        record("old", "Keep this."),
    ]);
    let bytes = fold(
        &filters(&h.0),
        &old,
        &lines(&[record("new", "Keep it small.")]),
    )
    .unwrap();
    assert_eq!(corpus::read_records("history", &bytes).unwrap().len(), 4);
}

#[test]
fn sent_names_are_screened_on_return_but_stored_as_given() {
    let h = Scratch::new();
    let s = assistant(&h.0, true);
    private::write_private(&h.0, "wren", "pending-send.jsonl", b"history").unwrap();
    let name = "Work\u{1b}[31m laptop\u{202e}";
    let sent = finish_send(&h.0, "wren", name.into(), Ok(7)).unwrap();
    assert_eq!(sent.to, "Work?[31m laptop?");
    let saved: Value =
        serde_json::from_slice(&fs::read(s.join("last-sent.json")).unwrap()).unwrap();
    assert_eq!(saved["to"], name);
    assert_eq!(
        history_status(&h.0, "wren").unwrap().last_sent.unwrap().to,
        "Work?[31m laptop?"
    );
}

#[test]
fn take_and_status_screen_delivered_names_but_keep_the_original_on_disk() {
    let (h, s) = prepare_delivery();
    fs::write(
        s.join("deliveries/history-1-1.jsonl"),
        lines(&[record("one", "Keep it small.")]),
    )
    .unwrap();
    let mut d = delivery(1, 'a', "history-1-1.jsonl");
    d["computer"] = json!("Work\u{1b}[31m laptop\u{202e}");
    let mut answers = setup();
    answers.extend([
        json!({"ok":true,"deliveries":[d]}),
        json!({"ok":true,"deliveries":[]}),
    ]);
    let fake = FakeRadio::answering(&answers);
    let taken = take_with(&h.0, "wren", &fake.radio()).unwrap();
    assert_eq!(taken.computers[0].name, "Work?[31m laptop?");
    assert_eq!(
        history_status(&h.0, "wren").unwrap().computers[0].name,
        "Work?[31m laptop?"
    );
    let saved: Value =
        serde_json::from_slice(&fs::read(s.join("deliveries-taken.json")).unwrap()).unwrap();
    assert_eq!(
        saved["computers"]["a".repeat(64)]["name"],
        "Work\u{1b}[31m laptop\u{202e}"
    );
    fake.requests();
}

#[test]
fn status_screens_saved_computer_names() {
    let h = Scratch::new();
    let s = assistant(&h.0, false);
    fs::write(s.join("deliveries-taken.json"), json!({"after":1,"computers":{"a".repeat(64):{"name":"Work\u{1b}[31m laptop\u{202e}","at":1,"records":1}}}).to_string()).unwrap();
    assert_eq!(
        history_status(&h.0, "wren").unwrap().computers[0].name,
        "Work?[31m laptop?"
    );
}

#[test]
fn older_last_sent_without_bytes_reads_as_zero() {
    let h = Scratch::new();
    let s = assistant(&h.0, true);
    fs::write(s.join("last-sent.json"), br#"{"at":12,"to":"Studio"}"#).unwrap();
    assert_eq!(
        history_status(&h.0, "wren")
            .unwrap()
            .last_sent
            .unwrap()
            .bytes,
        0
    );
}

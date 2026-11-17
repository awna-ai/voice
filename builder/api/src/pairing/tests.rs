use super::*;
use crate::radio::test_support::{FakeRadio, Scratch, link_hello};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn assistant(home: &Path, slug: &str, name: &str, member: bool) -> PathBuf {
    voice_core::assistants::make(
        home,
        &voice_core::assistants::New {
            name,
            pronouns: "she/her",
            principal: "Robin",
            principal_pronouns: "they/them",
        },
        "2026-11-17",
        "voice",
    )
    .unwrap();
    let state = home.join(".claude/state").join(slug);
    fs::create_dir_all(&state).unwrap();
    if member {
        fs::write(state.join("save.json"), serde_json::to_vec(&json!({"version":1,"mode":"github","owner":"robin","name":"wren","branch":"main","role":"member","builder":"Studio"})).unwrap()).unwrap();
    }
    record::read_record(&state).expect("the hand-written save record is valid");
    state
}
fn addresses() -> Value {
    json!({"ok":true,"port":7341,"addresses":["10.0.0.5"]})
}
fn fleet() -> Value {
    json!({"id":"f1","name":"voice","version":1,"founder":"k1"})
}
fn fleet_answer() -> Value {
    json!({"ok":true,"fleet":fleet(),"recovery_file":"/tmp/recovery"})
}
fn code_answer() -> Value {
    json!({"ok":true,"code":"rj1.fixture","code_id":"c1","expires_at":1800.0})
}
fn listing(home: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(p: &Path, entries: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut paths: Vec<_> = fs::read_dir(p)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                entries.push((path.clone(), vec![]));
                walk(&path, entries)
            } else {
                entries.push((path.clone(), fs::read(path).unwrap()));
            }
        }
    }
    let mut entries = vec![];
    walk(home, &mut entries);
    entries
}
#[test]
fn invite_orders_requests_and_answers_fields() {
    let h = Scratch::new();
    let state = assistant(&h.0, "wren", "Wren", false);
    let fake = FakeRadio::answering(&[
        link_hello(),
        addresses(),
        json!({"ok":true}),
        json!({"ok":true}),
        fleet_answer(),
        code_answer(),
    ]);
    let answer = invite_with(&h.0, "wren", &fake.radio()).unwrap();
    assert_eq!(answer.code, "rj1.fixture");
    assert_eq!(answer.expires_at, 1800.0);
    assert_eq!(answer.port, 7341);
    assert_eq!(answer.addresses, vec!["10.0.0.5"]);
    assert_eq!(answer.recovery_file.as_deref(), Some("/tmp/recovery"));
    assert_eq!(
        fake.requests(),
        vec![
            json!({"op":"hello","v":1}),
            json!({"op":"link_address_list","v":1}),
            json!({"op":"register","v":1,"app":"voice","app_version":env!("CARGO_PKG_VERSION"),"members":[]}),
            json!({"op":"delivery_folder_set","v":1,"app":"voice","folder":state.join("deliveries")}),
            json!({"op":"fleet_init","v":1,"name":"voice"}),
            json!({"op":"code_create","v":1})
        ]
    );
}
#[test]
fn already_in_a_fleet_invites_without_recovery() {
    let h = Scratch::new();
    assistant(&h.0, "wren", "Wren", false);
    let fake = FakeRadio::answering(&[
        link_hello(),
        addresses(),
        json!({"ok":true}),
        json!({"ok":true}),
        json!({"ok":false,"error":"already in a fleet"}),
        code_answer(),
    ]);
    assert_eq!(
        invite_with(&h.0, "wren", &fake.radio())
            .unwrap()
            .recovery_file,
        None
    );
    assert_eq!(fake.requests().len(), 6);
}
#[test]
fn failed_code_creation_keeps_the_recovery_notice() {
    let h = Scratch::new();
    assistant(&h.0, "wren", "Wren", false);
    let fake = FakeRadio::answering(&[
        link_hello(),
        addresses(),
        json!({"ok":true}),
        json!({"ok":true}),
        fleet_answer(),
        json!({"ok":false,"error":"code refused"}),
    ]);
    assert_eq!(
        invite_with(&h.0, "wren", &fake.radio())
            .unwrap_err()
            .message,
        "Radio said: code refused. Radio saved a recovery key for your computers in /tmp/recovery. Move it into your password manager, then delete that file."
    );
    assert_eq!(fake.requests().len(), 6);
}
#[test]
fn member_cannot_invite_before_any_request() {
    let h = Scratch::new();
    assistant(&h.0, "wren", "Wren", true);
    let fake = FakeRadio::answering(&[]);
    assert_eq!(
        invite_with(&h.0, "wren", &fake.radio())
            .unwrap_err()
            .message,
        "Add another computer from Studio, the computer that builds Wren."
    );
    assert!(fake.requests().is_empty());
}
#[test]
fn another_assistants_deliveries_refuse_before_any_request() {
    let h = Scratch::new();
    assistant(&h.0, "wren", "Wren", false);
    let state = assistant(&h.0, "lark", "Lark", false);
    fs::create_dir(state.join("deliveries")).unwrap();
    let fake = FakeRadio::answering(&[]);
    assert_eq!(
        invite_with(&h.0, "wren", &fake.radio())
            .unwrap_err()
            .message,
        "Lark already takes the history your other computers send to this one. Voice does this for one assistant on each computer for now."
    );
    assert!(fake.requests().is_empty());
}
#[test]
fn invite_needs_an_address_before_registering() {
    let h = Scratch::new();
    assistant(&h.0, "wren", "Wren", false);
    let fake = FakeRadio::answering(&[link_hello(), json!({"ok":true,"port":7341,"addresses":[]})]);
    assert_eq!(
        invite_with(&h.0, "wren", &fake.radio())
            .unwrap_err()
            .message,
        "Add this computer's VPN address first, so your other computers can find it."
    );
    assert_eq!(fake.requests().len(), 2);
}
#[test]
fn folder_not_made_by_radio_is_explained() {
    let h = Scratch::new();
    assistant(&h.0, "wren", "Wren", false);
    let fake = FakeRadio::answering(&[
        link_hello(),
        addresses(),
        json!({"ok":true}),
        json!({"ok":false,"error":"the folder already exists; name a new one"}),
    ]);
    assert_eq!(
        invite_with(&h.0, "wren", &fake.radio())
            .unwrap_err()
            .message,
        "~/.claude/state/wren/deliveries is already there, and Radio didn't make it. Move it somewhere else, then try again."
    );
    assert_eq!(fake.requests().len(), 4);
}
#[test]
fn naming_registers_before_resigning_only_the_name() {
    let home = Scratch::new();
    let fake = FakeRadio::answering(&[link_hello(), json!({"ok":true}), json!({"ok":true})]);
    name_in(&home.0, &fake.radio(), "Work laptop").unwrap();
    assert_eq!(
        fake.requests(),
        vec![
            json!({"op":"hello","v":1}),
            json!({"op":"register","v":1,"app":"voice","app_version":env!("CARGO_PKG_VERSION"),"members":[]}),
            json!({"op":"resign","v":1,"app":"voice","section":"{\"computer_name\":\"Work laptop\"}"}),
        ]
    );
}
#[test]
fn naming_without_a_fleet_explains_how_to_link() {
    let home = Scratch::new();
    let fake = FakeRadio::answering(&[
        link_hello(),
        json!({"ok":true}),
        json!({"ok":false,"error":"not in a fleet"}),
    ]);
    assert_eq!(
        name_in(&home.0, &fake.radio(), "Work laptop")
            .unwrap_err()
            .message,
        "This computer isn't linked to your other computers yet. Join them first, or make a join code here."
    );
    assert_eq!(fake.requests().len(), 3);
}
#[test]
fn debug_hides_join_codes() {
    let invite = dto::Invite {
        code: "rj1.fixture".into(),
        expires_at: 1.0,
        port: 7341,
        addresses: vec![],
        recovery_file: None,
    };
    assert!(!format!("{invite:?}").contains("rj1.fixture"));
    let request = dto::RadioJoinRequest {
        slug: "wren".into(),
        code: "rj1.fixture".into(),
        name: "Work laptop".into(),
    };
    assert!(!format!("{request:?}").contains("rj1.fixture"));
}
#[test]
fn join_local_refusals_send_nothing_and_write_nothing() {
    for (member, code, name, expected) in [
        (
            false,
            "rj1.fixture",
            "Work laptop",
            "This computer builds Wren. Make a join code here, then paste it on the other computer.",
        ),
        (
            true,
            "rj1.fixture",
            "bad\nname",
            "Give this computer a name of 1 to 64 characters, such as Work laptop, without symbols like @, #, $ or brackets.",
        ),
        (
            true,
            "not a code",
            "Work laptop",
            "That isn't a join code. Copy the whole code from the computer that builds your assistant, then try again.",
        ),
    ] {
        let h = Scratch::new();
        assistant(&h.0, "wren", "Wren", member);
        let before = listing(&h.0);
        let fake = FakeRadio::answering(&[]);
        assert_eq!(
            join_with(&h.0, "wren", code, name, &fake.radio())
                .unwrap_err()
                .message,
            expected
        );
        assert!(fake.requests().is_empty());
        assert_eq!(listing(&h.0), before);
    }
}
#[test]
fn join_maps_each_phrase_without_resigning_or_writing() {
    for (phrase, expected) in [
        (
            "not a join code",
            "That isn't a join code. Copy the whole code from the computer that builds your assistant, then try again.",
        ),
        (
            "the join was refused",
            "The other computer turned that code down. It may be used or out of date: make a new one there, then try again.",
        ),
        (
            "the computer could not be reached",
            "Voice couldn't reach the other computer. Check that both are on the VPN and Voice is open there, then try again.",
        ),
        (
            "the computer did not answer",
            "Voice couldn't reach the other computer. Check that both are on the VPN and Voice is open there, then try again.",
        ),
        (
            "the join code has no address to dial",
            "That code doesn't say where the other computer is. Add its VPN address there, make a new code, then try again.",
        ),
        (
            "already in a fleet",
            "This computer is already linked to your other computers.",
        ),
        ("another refusal", "Radio said: another refusal."),
    ] {
        let h = Scratch::new();
        assistant(&h.0, "wren", "Wren", true);
        let before = listing(&h.0);
        let fake = FakeRadio::answering(&[
            link_hello(),
            addresses(),
            json!({"ok":true}),
            json!({"ok":false,"error":phrase}),
        ]);
        assert_eq!(
            join_with(&h.0, "wren", "rj1.fixture", "Work laptop", &fake.radio())
                .unwrap_err()
                .message,
            expected
        );
        let requests = fake.requests();
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[3]["op"], "join");
        assert_eq!(listing(&h.0), before);
    }
}
#[test]
fn join_resigns_only_the_name_and_reports_naming_failure() {
    for naming_ok in [true, false] {
        let h = Scratch::new();
        assistant(&h.0, "wren", "Wren", true);
        let fake = FakeRadio::answering(&[
            link_hello(),
            addresses(),
            json!({"ok":true}),
            json!({"ok":true,"fleet":fleet()}),
            json!({"ok":naming_ok,"error":"name refused"}),
        ]);
        let answer = join_with(&h.0, "wren", " rj1.fixture ", "Work laptop", &fake.radio());
        if naming_ok {
            assert_eq!(
                answer.unwrap(),
                dto::Joined {
                    fleet: "voice".into(),
                    computer_name: "Work laptop".into()
                }
            );
        } else {
            assert_eq!(
                answer.unwrap_err().message,
                "This computer joined, but Voice couldn't save its name: Radio said: name refused. Try again with: voice radio name \"Work laptop\""
            );
        }
        let requests = fake.requests();
        assert_eq!(requests[3], json!({"op":"join","v":1,"code":"rj1.fixture"}));
        assert_eq!(
            requests[4],
            json!({"op":"resign","v":1,"app":"voice","section":"{\"computer_name\":\"Work laptop\"}"})
        );
    }
}
#[test]
fn setting_address_checks_radio_before_listing_addresses() {
    let home = Scratch::new();
    let fake = FakeRadio::answering(&[link_hello(), addresses(), addresses()]);
    let answer = set_address_in(&home.0, &fake.radio(), "10.0.0.5").unwrap();
    assert_eq!(answer.addresses, vec!["10.0.0.5"]);
    assert_eq!(
        fake.requests(),
        vec![
            json!({"op":"hello","v":1}),
            json!({"op":"link_address_list","v":1}),
            json!({"op":"link_address_add","v":1,"address":"10.0.0.5"}),
        ]
    );
}
#[test]
fn radio_refused_addresses_send_nothing() {
    let home = Scratch::new();
    for address in [
        "0.0.0.0",
        "::",
        "224.0.0.1",
        "ff02::1",
        "255.255.255.255",
        "fe80::1",
        "febf::1",
        "::ffff:0.0.0.0",
        "::ffff:224.0.0.1",
        "::ffff:255.255.255.255",
    ] {
        let fake = FakeRadio::answering(&[]);
        assert_eq!(
            set_address_in(&home.0, &fake.radio(), address)
                .unwrap_err()
                .message,
            "That isn't a VPN address. Type the numbers only, like 10.0.0.5, with no port."
        );
        assert_eq!(
            set_address_with(&fake.radio(), address)
                .unwrap_err()
                .message,
            "That isn't a VPN address. Type the numbers only, like 10.0.0.5, with no port."
        );
        assert!(fake.requests().is_empty());
    }
}
#[test]
fn radio_entry_points_explain_missing_home() {
    // Remove HOME only in a child: other tests may read the process environment.
    if std::env::var_os("HOME").is_some() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "pairing::tests::radio_entry_points_explain_missing_home",
            ])
            .env_remove("HOME")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let failures = [
        radio_link_address().unwrap_err(),
        radio_set_link_address(dto::RadioAddressRequest {
            address: "10.0.0.5".into(),
        })
        .unwrap_err(),
        radio_invite(dto::SlugRequest {
            slug: "wren".into(),
        })
        .unwrap_err(),
        radio_join(dto::RadioJoinRequest {
            slug: "wren".into(),
            code: "rj1.fixture".into(),
            name: "Work laptop".into(),
        })
        .unwrap_err(),
        radio_name(dto::RadioNameRequest {
            name: "Work laptop".into(),
        })
        .unwrap_err(),
    ];
    for failure in failures {
        assert_eq!(
            failure.message,
            "Voice can't find your home folder, so it can't reach Radio."
        );
    }
}
#[test]
fn address_invalid_inputs_send_nothing() {
    for address in ["10.0.0.5:7341", "office.example", ""] {
        let fake = FakeRadio::answering(&[]);
        assert_eq!(
            set_address_with(&fake.radio(), address)
                .unwrap_err()
                .message,
            "That isn't a VPN address. Type the numbers only, like 10.0.0.5, with no port."
        );
        assert!(fake.requests().is_empty());
    }
}
#[test]
fn address_replaces_same_family_and_same_address_still_adds() {
    for old in ["192.0.2.4", "10.0.0.5"] {
        let mut answers = vec![json!({"ok":true,"port":7341,"addresses":[old,"2001:db8::1"]})];
        if old != "10.0.0.5" {
            answers.push(addresses());
        }
        answers.push(addresses());
        let fake = FakeRadio::answering(&answers);
        assert_eq!(
            set_address_with(&fake.radio(), " 10.0.0.5 ")
                .unwrap()
                .addresses,
            vec!["10.0.0.5"]
        );
        let requests = fake.requests();
        assert_eq!(requests[0], json!({"op":"link_address_list","v":1}));
        if old != "10.0.0.5" {
            assert_eq!(
                requests[1],
                json!({"op":"link_address_remove","v":1,"address":old})
            );
        }
        assert_eq!(
            requests.last().unwrap(),
            &json!({"op":"link_address_add","v":1,"address":"10.0.0.5"})
        );
    }
}

#[test]
fn local_join_guards_work_without_a_running_radio() {
    for (member, code, name, expected) in [
        (
            false,
            "rj1.fixture",
            "Work laptop",
            "This computer builds Wren. Make a join code here, then paste it on the other computer.",
        ),
        (
            true,
            "rj1.fixture",
            "bad\nname",
            "Give this computer a name of 1 to 64 characters, such as Work laptop, without symbols like @, #, $ or brackets.",
        ),
        (
            true,
            "wrong",
            "Work laptop",
            "That isn't a join code. Copy the whole code from the computer that builds your assistant, then try again.",
        ),
    ] {
        let home = Scratch::new();
        assistant(&home.0, "wren", "Wren", member);
        let before = listing(&home.0);
        assert_eq!(
            join(&home.0, "wren", code, name).unwrap_err().message,
            expected
        );
        assert_eq!(listing(&home.0), before);
    }
}

#[test]
fn every_bad_code_shape_is_refused_locally_without_writes() {
    let home = Scratch::new();
    assistant(&home.0, "wren", "Wren", true);
    let before = listing(&home.0);
    for code in [
        String::new(),
        "rj2.fixture".into(),
        "rj1.one\nrj1.two".into(),
        "rj1.one\rtwo".into(),
        format!("rj1.{}", "x".repeat(2045)),
    ] {
        assert_eq!(
            join(&home.0, "wren", &code, "Work laptop")
                .unwrap_err()
                .message,
            radio::text::NOT_A_CODE
        );
        assert_eq!(listing(&home.0), before);
    }
}

#[test]
fn invalid_address_and_name_need_no_running_radio() {
    let home = Scratch::new();
    let before = listing(&home.0);
    for address in ["10.0.0.5:7341", "office.example", ""] {
        assert_eq!(
            set_link_address(&home.0, address).unwrap_err().message,
            radio::text::NOT_AN_ADDRESS
        );
    }
    for invalid in ["", "Work\nlaptop", "@office"] {
        assert_eq!(
            name(&home.0, invalid).unwrap_err().message,
            radio::text::NOT_A_NAME
        );
    }
    assert_eq!(listing(&home.0), before);
}

#[test]
fn started_address_checks_hello_before_listing() {
    let home = Scratch::new();
    let fake = FakeRadio::answering(&[link_hello(), addresses()]);
    let answer = started_address(&home.0, &fake.radio()).unwrap();
    assert_eq!(answer.port, 7341);
    assert_eq!(answer.addresses, vec!["10.0.0.5"]);
    assert_eq!(answer.computer_name, None);
    assert_eq!(
        fake.requests(),
        vec![
            json!({"op":"hello","v":1}),
            json!({"op":"link_address_list","v":1}),
        ]
    );
}

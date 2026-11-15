mod common;
#[cfg(target_os = "macos")]
use common::*;
use voice_agent::console_key;

#[test]
fn console_keys_accept_only_the_console_key_shape() {
    for key in [
        format!("sk-ant-api03-{}", "a".repeat(40)),
        format!("sk-ant-api03-{}_a-", "a".repeat(40)),
    ] {
        assert!(console_key::valid(&key));
    }
    for key in [
        String::new(),
        "sk-ant-".into(),
        "api03-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        format!("sk-ant-admin01-{}", "a".repeat(40)),
        format!("sk-ant-api03-{} ", "a".repeat(40)),
        format!("sk-ant-api03-{}\"", "a".repeat(40)),
        format!("sk-ant-{}", "a".repeat(250)),
        format!("sk-ant-api03-{}é", "a".repeat(40)),
    ] {
        assert!(!console_key::valid(&key));
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    #[cfg(feature = "claude-subscription")]
    use std::sync::atomic::AtomicBool;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };
    #[cfg(feature = "claude-subscription")]
    use voice_agent::claude::SignIn;
    use voice_agent::{
        claude::{self, Claude},
        guard::Guard,
        text,
    };
    use voice_core::ceilings::Ceilings;
    use voice_save::secret::Secret;

    const KEY: &str = "sk-ant-api03-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const FILE: &str = ".voice-test-claude-console-key";
    fn key() -> Secret {
        Secret::new(KEY.into())
    }
    fn found(stub: &Stub, home: &Scratch) -> Claude {
        claude::find_in(&home.0, None, &[stub.program()]).unwrap()
    }
    fn stub() -> Stub {
        Stub::new(
            r#"case "$3 $4" in
'--version ') echo '2.1.292 (Claude Code)';;
'auth status')
    if [ -f "$here/subscription" ]; then
        echo '{"loggedIn":true,"authMethod":"claude.ai"}'
    elif [ "$(cat "$here/key$n")" = match ] && [ ! -f "$here/refuse" ]; then
        echo '{"loggedIn":true,"authMethod":"api_key"}'
    else echo '{"loggedIn":false}'; fi;;
'auth logout')
    [ ! -e "$HOME/.voice-test-claude-console-key" ] || exit 1
    rm -f "$here/subscription";;
'auth login') exit 0;;
*) cat "$here/reply";;
esac"#,
        )
    }

    #[test]
    fn saves_privately_loads_again_and_passes_the_key_to_checks_and_calls() {
        let home = Scratch::new("console-save");
        let stub = stub();
        let claude = found(&stub, &home).save_key(&home.0, key()).unwrap();
        assert!(claude.check(&home.0).is_ok());
        assert_eq!(
            fs::metadata(home.0.join(FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let claude = found(&stub, &home);
        assert!(claude.check(&home.0).is_ok());
        fs::write(
            stub.folder.0.join("reply"),
            ok_reply(&serde_json::json!({"labels":[]})),
        )
        .unwrap();
        let session = session(&home.0);
        let schema = schema();
        Guard::new(vec![&claude], Ceilings::defaults())
            .call(&session, &call("record one", &schema))
            .unwrap();
        assert_eq!(stub.file("key8"), "match\n");
        assert!(!format!("{claude:?}").contains(KEY));
    }

    #[test]
    fn sign_out_forgets_before_logout_and_a_later_find_is_signed_out() {
        let home = Scratch::new("console-forget");
        let stub = stub();
        let claude = found(&stub, &home).save_key(&home.0, key()).unwrap();
        claude.sign_out(&home.0).unwrap();
        assert!(!home.0.join(FILE).exists());
        assert_eq!(stub.argv(4).unwrap()[2..], ["auth", "logout"]);
        assert_eq!(stub.file("key4"), "unset\n");
        assert!(!found(&stub, &home).status(&home.0).unwrap().signed_in);
    }

    #[test]
    fn an_existing_run_stops_using_a_forgotten_or_unreadable_key() {
        for unreadable in [false, true] {
            let home = Scratch::new("console-live-forget");
            let stub = stub();
            let claude = found(&stub, &home).save_key(&home.0, key()).unwrap();
            found(&stub, &home).sign_out(&home.0).unwrap();
            if unreadable {
                fs::write(home.0.join(FILE), "bad").unwrap();
            }
            fs::write(
                stub.folder.0.join("reply"),
                ok_reply(&serde_json::json!({"labels":[]})),
            )
            .unwrap();
            let session = session(&home.0);
            let schema = schema();
            Guard::new(vec![&claude], Ceilings::defaults())
                .call(&session, &call("record one", &schema))
                .unwrap();
            assert_eq!(stub.file("key5"), "unset\n");
        }
    }

    #[test]
    fn stale_login_files_of_any_type_are_logged_out_before_saving() {
        for linked in [false, true] {
            let home = Scratch::new("console-stale");
            let stub = stub();
            let claude = found(&stub, &home);
            let credentials = claude.profile().join(".credentials.json");
            if linked {
                symlink(home.0.join("missing"), &credentials).unwrap();
            } else {
                fs::write(&credentials, "stale").unwrap();
            }
            let program = stub.program();
            let script = fs::read_to_string(&program).unwrap();
            fs::write(
                &program,
                script.replace(
                    "rm -f \"$here/subscription\"",
                    "rm -f \"$here/subscription\" \"$CLAUDE_CONFIG_DIR/.credentials.json\"",
                ),
            )
            .unwrap();
            found(&stub, &home).save_key(&home.0, key()).unwrap();
            assert_eq!(stub.argv(2).unwrap()[2..], ["auth", "logout"]);
            assert_eq!(stub.file("key2"), "unset\n");
            assert!(fs::symlink_metadata(credentials).is_err());
        }
    }

    #[test]
    fn failed_save_removes_the_store_and_refused_cleanup_failure_is_reported() {
        for refused in [false, true] {
            let home = Scratch::new("console-cleanup");
            let stub = stub();
            let program = stub.program();
            let script = fs::read_to_string(&program).unwrap();
            let sabotage = if refused {
                "rm -f \"$HOME/.voice-test-claude-console-key\"; mkdir \"$HOME/.voice-test-claude-console-key\"; touch \"$here/refuse\""
            } else {
                "ln -s \"$here/missing\" \"$HOME/.voice-test-claude-console-key\""
            };
            // Sabotage during the first status (save failure) or the second (refusal).
            let run = if refused { 3 } else { 1 };
            fs::write(
                &program,
                script.replace(
                    "'auth status')",
                    &format!("'auth status') if [ \"$n\" = {run} ]; then {sabotage}; fi"),
                ),
            )
            .unwrap();
            let error = found(&stub, &home).save_key(&home.0, key()).unwrap_err();
            assert_eq!(
                error.to_string(),
                if refused {
                    text::KEY_NOT_FORGOTTEN
                } else {
                    text::KEY_NOT_KEPT
                }
            );
            if !refused {
                assert!(fs::symlink_metadata(home.0.join(FILE)).is_err());
            }
        }
    }

    #[test]
    fn failed_save_reports_when_cleanup_also_fails() {
        let home = Scratch::new("console-save-cleanup-failure");
        let stub = stub();
        let program = stub.program();
        let script = fs::read_to_string(&program).unwrap();
        fs::write(
            &program,
            script.replace(
                "'auth status')",
                "'auth status') if [ \"$n\" = 1 ]; then mkdir \"$HOME/.voice-test-claude-console-key\"; fi",
            ),
        )
        .unwrap();
        let error = found(&stub, &home).save_key(&home.0, key()).unwrap_err();
        assert_eq!(error.to_string(), text::KEY_NOT_FORGOTTEN);
        assert!(home.0.join(FILE).is_dir());
    }

    #[test]
    fn sign_out_still_logs_out_when_forgetting_fails() {
        let home = Scratch::new("console-logout-forget-failure");
        let stub = stub();
        let claude = found(&stub, &home);
        fs::create_dir(home.0.join(FILE)).unwrap();
        let error = claude.sign_out(&home.0).unwrap_err();
        assert_eq!(error.to_string(), text::KEY_NOT_FORGOTTEN);
        assert_eq!(stub.argv(0).unwrap()[2..], ["auth", "logout"]);
    }

    #[test]
    fn a_refused_key_is_forgotten() {
        let home = Scratch::new("console-refused");
        let stub = stub();
        fs::write(stub.folder.0.join("refuse"), "").unwrap();
        assert_eq!(
            found(&stub, &home)
                .save_key(&home.0, key())
                .unwrap_err()
                .to_string(),
            text::KEY_REFUSED
        );
        assert!(!home.0.join(FILE).exists());
    }

    #[test]
    fn bad_shape_never_runs_or_saves() {
        let home = Scratch::new("console-shape");
        let stub = stub();
        assert_eq!(
            found(&stub, &home)
                .save_key(&home.0, Secret::new("bad".into()))
                .unwrap_err()
                .to_string(),
            text::KEY_SHAPE
        );
        assert!(!stub.ran());
        assert!(!home.0.join(FILE).exists());
    }

    #[cfg(not(feature = "claude-subscription"))]
    #[test]
    fn a_refused_subscription_logs_out_before_saving() {
        let home = Scratch::new("console-subscription");
        let stub = stub();
        fs::write(stub.folder.0.join("subscription"), "").unwrap();
        assert!(
            found(&stub, &home)
                .save_key(&home.0, key())
                .unwrap()
                .check(&home.0)
                .is_ok()
        );
        assert_eq!(stub.argv(2).unwrap()[2..], ["auth", "logout"]);
        assert_eq!(stub.file("key2"), "unset\n");
        assert_eq!(stub.file("key4"), "match\n");
    }

    #[test]
    fn invalid_and_linked_stores_are_unreadable() {
        for linked in [false, true] {
            let home = Scratch::new("console-unreadable");
            let stub = stub();
            if linked {
                fs::write(home.0.join("target"), KEY).unwrap();
                symlink(home.0.join("target"), home.0.join(FILE)).unwrap();
            } else {
                fs::write(home.0.join(FILE), "bad").unwrap();
            }
            assert_eq!(
                claude::find_in(&home.0, None, &[stub.program()])
                    .unwrap_err()
                    .to_string(),
                text::KEY_UNREADABLE
            );
            assert!(!stub.ran());
        }
    }

    #[cfg(feature = "claude-subscription")]
    #[test]
    fn browser_sign_in_does_not_get_the_saved_key() {
        let home = Scratch::new("console-browser");
        let stub = stub();
        let claude = found(&stub, &home).save_key(&home.0, key()).unwrap();
        claude
            .sign_in_with(
                &home.0,
                &AtomicBool::new(false),
                SignIn::Subscription,
                &|_| panic!("no link"),
            )
            .unwrap();
        assert_eq!(stub.file("key4"), "unset\n");
    }
}

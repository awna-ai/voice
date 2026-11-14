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

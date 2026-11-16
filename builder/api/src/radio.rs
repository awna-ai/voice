//! Voice and its Radio: the copy Voice.app carries, its install, and the socket ops that link computers.
//! Radio's contract is `CONTRACT.md` in `awna-ai/radio` at the tag `Cargo.toml` pins.
pub use radio_client::{Computer, Delivery, Fleet, Member};
use radio_client::{PROTOCOL_MAJOR, PROTOCOL_MINOR, Request, Response};
use std::{
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use voice_core::error::{Error, Result};

/// Voice's app name in Radio.
pub const APP: &str = "voice";
// Radio's install waits up to 120 s for another install's lock; leave it room to finish.
const INSTALL_WAIT: Duration = Duration::from_secs(180);
const PHRASE_MAX: usize = 200;
/// The ops Voice uses to link computers; `hello` refuses a Radio that lacks one.
const LINK_OPS: &[&str] = &[
    "hello",
    "register",
    "fleet_init",
    "code_create",
    "join",
    "link_address_add",
    "link_address_list",
    "link_address_remove",
    "computer_list",
    "send",
    "delivery_folder_set",
    "deliveries",
];

pub mod text {
    pub const NO_COPY: &str =
        "Voice can't find its copy of Radio. Install Voice again with install.sh, then try again.";
    pub const INSTALLED: &str = "Radio is installed and running.";
    pub const INSTALL_FAILED: &str = "Radio couldn't be installed on this computer. Try again; if it fails again, please report it.";
    pub const INSTALL_REFUSED: &str = "Radio refused to install for Voice. Another user may be able to change Voice where it is: move Voice to the Applications folder in your home folder, then try again.";
    pub const NO_SCREEN: &str = "Radio can only be installed while you're signed in at this computer's own screen. Sign in there, then try again.";
    pub const INSTALL_STOPPED: &str = "Radio stopped before it finished installing. Try again.";
    pub const INSTALL_TOO_LONG: &str =
        "Radio took too long to install, so Voice stopped it. Try again.";
    pub const NOT_RUNNING: &str = "Radio isn't running on this computer.";
    pub const NO_ANSWER: &str = "Radio didn't answer. Try again in a moment.";
    pub const REFUSED: &str = "Radio refused, without saying why.";
    pub const TOO_OLD: &str =
        "The Radio on this computer can't link computers. Install Voice again to update it.";
    pub const NO_COMPUTERS: &str = "Linked computers: none yet.";
    pub const OTHER_MAJOR: &str =
        "The Radio on this computer speaks a version this Voice doesn't. Install the newest Voice.";
    pub const NOT_TEXT: &str = "Voice can't hand Radio a file whose path isn't plain text.";
}

/// The copy of Radio beside Voice.app's `voice` command (PB8.9c1 puts it there). A release build
/// refuses a Voice that isn't in a place that lasts, as turning an assistant on does.
pub fn shipped_radio(home: &Path) -> Result<PathBuf> {
    radio_beside(&voice_core::cli_command::shipped_lasting_cli(home)?)
}

fn radio_beside(voice: &Path) -> Result<PathBuf> {
    let radio = voice.with_file_name("radio");
    if regular_file(&radio) {
        Ok(radio)
    } else {
        Err(Error::Invalid(text::NO_COPY.into()))
    }
}

// A link or a folder in a copy's place is a damaged copy.
fn regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file())
}

/// Installs or upgrades Radio for Voice, or adds Voice as a client of a newer Radio already there.
/// The handler is Voice's own `voice` command, which Radio records as the only program it runs for
/// Voice, so both come from a place that lasts.
pub fn install(home: &Path) -> Result<&'static str> {
    let voice = voice_core::cli_command::shipped_lasting_cli(home)?;
    install_within(&radio_beside(&voice)?, &voice, INSTALL_WAIT)
}

fn install_within(radio: &Path, handler: &Path, wait: Duration) -> Result<&'static str> {
    let invalid = |sentence: &str| Error::Invalid(sentence.into());
    // Only an absolute path: a bare name would be looked up on PATH.
    if !radio.is_absolute() || !regular_file(radio) {
        return Err(invalid(text::NO_COPY));
    }
    let mut child = Command::new(radio)
        .args([
            "install",
            "--app",
            APP,
            "--app-version",
            env!("CARGO_PKG_VERSION"),
            "--handler",
        ])
        .arg(handler)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Its own process group, so giving up stops whatever it started too.
        .process_group(0)
        .spawn()
        .map_err(|_| invalid(text::NO_COPY))?;
    let deadline = Instant::now() + wait;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            waited => {
                // SAFETY: kill takes no pointers; a negative pid names the child's own group.
                unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
                let _ = child.wait();
                return Err(invalid(if waited.is_err() {
                    text::INSTALL_STOPPED
                } else {
                    text::INSTALL_TOO_LONG
                }));
            }
        }
    };
    match status.code() {
        // 4: installed, but older rooms weren't brought in; Voice passes none.
        Some(0 | 4) => Ok(text::INSTALLED),
        Some(1) => Err(invalid(text::INSTALL_FAILED)),
        Some(2) => Err(invalid(text::INSTALL_REFUSED)),
        Some(3) => Err(invalid(text::NO_SCREEN)),
        _ => Err(invalid(text::INSTALL_STOPPED)),
    }
}

/// What `hello` says about the Radio that answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Hello {
    pub radio: String,
    pub major: u32,
    pub minor: u32,
}

/// A one-time join code. Its secret goes only to the person and to Radio's socket, so `Debug` hides
/// it and dropping it zeroes it. `radio_client`'s `Request::Join` and `Response` print it in `Debug`:
/// never log or `{:?}` either.
#[derive(Clone, PartialEq)]
pub struct JoinCode {
    code: String,
    pub id: String,
    pub expires_at: f64,
}

impl JoinCode {
    pub fn code(&self) -> &str {
        &self.code
    }
}

impl Drop for JoinCode {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.code);
    }
}

impl std::fmt::Debug for JoinCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinCode")
            .field("code", &"<hidden>")
            .field("id", &self.id)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// This computer's link addresses and the link's fixed port.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkAddresses {
    pub port: u16,
    pub addresses: Vec<String>,
}

/// Radio's socket, spoken through the link ops only.
#[derive(Debug, Clone)]
pub struct Radio {
    socket: PathBuf,
}

impl Radio {
    pub fn at(socket: PathBuf) -> Self {
        Self { socket }
    }

    /// The socket Radio listens on for this home folder.
    pub fn for_home(home: &Path) -> Self {
        Self::at(radio_client::socket_path(home))
    }

    fn ask(&self, request: Request) -> Result<Response> {
        let response = radio_client::request(&self.socket, &request).map_err(|phrase| {
            Error::Invalid(
                if phrase == radio_client::NOT_RUNNING {
                    text::NOT_RUNNING
                } else {
                    text::NO_ANSWER
                }
                .into(),
            )
        })?;
        if response.ok {
            Ok(response)
        } else if response.error.as_deref() == Some("unsupported_version") {
            Err(Error::Invalid(text::OTHER_MAJOR.into()))
        } else {
            Err(Error::Invalid(refusal(response.error.as_deref())))
        }
    }

    /// Radio's version, refusing one that can't link computers.
    pub fn hello(&self) -> Result<Hello> {
        let response = self.ask(Request::Hello {})?;
        let protocol = response.protocol.ok_or_else(no_answer)?;
        let ops = response.ops.unwrap_or_default();
        if protocol.major != PROTOCOL_MAJOR
            || protocol.minor < PROTOCOL_MINOR
            || !LINK_OPS.iter().all(|op| ops.iter().any(|have| have == op))
        {
            return Err(Error::Invalid(text::TOO_OLD.into()));
        }
        Ok(Hello {
            radio: response.radio.ok_or_else(no_answer)?,
            major: protocol.major,
            minor: protocol.minor,
        })
    }

    /// Replaces Voice's whole member list in Radio.
    pub fn register(&self, members: Vec<Member>) -> Result<()> {
        self.ask(Request::Register {
            app: APP.into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            members,
        })
        .map(drop)
    }

    /// Makes a fleet with this computer as its founder; also answers the recovery key's file.
    pub fn fleet_init(&self, name: &str) -> Result<(Fleet, String)> {
        let response = self.ask(Request::FleetInit { name: name.into() })?;
        Ok((
            response.fleet.ok_or_else(no_answer)?,
            response.recovery_file.ok_or_else(no_answer)?,
        ))
    }

    pub fn code_create(&self, name: Option<&str>, ttl: Option<u64>) -> Result<JoinCode> {
        let response = self.ask(Request::CodeCreate {
            name: name.map(str::to_owned),
            ttl,
        })?;
        Ok(JoinCode {
            code: response.code.ok_or_else(no_answer)?,
            id: response.code_id.ok_or_else(no_answer)?,
            expires_at: response.expires_at.ok_or_else(no_answer)?,
        })
    }

    /// Joins the fleet of the computer that made `code`. The code travels only through the socket.
    pub fn join(&self, code: &str) -> Result<Fleet> {
        self.ask(Request::Join { code: code.into() })?
            .fleet
            .ok_or_else(no_answer)
    }

    pub fn link_address_add(&self, address: &str) -> Result<LinkAddresses> {
        addresses(self.ask(Request::LinkAddressAdd {
            address: address.into(),
        })?)
    }

    pub fn link_address_list(&self) -> Result<LinkAddresses> {
        addresses(self.ask(Request::LinkAddressList {})?)
    }

    pub fn link_address_remove(&self, address: &str) -> Result<LinkAddresses> {
        addresses(self.ask(Request::LinkAddressRemove {
            address: address.into(),
        })?)
    }

    pub fn computer_list(&self) -> Result<Vec<Computer>> {
        self.ask(Request::ComputerList {})?
            .computers
            .ok_or_else(no_answer)
    }

    /// Sends one file to Voice on another computer of the fleet; answers the bytes sent.
    pub fn send(&self, computer: &str, name: &str, file: &Path) -> Result<u64> {
        self.ask(Request::Send {
            app: APP.into(),
            computer: computer.into(),
            name: name.into(),
            file: plain_path(file)?,
        })?
        .size
        .ok_or_else(no_answer)
    }

    /// Names the folder Radio makes for Voice's arrivals on this computer.
    pub fn delivery_folder_set(&self, folder: &Path) -> Result<()> {
        self.ask(Request::DeliveryFolderSet {
            app: APP.into(),
            folder: plain_path(folder)?,
        })
        .map(drop)
    }

    pub fn deliveries(&self, after: Option<u64>, limit: Option<u32>) -> Result<Vec<Delivery>> {
        self.ask(Request::Deliveries {
            app: APP.into(),
            after,
            limit,
        })?
        .deliveries
        .ok_or_else(no_answer)
    }
}

// Never a lossy path: that would name a different file.
fn plain_path(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::Invalid(text::NOT_TEXT.into()))
}

fn no_answer() -> Error {
    Error::Invalid(text::NO_ANSWER.into())
}

fn addresses(response: Response) -> Result<LinkAddresses> {
    Ok(LinkAddresses {
        port: response.port.ok_or_else(no_answer)?,
        addresses: response.addresses.ok_or_else(no_answer)?,
    })
}

/// Control characters and the invisible ones that reorder or hide text (zero-width, bidirectional,
/// the byte-order mark, the soft hyphen, tag characters).
fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{2069}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{E0000}'..='\u{E007F}'
        )
}

/// A line for the terminal, with each hidden character from the daemon shown as `?`.
fn shown(line: String) -> String {
    if line.chars().any(hidden) {
        line.chars()
            .map(|c| if hidden(c) { '?' } else { c })
            .collect()
    } else {
        line
    }
}

/// Radio's refusals are fixed ASCII phrases; show one only when it is one short line of them.
fn refusal(phrase: Option<&str>) -> String {
    match phrase {
        Some(phrase)
            if !phrase.is_empty()
                && phrase.chars().count() <= PHRASE_MAX
                && phrase.chars().all(|c| c == ' ' || c.is_ascii_graphic()) =>
        {
            format!("Radio said: {phrase}.")
        }
        _ => text::REFUSED.into(),
    }
}

/// `voice radio status`: where Voice's copy is, whether Radio answers, and the linked computers.
/// The flag is whether Radio answered.
pub fn status_lines(copy: Result<PathBuf>, radio: &Radio) -> (Vec<String>, bool) {
    let mut lines = vec![match copy {
        Ok(path) => format!("Voice's copy of Radio: {}", path.display()),
        Err(error) => format!("Voice's copy of Radio: {error}"),
    }];
    let hello = match radio.hello() {
        Ok(hello) => hello,
        Err(error) => {
            lines.push(error.to_string());
            return (lines, false);
        }
    };
    lines.push(format!(
        "Radio is running: version {}, protocol {}.{}.",
        hello.radio, hello.major, hello.minor
    ));
    lines.push(match radio.computer_list() {
        Ok(computers) if computers.is_empty() => text::NO_COMPUTERS.into(),
        Ok(computers) => format!(
            "Linked computers: {}.",
            computers
                .iter()
                .map(|computer| format!(
                    "{} ({})",
                    computer.name.as_deref().unwrap_or(&computer.id),
                    computer.state
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Err(error) => error.to_string(),
    });
    // Radio checks names, but every line is printed: never pass on a hidden character.
    (lines.into_iter().map(shown).collect(), true)
}

#[cfg(test)]
mod tests;

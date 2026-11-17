//! The Radio that links your computers.
use clap::Subcommand;
use std::io::{IsTerminal, Read, Write};
use voice_api::{
    dto, pairing,
    radio::{Radio, install, shipped_radio, status_lines, text},
    sending,
};
use voice_core::error::{Error, Result};
use zeroize::Zeroizing;

#[derive(Subcommand)]
pub enum RadioCommand {
    /// Show Voice's copy of Radio, whether it answers, and your linked computers
    Status,
    /// Install or update Radio from Voice's own copy
    Install,
    /// Show or set this computer's VPN address
    Address { ip: Option<String> },
    /// Make a join code for another computer
    Invite {
        #[arg(value_parser = checked_slug)]
        slug: String,
    },
    /// Join your other computers using a code read from standard input
    Join {
        #[arg(value_parser = checked_slug)]
        slug: String,
        #[arg(long)]
        name: String,
    },
    /// Send this computer’s read history to the computer that builds your assistant
    Send {
        #[arg(value_parser = checked_slug)]
        slug: String,
    },
    /// Take history sent by your other computers
    Take {
        #[arg(value_parser = checked_slug)]
        slug: String,
    },
    /// Show history sent and received by this computer
    History {
        #[arg(value_parser = checked_slug)]
        slug: String,
    },
    /// Name this computer
    Name { name: String },
}

fn checked_slug(slug: &str) -> std::result::Result<String, String> {
    if voice_core::crew::slugify(slug).as_deref() == Some(slug) {
        Ok(slug.into())
    } else {
        Err("Use the assistant's folder name, such as wren.".into())
    }
}

fn failure(error: dto::Failure) -> Error {
    Error::Invalid(error.message)
}

fn address_line(addresses: &[String], port: u16) -> String {
    if addresses.is_empty() {
        "This computer has no VPN address yet.".into()
    } else {
        format!(
            "This computer's VPN address: {} (port {port}).",
            addresses.join(", ")
        )
    }
}

fn local_time(expires_at: f64) -> Option<String> {
    let timestamp = expires_at as libc::time_t;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers refer to live storage; localtime_r initializes local on success.
    if unsafe { libc::localtime_r(&timestamp, local.as_mut_ptr()) }.is_null() {
        return None;
    }
    // SAFETY: localtime_r succeeded and initialized every field.
    let local = unsafe { local.assume_init() };
    Some(format!("{:02}:{:02}", local.tm_hour, local.tm_min))
}

fn invite_lines(invite: &dto::Invite) -> Vec<String> {
    let expiry = match local_time(invite.expires_at) {
        Some(until) => format!("It works once, until {until}."),
        None => "It works once, for a short while.".into(),
    };
    let mut lines = vec![
        address_line(&invite.addresses, invite.port),
        format!("Join code: {}", invite.code),
        expiry,
    ];
    if let Some(path) = &invite.recovery_file {
        lines.push(text::recovery(path));
    }
    lines
}

// History shows a clock time only on the same local calendar day.
fn local_date(at: f64) -> Option<String> {
    let timestamp = at as libc::time_t;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as libc::time_t;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    let mut today = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: all pointers refer to live storage; localtime_r initializes each output on success.
    if unsafe { libc::localtime_r(&timestamp, local.as_mut_ptr()) }.is_null()
        || unsafe { libc::localtime_r(&now, today.as_mut_ptr()) }.is_null()
    {
        return None;
    }
    // SAFETY: both calls succeeded and initialized every field.
    let (local, today) = unsafe { (local.assume_init(), today.assume_init()) };
    if local.tm_year == today.tm_year && local.tm_yday == today.tm_yday {
        local_time(at)
    } else {
        Some(format!(
            "{:04}-{:02}-{:02}",
            local.tm_year + 1900,
            local.tm_mon + 1,
            local.tm_mday
        ))
    }
}

fn shown_time(at: f64) -> String {
    local_date(at).unwrap_or_else(|| "unknown time".into())
}

fn print_computers(computers: &[dto::SentFrom]) {
    for computer in computers {
        println!(
            "{}: {} records, last {}",
            computer.name,
            computer.records,
            shown_time(computer.at)
        );
    }
}

static SAVED_TERMINAL: std::sync::atomic::AtomicPtr<libc::termios> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
const TERMINAL_SIGNALS: [libc::c_int; 4] =
    [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT];

extern "C" fn interrupted(signal: libc::c_int) {
    let saved = SAVED_TERMINAL.load(std::sync::atomic::Ordering::SeqCst);
    if !saved.is_null() {
        // SAFETY: the guard owns this stable allocation while its handlers are installed.
        // Only async-signal-safe libc calls run here; the handler never returns.
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, saved);
        }
    }
    // SAFETY: _exit terminates without running non-signal-safe cleanup or logging.
    unsafe {
        libc::_exit(128 + signal);
    }
}

struct EchoGuard {
    saved: Box<libc::termios>,
    handlers: Vec<(libc::c_int, libc::sigaction)>,
}
impl EchoGuard {
    fn hide() -> std::io::Result<Self> {
        let mut saved = std::mem::MaybeUninit::<libc::termios>::uninit();
        // SAFETY: saved points to valid storage initialized by tcgetattr on success.
        if unsafe { libc::tcgetattr(0, saved.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut guard = Self {
            // SAFETY: tcgetattr succeeded and initialized saved.
            saved: Box::new(unsafe { saved.assume_init() }),
            handlers: Vec::with_capacity(TERMINAL_SIGNALS.len()),
        };
        SAVED_TERMINAL.store(&mut *guard.saved, std::sync::atomic::Ordering::SeqCst);
        // SAFETY: zero initializes sigaction's flags and optional fields; its mask and
        // handler are set below before installing it.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = interrupted as *const () as libc::sighandler_t;
        // SAFETY: action's mask is live storage; each signal is a valid constant.
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
            for signal in TERMINAL_SIGNALS {
                libc::sigaddset(&mut action.sa_mask, signal);
            }
        }
        for signal in TERMINAL_SIGNALS {
            let mut previous = std::mem::MaybeUninit::<libc::sigaction>::uninit();
            // SAFETY: both action pointers are valid; sigaction initializes previous on success.
            if unsafe { libc::sigaction(signal, &action, previous.as_mut_ptr()) } != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // SAFETY: sigaction succeeded and initialized previous.
            guard
                .handlers
                .push((signal, unsafe { previous.assume_init() }));
        }
        let mut hidden = *guard.saved;
        hidden.c_lflag &= !libc::ECHO;
        // SAFETY: hidden holds valid settings with ECHO cleared; guard already restores them.
        if unsafe { libc::tcsetattr(0, libc::TCSANOW, &hidden) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(guard)
    }
}
impl Drop for EchoGuard {
    fn drop(&mut self) {
        // SAFETY: these settings came from stdin, which stays open during the read.
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &*self.saved);
        }
        // This guard runs on the CLI's main thread before pairing starts. A handler on
        // this thread either exits with the live allocation or sees the cleared pointer.
        SAVED_TERMINAL.store(std::ptr::null_mut(), std::sync::atomic::Ordering::SeqCst);
        for (signal, previous) in &self.handlers {
            // SAFETY: previous is the unchanged action returned when this handler was installed.
            unsafe {
                libc::sigaction(*signal, previous, std::ptr::null_mut());
            }
        }
        eprintln!();
    }
}

fn read_code() -> Result<Zeroizing<String>> {
    let read_failed = || Error::Invalid("Voice couldn't read the join code. Try again.".into());
    let _echo = if std::io::stdin().is_terminal() {
        let guard = EchoGuard::hide().map_err(|_| read_failed())?;
        eprintln!("Paste the join code, then press Return:");
        std::io::stderr().flush().map_err(|_| read_failed())?;
        Some(guard)
    } else {
        None
    };
    use std::os::fd::BorrowedFd;
    // SAFETY: stdin stays open while its descriptor is borrowed and duplicated.
    let fd = unsafe { BorrowedFd::borrow_raw(0) }
        .try_clone_to_owned()
        .map_err(|_| read_failed())?;
    let mut stdin = std::fs::File::from(fd).take(4096);
    let mut bytes = Zeroizing::new(Vec::with_capacity(4096));
    let mut byte = Zeroizing::new([0u8; 1]);
    loop {
        match stdin.read(byte.as_mut()) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => bytes.push(byte[0]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(read_failed()),
        }
    }
    let code = std::str::from_utf8(&bytes).map_err(|_| Error::Invalid(text::NOT_A_CODE.into()))?;
    Ok(Zeroizing::new(code.into()))
}

pub fn run(command: RadioCommand) -> Result<std::process::ExitCode> {
    let home = crate::home()?;
    match command {
        RadioCommand::Status => {
            let (lines, answers) = status_lines(shipped_radio(&home), &Radio::for_home(&home));
            for line in lines {
                println!("{line}");
            }
            return Ok(if answers {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::FAILURE
            });
        }
        RadioCommand::Install => println!("{}", install(&home)?),
        RadioCommand::Address { ip } => {
            let address = match ip {
                Some(ip) => pairing::set_link_address(&home, &ip),
                None => pairing::link_address(&home),
            }
            .map_err(failure)?;
            println!("{}", address_line(&address.addresses, address.port));
        }
        RadioCommand::Invite { slug } => {
            let invite = pairing::invite(&home, &slug).map_err(failure)?;
            for line in invite_lines(&invite) {
                println!("{line}");
            }
        }
        RadioCommand::Join { slug, name } => {
            let code = read_code()?;
            let joined = pairing::join(&home, &slug, &code, &name).map_err(failure)?;
            println!("This computer joined as {}.", joined.computer_name);
        }
        RadioCommand::Send { slug } => {
            let sent = sending::send_history(&home, &slug).map_err(failure)?;
            println!("Sent this computer's history to {}.", sent.to);
        }
        RadioCommand::Take { slug } => {
            let taken = sending::take_deliveries(&home, &slug).map_err(failure)?;
            println!("Took {} deliveries.", taken.taken);
            print_computers(&taken.computers);
            if taken.refused > 0 {
                println!(
                    "Left out {} files that weren't Voice's history.",
                    taken.refused
                );
            }
        }
        RadioCommand::History { slug } => {
            let status = sending::history_status(&home, &slug).map_err(failure)?;
            if status.pending {
                println!("This computer's history is waiting to be sent.");
            }
            if let Some(sent) = &status.last_sent {
                println!("Last sent to {}, at {}.", sent.to, shown_time(sent.at));
            }
            print_computers(&status.computers);
            if !status.pending && status.last_sent.is_none() && status.computers.is_empty() {
                println!("No history has been sent or received yet.");
            }
        }
        RadioCommand::Name { name } => {
            pairing::name(&home, &name).map_err(failure)?;
            println!("This computer is now called {name}.");
        }
    }
    Ok(std::process::ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invite(expires_at: f64) -> dto::Invite {
        dto::Invite {
            code: "test-join-code".into(),
            expires_at,
            port: 43123,
            addresses: vec!["192.0.2.1".into()],
            recovery_file: Some("radio-recovery.key".into()),
        }
    }

    #[test]
    fn invite_lines_keep_recovery_when_time_cannot_be_read() {
        let invite = invite(1e300);
        let lines = invite_lines(&invite);

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], address_line(&invite.addresses, invite.port));
        assert_eq!(lines[1], "Join code: test-join-code");
        assert_eq!(lines[2], "It works once, for a short while.");
        assert_eq!(
            lines.last(),
            Some(&text::recovery(invite.recovery_file.as_deref().unwrap()))
        );
    }

    #[test]
    fn invite_lines_show_normal_time_before_recovery() {
        let invite = invite(1_700_000_000.0);
        let lines = invite_lines(&invite);

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], address_line(&invite.addresses, invite.port));
        assert_eq!(lines[1], "Join code: test-join-code");
        assert!(lines[2].starts_with("It works once, until "));
        assert_eq!(
            lines.last(),
            Some(&text::recovery(invite.recovery_file.as_deref().unwrap()))
        );
    }
}

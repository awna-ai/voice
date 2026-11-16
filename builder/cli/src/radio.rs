//! The Radio that links your computers: `voice radio status`.
use clap::Subcommand;
use voice_api::radio::{Radio, shipped_radio, status_lines};
use voice_core::error::Result;

#[derive(Subcommand)]
pub enum RadioCommand {
    /// Show Voice's copy of Radio, whether it answers, and your linked computers
    Status,
}

pub fn run(command: RadioCommand) -> Result<std::process::ExitCode> {
    match command {
        RadioCommand::Status => {
            let home = crate::home()?;
            let (lines, answers) = status_lines(shipped_radio(&home), &Radio::for_home(&home));
            for line in lines {
                println!("{line}");
            }
            Ok(if answers {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::FAILURE
            })
        }
    }
}

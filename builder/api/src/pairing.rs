//! Linking another computer through Voice's Radio. Join codes travel only to the person and Radio.
use crate::{
    assistants::{failure, listed},
    dto,
    radio::{self, AskError, Radio, text},
};
use std::{
    net::IpAddr,
    path::{Path, PathBuf},
};
use voice_core::{error::Error, member};
use voice_save::record::{self, Role, SaveRecord};
type Result<T> = std::result::Result<T, dto::Failure>;

fn radio_error(error: Error) -> dto::Failure {
    // Radio's errors are already fixed sentences, including its screened refusal phrases.
    match error {
        Error::Invalid(sentence) => failure(&sentence),
        _ => failure(text::NO_ANSWER),
    }
}
fn phrase_error(error: AskError) -> dto::Failure {
    radio_error(error.sentence())
}
fn addresses(answer: radio::LinkAddresses) -> dto::LinkAddress {
    dto::LinkAddress {
        port: answer.port,
        addresses: answer.addresses,
    }
}
pub fn link_address(home: &Path) -> Result<dto::LinkAddress> {
    Radio::for_home(home)
        .link_address_list()
        .map(addresses)
        .map_err(radio_error)
}
pub fn set_link_address(home: &Path, address: &str) -> Result<dto::LinkAddress> {
    set_address_in(home, &Radio::for_home(home), address)
}
fn set_address_in(home: &Path, radio: &Radio, address: &str) -> Result<dto::LinkAddress> {
    let address = parse_address(address)?;
    ready(home, radio)?;
    replace_address(radio, address)
}
#[cfg(test)]
fn set_address_with(radio: &Radio, address: &str) -> Result<dto::LinkAddress> {
    replace_address(radio, parse_address(address)?)
}
fn parse_address(address: &str) -> Result<IpAddr> {
    let address: IpAddr = address
        .trim()
        .parse()
        .map_err(|_| failure(text::NOT_AN_ADDRESS))?;
    // Radio stores IPv4-mapped IPv6 as IPv4: compare the family it actually keeps.
    let address = match address {
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(address),
        _ => address,
    };
    let refused = match address {
        IpAddr::V4(ip) => ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast(),
        IpAddr::V6(ip) => ip.is_unspecified() || ip.is_multicast() || ip.is_unicast_link_local(),
    };
    if refused {
        return Err(failure(text::NOT_AN_ADDRESS));
    }
    Ok(address)
}
fn replace_address(radio: &Radio, address: IpAddr) -> Result<dto::LinkAddress> {
    let old = radio.link_address_list().map_err(radio_error)?;
    for previous in old.addresses {
        if let Ok(ip) = previous.parse::<IpAddr>()
            && ip.is_ipv4() == address.is_ipv4()
            && ip != address
        {
            radio.link_address_remove(&previous).map_err(radio_error)?;
        }
    }
    radio
        .link_address_add(&address.to_string())
        .map(addresses)
        .map_err(radio_error)
}
fn assistant(home: &Path, slug: &str) -> Result<(member::Member, PathBuf, Option<SaveRecord>)> {
    let assistant = listed(home, slug)?.member;
    let state = member::private_state_dir(&assistant, home)
        .ok_or_else(|| failure(crate::history::ELSEWHERE))?;
    let record =
        record::read_record(&state).map_err(|_| failure(crate::save::OPEN_SAVED_FAILED))?;
    Ok((assistant, state, record))
}
fn ready(home: &Path, radio: &Radio) -> Result<()> {
    match radio.hello() {
        Ok(_) => Ok(()),
        Err(Error::Invalid(sentence)) if sentence == text::NOT_RUNNING => {
            radio::install(home).map_err(radio_error)?;
            radio.hello().map(drop).map_err(radio_error)
        }
        Err(error) => Err(radio_error(error)),
    }
}
fn register(home: &Path, radio: &Radio) -> Result<dto::LinkAddress> {
    ready(home, radio)?;
    let address = radio
        .link_address_list()
        .map(addresses)
        .map_err(radio_error)?;
    if address.addresses.is_empty() {
        return Err(failure(text::NEEDS_ADDRESS));
    }
    // VO16.6b2 (panels on Radio) owns this list and must keep calling with every member.
    radio.register(vec![]).map_err(radio_error)?;
    Ok(address)
}
pub fn invite(home: &Path, slug: &str) -> Result<dto::Invite> {
    invite_with(home, slug, &Radio::for_home(home))
}
fn invite_with(home: &Path, slug: &str, radio: &Radio) -> Result<dto::Invite> {
    let (assistant, state, record) = assistant(home, slug)?;
    if let Some(SaveRecord::Github {
        role: Role::Member,
        builder,
        ..
    }) = record
    {
        return Err(failure(&match builder {
            Some(builder) => format!(
                "Add another computer from {builder}, the computer that builds {}.",
                assistant.name
            ),
            None => format!(
                "Add another computer from the computer that builds {}.",
                assistant.name
            ),
        }));
    }
    for other in voice_core::assistants::list(home).map_err(|_| failure(crate::assistants::FIND))? {
        if other.member.slug != slug
            && let Some(state) = member::private_state_dir(&other.member, home)
            && std::fs::symlink_metadata(state.join("deliveries")).is_ok()
        {
            return Err(failure(&format!(
                "{} already takes the history your other computers send to this one. Voice does this for one assistant on each computer for now.",
                other.member.name
            )));
        }
    }
    let address = register(home, radio)?;
    let folder = state.join("deliveries");
    match radio.delivery_folder_set_phrase(&folder) {
        Ok(()) => (),
        Err(AskError::Refused(phrase)) if phrase == "the folder already exists; name a new one" => {
            let shown = folder
                .strip_prefix(home)
                .map(|path| format!("~/{}", path.display()))
                .unwrap_or_else(|_| folder.display().to_string());
            return Err(failure(&format!(
                "{shown} is already there, and Radio didn't make it. Move it somewhere else, then try again."
            )));
        }
        Err(error) => return Err(phrase_error(error)),
    }
    let recovery_file = match radio.fleet_init_phrase("voice") {
        Ok((_, file)) => Some(file),
        Err(AskError::Refused(phrase)) if phrase == "already in a fleet" => None,
        Err(error) => return Err(phrase_error(error)),
    };
    let code = radio.code_create(None, None).map_err(|error| {
        let mut error = radio_error(error);
        if let Some(file) = &recovery_file {
            error.message.push(' ');
            error.message.push_str(&text::recovery(file));
        }
        error
    })?;
    Ok(dto::Invite {
        code: code.code().into(),
        expires_at: code.expires_at,
        port: address.port,
        addresses: address.addresses,
        recovery_file,
    })
}
pub fn join(home: &Path, slug: &str, code: &str, name: &str) -> Result<dto::Joined> {
    join_with(home, slug, code, name, &Radio::for_home(home))
}
fn join_with(
    home: &Path,
    slug: &str,
    code: &str,
    name: &str,
    radio: &Radio,
) -> Result<dto::Joined> {
    let (assistant, _, record) = assistant(home, slug)?;
    if !matches!(
        record,
        Some(SaveRecord::Github {
            role: Role::Member,
            ..
        })
    ) {
        return Err(failure(&format!(
            "This computer builds {}. Make a join code here, then paste it on the other computer.",
            assistant.name
        )));
    }
    valid_name(name)?;
    let code = code.trim();
    if code.len() > 2048 || !code.starts_with("rj1.") || code.contains(['\n', '\r']) {
        return Err(failure(text::NOT_A_CODE));
    }
    register(home, radio)?;
    // No Voice writes before Radio accepts the code; the name goes only into its signed record.
    let fleet = radio.join_phrase(code).map_err(|error| {
        if let AskError::Refused(phrase) = &error {
            let sentence = match phrase.as_str() {
                "not a join code" => Some(text::NOT_A_CODE),
                "the join was refused" => Some(text::CODE_REFUSED),
                "the computer could not be reached" | "the computer did not answer" => {
                    Some(text::NOT_REACHED)
                }
                "the join code has no address to dial" => Some(text::CODE_NO_ADDRESS),
                "already in a fleet" => Some(text::ALREADY_LINKED),
                _ => None,
            };
            if let Some(sentence) = sentence {
                return failure(sentence);
            }
        }
        phrase_error(error)
    })?;
    resign(radio, name).map_err(|error| failure(&format!(
        "This computer joined, but Voice couldn't save its name: {}. Try again with: voice radio name \"{name}\"",
        error.message.trim_end_matches('.')
    )))?;
    Ok(dto::Joined {
        fleet: fleet.name,
        computer_name: name.into(),
    })
}
fn valid_name(name: &str) -> Result<()> {
    if voice_save::computer::valid_computer_name(name) {
        Ok(())
    } else {
        Err(failure(text::NOT_A_NAME))
    }
}
fn resign(radio: &Radio, name: &str) -> Result<()> {
    let section = serde_json::json!({"computer_name": name}).to_string();
    radio.resign_section(&section).map_err(radio_error)
}
pub fn name(home: &Path, name: &str) -> Result<()> {
    name_in(home, &Radio::for_home(home), name)
}
fn name_in(home: &Path, radio: &Radio, name: &str) -> Result<()> {
    valid_name(name)?;
    ready(home, radio)?;
    // VO16.6b2 (panels on Radio) owns this list and must keep calling with every member.
    radio.register(vec![]).map_err(radio_error)?;
    let section = serde_json::json!({"computer_name": name}).to_string();
    radio.resign_section_phrase(&section).map_err(|error| {
        if let AskError::Refused(phrase) = &error
            && phrase == "not in a fleet"
        {
            return failure(text::NOT_LINKED);
        }
        phrase_error(error)
    })
}

pub fn radio_link_address() -> Result<dto::LinkAddress> {
    link_address(&crate::assistants::home(text::NO_HOME)?)
}
pub fn radio_set_link_address(r: dto::RadioAddressRequest) -> Result<dto::LinkAddress> {
    set_link_address(&crate::assistants::home(text::NO_HOME)?, &r.address)
}
pub fn radio_invite(r: dto::SlugRequest) -> Result<dto::Invite> {
    invite(&crate::assistants::home(text::NO_HOME)?, &r.slug)
}
pub fn radio_join(r: dto::RadioJoinRequest) -> Result<dto::Joined> {
    join(
        &crate::assistants::home(text::NO_HOME)?,
        &r.slug,
        &r.code,
        &r.name,
    )
}
pub fn radio_name(r: dto::RadioNameRequest) -> Result<()> {
    name(&crate::assistants::home(text::NO_HOME)?, &r.name)
}
#[cfg(test)]
mod tests;

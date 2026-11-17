//! This computer's filtered history travels only to the fleet's build computer over Radio.
use crate::{
    assistants::failure,
    dto, pairing,
    radio::{AskError, Radio, text},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use voice_ingest::{corpus, filters::Filters, privacy, private};
use voice_save::record::{Role, SaveRecord};
type Result<T> = std::result::Result<T, dto::Failure>;
const FILE_LIMIT: u64 = 64 * 1024 * 1024;
const HISTORY_FAILED: &str = "Voice couldn't open this computer's history. Try reading it again.";
const TAKE_FAILED: &str = "Voice couldn't take the history your other computers sent. Try again.";
const STATUS_FAILED: &str = "Voice couldn't read the history status. Try again.";
const SEND_FAILED: &str = "Voice couldn't save this computer's history status. Try again.";

#[derive(Default, Serialize, Deserialize)]
struct Received {
    after: u64,
    computers: BTreeMap<String, dto::SentFrom>,
}
#[derive(Serialize, Deserialize)]
struct LastSent {
    at: f64,
    to: String,
}
fn now() -> std::result::Result<std::time::Duration, dto::Failure> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| failure(SEND_FAILED))
}
fn is_member(record: &Option<SaveRecord>) -> bool {
    matches!(
        record,
        Some(SaveRecord::Github {
            role: Role::Member,
            ..
        })
    )
}
fn read_json<T: serde::de::DeserializeOwned>(
    home: &Path,
    slug: &str,
    name: &str,
) -> Result<Option<T>> {
    private::read_private_optional(home, slug, name)
        .map_err(|_| failure(STATUS_FAILED))?
        .map(|bytes| serde_json::from_slice(&bytes).map_err(|_| failure(STATUS_FAILED)))
        .transpose()
}
fn save_json(home: &Path, slug: &str, name: &str, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| failure(SEND_FAILED))?;
    private::write_private(home, slug, name, &bytes)
        .map(drop)
        .map_err(|_| failure(SEND_FAILED))
}
fn founder(computers: Vec<radio_client::Computer>) -> Result<radio_client::Computer> {
    let mut founders = computers
        .into_iter()
        .filter(|c| c.state == "accepted" && c.invited_by.is_none());
    let first = founders
        .next()
        .ok_or_else(|| failure(text::CANT_FIND_BUILDER))?;
    if founders.next().is_some() {
        return Err(failure(text::CANT_FIND_BUILDER));
    }
    Ok(first)
}
fn send_error(error: AskError) -> dto::Failure {
    if let AskError::Refused(phrase) = &error {
        let sentence = match phrase.as_str() {
            "the computer could not be reached" | "the computer did not answer" => {
                Some(text::NOT_REACHED_BUILDER)
            }
            "that app takes no files on that computer" => Some(text::BUILDER_NOT_READY),
            "that computer is busy; try again" | "a file with that name is already there" => {
                Some(text::BUSY_BUILDER)
            }
            _ => None,
        };
        if let Some(sentence) = sentence {
            return failure(sentence);
        }
    }
    pairing::phrase_error(error)
}
pub fn send_history(home: &Path, slug: &str) -> Result<dto::Sent> {
    send_with(home, slug, &Radio::for_home(home))
}
fn send_with(home: &Path, slug: &str, radio: &Radio) -> Result<dto::Sent> {
    let (assistant, _, record) = pairing::assistant(home, slug)?;
    if !is_member(&record) {
        return Err(failure(&format!(
            "This computer builds {}. Your other computers send their history here.",
            assistant.name
        )));
    }
    let history = private::read_private_optional(home, slug, "decisions-claude-code.jsonl")
        .map_err(|_| failure(HISTORY_FAILED))?
        .ok_or_else(|| {
            failure(&format!(
                "Read this computer's Claude Code history for {} first, on the Your history step.",
                assistant.name
            ))
        })?;
    let _lock = private::lock_state(home, slug).map_err(|_| failure(SEND_FAILED))?;
    pairing::register(home, radio)?;
    let builder = founder(radio.computer_list().map_err(pairing::radio_error)?)?;
    let pending = private::write_private(home, slug, "pending-send.jsonl", &history)
        .map_err(|_| failure(SEND_FAILED))?;
    let name = format!("history-{}-{}.jsonl", now()?.as_nanos(), std::process::id());
    let sent = radio.send_phrase(&builder.id, &name, &pending);
    finish_send(home, slug, builder.name.unwrap_or(builder.id), sent)
}
fn finish_send(
    home: &Path,
    slug: &str,
    to: String,
    sent: std::result::Result<u64, AskError>,
) -> Result<dto::Sent> {
    let bytes = sent.map_err(send_error)?;
    private::remove_private(home, slug, "pending-send.jsonl").map_err(|_| failure(SEND_FAILED))?;
    let at = now()?.as_secs_f64();
    save_json(
        home,
        slug,
        "last-sent.json",
        &LastSent { at, to: to.clone() },
    )?;
    Ok(dto::Sent { to, at, bytes })
}
fn history_name(name: &str) -> bool {
    let Some(parts) = name
        .strip_prefix("history-")
        .and_then(|s| s.strip_suffix(".jsonl"))
    else {
        return false;
    };
    let Some((time, pid)) = parts.split_once('-') else {
        return false;
    };
    [(time, 20), (pid, 10)]
        .iter()
        .all(|(s, max)| !s.is_empty() && s.len() <= *max && s.bytes().all(|b| b.is_ascii_digit()))
}
fn peer_file(key: &str) -> Option<String> {
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("decisions-from-{}.jsonl", &key[..16]))
}
/// Preserve existing records verbatim; only new ids pass through this member's privacy filters.
fn fold(filters: &Filters, old: &[u8], delivered: &[u8]) -> voice_core::error::Result<Vec<u8>> {
    let mut kept: Vec<Map<String, Value>> = corpus::read_records("history", old)?
        .into_iter()
        .map(|(_, r)| r)
        .collect();
    let mut ids: HashSet<String> = kept
        .iter()
        .filter_map(|r| r.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect();
    for (_, record) in corpus::read_records("history", delivered)? {
        let Some(id) = record.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !ids.insert(id.into()) {
            continue;
        }
        let value = Value::Object(record);
        if privacy::privacy(filters, &value)?.is_some() {
            continue;
        }
        if value.get("timestamp").and_then(Value::as_str).is_none() {
            return Err(voice_core::error::Error::Invalid(
                "A history record has no date.".into(),
            ));
        }
        let Value::Object(record) = value else {
            unreachable!()
        };
        kept.push(record);
    }
    kept.sort_by(|a, b| {
        (a["timestamp"].as_str(), a["id"].as_str())
            .cmp(&(b["timestamp"].as_str(), b["id"].as_str()))
    });
    let mut bytes = Vec::new();
    for record in kept {
        // JSON values already parsed by read_records are serializable.
        serde_json::to_writer(&mut bytes, &record).expect("parsed history record");
        bytes.push(b'\n');
    }
    Ok(bytes)
}
pub fn take_deliveries(home: &Path, slug: &str) -> Result<dto::Taken> {
    take_with(home, slug, &Radio::for_home(home))
}
fn take_with(home: &Path, slug: &str, radio: &Radio) -> Result<dto::Taken> {
    let (assistant, _, record) = pairing::assistant(home, slug)?;
    if is_member(&record) {
        return Err(failure(&format!(
            "This computer sends its history to the computer that builds {}; take deliveries there.",
            assistant.name
        )));
    }
    let _lock = private::lock_state(home, slug).map_err(|_| failure(TAKE_FAILED))?;
    let filters = Filters::compile(&assistant.filters).map_err(|_| failure(TAKE_FAILED))?;
    let mut saved: Received = read_json(home, slug, "deliveries-taken.json")?.unwrap_or_default();
    // Radio's deliveries op requires the app to be registered, even when it has no members.
    pairing::register(home, radio)?;
    let mut answer = dto::Taken {
        taken: 0,
        refused: 0,
        computers: vec![],
    };
    loop {
        let deliveries = radio
            .deliveries(Some(saved.after), Some(500))
            .map_err(pairing::radio_error)?;
        if deliveries.is_empty() {
            break;
        }
        for delivery in deliveries {
            take_one(home, slug, &filters, &mut saved, &mut answer, &delivery)?;
        }
    }
    answer.computers = saved.computers.into_values().collect();
    Ok(answer)
}
fn take_one(
    home: &Path,
    slug: &str,
    filters: &Filters,
    saved: &mut Received,
    answer: &mut dto::Taken,
    delivery: &radio_client::Delivery,
) -> Result<()> {
    let after = u64::try_from(delivery.id)
        .ok()
        .filter(|id| *id > saved.after)
        .ok_or_else(|| failure(TAKE_FAILED))?;
    // Radio guarantees a plain file name. Refuse malformed replies without constructing any path.
    if delivery.name.is_empty()
        || delivery.name.starts_with('.')
        || !delivery
            .name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        answer.refused += 1;
    } else {
        let file = peer_file(&delivery.key);
        let incoming = if history_name(&delivery.name) && file.is_some() {
            private::read_private_optional_at_max(
                home,
                slug,
                Some("deliveries"),
                &delivery.name,
                FILE_LIMIT,
            )
        } else {
            Err(voice_core::error::Error::Invalid(
                "Not a history file.".into(),
            ))
        };
        match incoming {
            Ok(None) => (),
            Ok(Some(bytes)) => {
                let file = file.expect("checked peer key");
                let old = private::read_private_optional(home, slug, &file)
                    .map_err(|_| failure(TAKE_FAILED))?
                    .unwrap_or_default();
                match fold(filters, &old, &bytes) {
                    Ok(merged) => {
                        let records = corpus::read_records("history", &merged)
                            .map_err(|_| failure(TAKE_FAILED))?
                            .len() as u64;
                        private::write_private(home, slug, &file, &merged)
                            .map_err(|_| failure(TAKE_FAILED))?;
                        saved.computers.insert(
                            delivery.key.clone(),
                            dto::SentFrom {
                                name: delivery.computer.clone(),
                                at: delivery.at,
                                records,
                            },
                        );
                        answer.taken += 1;
                    }
                    Err(_) => answer.refused += 1,
                }
            }
            Err(_) => answer.refused += 1,
        }
        private::remove_private_entry_at(home, slug, "deliveries", &delivery.name)
            .map_err(|_| failure(TAKE_FAILED))?;
    }
    saved.after = after;
    save_json(home, slug, "deliveries-taken.json", saved)
}
pub fn history_status(home: &Path, slug: &str) -> Result<dto::HistoryStatus> {
    pairing::assistant(home, slug)?;
    let last: Option<LastSent> = read_json(home, slug, "last-sent.json")?;
    let received: Received = read_json(home, slug, "deliveries-taken.json")?.unwrap_or_default();
    Ok(dto::HistoryStatus {
        pending: private::read_private_optional(home, slug, "pending-send.jsonl")
            .map_err(|_| failure(STATUS_FAILED))?
            .is_some(),
        last_sent: last.map(|last| dto::Sent {
            to: last.to,
            at: last.at,
            bytes: 0,
        }),
        computers: received.computers.into_values().collect(),
    })
}
pub fn radio_send_history(r: dto::SlugRequest) -> Result<dto::Sent> {
    send_history(&crate::assistants::home(text::NO_HOME)?, &r.slug)
}
pub fn radio_take_deliveries(r: dto::SlugRequest) -> Result<dto::Taken> {
    take_deliveries(&crate::assistants::home(text::NO_HOME)?, &r.slug)
}
pub fn radio_history_status(r: dto::SlugRequest) -> Result<dto::HistoryStatus> {
    history_status(&crate::assistants::home(text::NO_HOME)?, &r.slug)
}
#[cfg(test)]
mod tests;

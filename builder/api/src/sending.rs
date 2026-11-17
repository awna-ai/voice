//! This computer's filtered history travels only to the fleet's build computer over Radio.
use crate::{
    assistants::failure,
    dto, pairing,
    radio::{AskError, Radio, shown, text},
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
const PEER_LIMIT: u64 = 256 * 1024 * 1024;
const FILE_LIMIT: u64 = 64 * 1024 * 1024;
/// A full backtest batch (40 questions, each with 6 precedents) stays below the
/// provider's 8 MiB prompt limit. A real message (4000 characters, 300-character
/// context, 255-byte project) fits unless mostly emoji or control characters;
/// skip only that record, never the whole send.
const DELIVERED_RECORD_BYTES: usize = 16 * 1024;
const HISTORY_FAILED: &str = "Voice couldn't open this computer's history. Try reading it again.";
const TAKE_FAILED: &str = "Voice couldn't take the history your other computers sent. Try again.";
const STATUS_FAILED: &str = "Voice couldn't read the history status. Try again.";
const SEND_FAILED: &str = "Voice couldn't save this computer's history status. Try again.";

#[derive(Clone, Default, Serialize, Deserialize)]
struct Received {
    after: u64,
    computers: BTreeMap<String, dto::SentFrom>,
}
#[derive(Serialize, Deserialize)]
struct LastSent {
    at: f64,
    to: String,
    #[serde(default)]
    bytes: u64,
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
    let name = format!("history-{}-{}.jsonl", now()?.as_nanos(), std::process::id());
    send_pending(
        home,
        slug,
        &history,
        builder.name.clone().unwrap_or(builder.id.clone()),
        |pending| radio.send_phrase(&builder.id, &name, pending),
    )
}
fn send_pending(
    home: &Path,
    slug: &str,
    history: &[u8],
    to: String,
    send: impl FnOnce(&Path) -> std::result::Result<u64, AskError>,
) -> Result<dto::Sent> {
    let pending = private::write_private(home, slug, "pending-send.jsonl", history)
        .map_err(|_| failure(SEND_FAILED))?;
    let sent = send(&pending);
    finish_send(home, slug, to, sent)
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
        &LastSent {
            at,
            to: to.clone(),
            bytes,
        },
    )?;
    Ok(dto::Sent {
        to: shown(to),
        at,
        bytes,
    })
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
    if key.len() != 64
        || !key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    Some(format!("decisions-from-{}.jsonl", &key[..16]))
}
#[derive(Debug)]
enum FoldError {
    Stored,
    Content,
}
impl From<voice_core::error::Error> for FoldError {
    fn from(_: voice_core::error::Error) -> Self {
        Self::Content
    }
}
/// Preserve existing records verbatim; only new ids pass through this member's privacy filters.
/// Skip individual delivered records whose JSONL line exceeds one build part.
fn fold(
    member: &voice_core::member::Member,
    filters: &Filters,
    old: &[u8],
    delivered: &[u8],
    limit: u64,
) -> std::result::Result<(Vec<u8>, u64), FoldError> {
    let mut kept: Vec<Map<String, Value>> = corpus::read_records("history", old)
        .map_err(|_| FoldError::Stored)?
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
        let Value::Object(record) = value else {
            unreachable!()
        };
        voice_ingest::mine::check_record(member, &record)?;
        if voice_ingest::mine::to_jsonl(std::slice::from_ref(&record)).len()
            > DELIVERED_RECORD_BYTES
        {
            continue;
        }
        kept.push(record);
    }
    kept.sort_by(|a, b| {
        (
            a.get("timestamp").and_then(Value::as_str),
            a.get("id").and_then(Value::as_str),
        )
            .cmp(&(
                b.get("timestamp").and_then(Value::as_str),
                b.get("id").and_then(Value::as_str),
            ))
    });
    let mut bytes = Vec::new();
    let count = kept.len() as u64;
    for record in kept {
        // JSON values already parsed by read_records are serializable.
        serde_json::to_writer(&mut bytes, &record).expect("parsed history record");
        bytes.push(b'\n');
        if bytes.len() as u64 > limit {
            return Err(FoldError::Content);
        }
    }
    Ok((bytes, count))
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
    let mut saved: Received = read_json(home, slug, "deliveries-taken.json")
        .map_err(|_| failure(TAKE_FAILED))?
        .unwrap_or_default();
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
            take_one(
                home,
                slug,
                (&assistant, &filters),
                &mut saved,
                &mut answer,
                &delivery,
                PEER_LIMIT,
            )?;
        }
    }
    answer.computers = saved.computers.into_values().map(shown_from).collect();
    Ok(answer)
}
fn take_one(
    home: &Path,
    slug: &str,
    (member, filters): (&voice_core::member::Member, &Filters),
    saved: &mut Received,
    answer: &mut dto::Taken,
    delivery: &radio_client::Delivery,
    peer_limit: u64,
) -> Result<()> {
    let after = u64::try_from(delivery.id)
        .ok()
        .filter(|id| *id > saved.after)
        .ok_or_else(|| failure(TAKE_FAILED))?;
    let mut updated = saved.clone();
    // Radio guarantees a plain file name. Refuse malformed replies without constructing any path.
    let malformed = delivery.name.is_empty()
        || delivery.name.starts_with('.')
        || !delivery
            .name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
    if malformed {
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
                let old =
                    private::read_private_optional_at_max(home, slug, None, &file, peer_limit)
                        .map_err(|_| failure(TAKE_FAILED))?
                        .unwrap_or_default();
                match fold(member, filters, &old, &bytes, peer_limit) {
                    Ok((merged, records)) => {
                        private::write_private(home, slug, &file, &merged)
                            .map_err(|_| failure(TAKE_FAILED))?;
                        updated.computers.insert(
                            delivery.key.clone(),
                            dto::SentFrom {
                                name: delivery.computer.clone(),
                                at: delivery.at,
                                records,
                            },
                        );
                        answer.taken += 1;
                    }
                    Err(FoldError::Content) => answer.refused += 1,
                    Err(FoldError::Stored) => return Err(failure(TAKE_FAILED)),
                }
            }
            Err(error) => {
                if !history_name(&delivery.name)
                    || file.is_none()
                    || delivery_content_error(home, slug, &delivery.name, &error)?
                {
                    answer.refused += 1;
                } else {
                    return Err(failure(TAKE_FAILED));
                }
            }
        }
    }
    let previous = private::read_private_optional(home, slug, "deliveries-taken.json")
        .map_err(|_| failure(TAKE_FAILED))?;
    updated.after = after;
    save_json(home, slug, "deliveries-taken.json", &updated).map_err(|_| failure(TAKE_FAILED))?;
    // Keep the delivery until every write succeeds. If cleanup fails, restore the cursor.
    if !malformed
        && private::remove_private_entry_at(home, slug, "deliveries", &delivery.name).is_err()
    {
        match previous {
            Some(bytes) => {
                private::write_private(home, slug, "deliveries-taken.json", &bytes).map(drop)
            }
            None => private::remove_private(home, slug, "deliveries-taken.json").map(drop),
        }
        .map_err(|_| failure(TAKE_FAILED))?;
        return Err(failure(TAKE_FAILED));
    }
    *saved = updated;
    Ok(())
}
// Inspect the entry after a failed read; error prose is not a file-type contract.
fn delivery_content_error(
    home: &Path,
    slug: &str,
    name: &str,
    error: &voice_core::error::Error,
) -> Result<bool> {
    let entry = private::private_entry_at(home, slug, Some("deliveries"), name)
        .map_err(|_| failure(TAKE_FAILED))?;
    let denied = matches!(error, voice_core::error::Error::Io { source, .. }
        if source.raw_os_error() == Some(libc::EACCES));
    Ok(entry.is_some_and(|entry| !entry.regular || entry.size > FILE_LIMIT || denied))
}
fn shown_from(mut computer: dto::SentFrom) -> dto::SentFrom {
    computer.name = shown(computer.name);
    computer
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
            to: shown(last.to),
            at: last.at,
            bytes: last.bytes,
        }),
        computers: received.computers.into_values().map(shown_from).collect(),
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

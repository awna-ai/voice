//! A blind backtest of the assistant's current decisions on the growth tests kept since the last
//! kept change, drafted as a change to the Calibration section only.
use crate::{
    apply::{self, check_answers},
    apply_reply::tests_prompt,
    diff,
    record::*,
    refresh,
    store::Store,
    text,
    version::version,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
use voice_agent::{guard::Guard, ledger::Session};
use voice_build::{
    build::{Asker, MIN_HELD, Misfit, batches},
    draft,
};
use voice_core::{
    error::{Error, Result},
    member::Member,
};
use voice_ingest::filters::Filters;
use voice_steps::{
    book::book,
    score::{Cell, Score, TIERS},
    share::percent,
};

pub const HEADING: &str = "## Calibration (the blind backtest)";
/// The most tests the change names: the newest the backtest got right, so `apply` checks it on
/// tests the assistant already passes and the rest stay new for `propose`.
pub const MOST_NAMED: usize = 10;
/// Fixed instructions only: this string is passed in argv.
pub const BACKTEST_SYSTEM: &str = include_str!("prompts/backtest.md");

#[derive(Debug)]
pub enum Backtested {
    /// Fewer than [`MIN_HELD`] tests since the last kept change; no model call was made.
    TooFew(usize),
    NoneRight,
    Unchanged,
    Drafted(Proposal),
}

pub fn backtest_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["answers"],"properties":{"answers":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["id","answer","confidence"],"properties":{"id":{"type":"string"},"answer":{"type":"string"},"confidence":{"type":"string","enum":TIERS}}}}}})
}

pub fn backtest_prompt(decisions: &str, batch: &[&Case]) -> String {
    tests_prompt(text::BACKTEST_PROMPT, decisions, batch)
}

/// A backtest reply checked whole: each confidence a tier, then the answers as `apply` checks them.
pub fn check_backtest_answers(
    batch: &[&Case],
    reply: &Value,
) -> std::result::Result<BTreeMap<String, (String, usize)>, Misfit> {
    let first = batch.first().map_or("", |case| case.id.as_str());
    let shape = || {
        Misfit::new(
            text::rule(first, text::APPLY_REPLY_SHAPE),
            text::apply_answers_count(1),
        )
    };
    let rows = reply
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("answers"))
        .and_then(Value::as_array)
        .ok_or_else(shape)?;
    let mut tiers = BTreeMap::new();
    let mut answers = Vec::new();
    let mut problems = Vec::new();
    for row in rows {
        let mut row = row
            .as_object()
            .filter(|object| object.len() == 3)
            .cloned()
            .ok_or_else(shape)?;
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(first)
            .to_owned();
        let tier = row
            .remove("confidence")
            .as_ref()
            .and_then(Value::as_str)
            .and_then(|t| TIERS.iter().position(|tier| *tier == t));
        match tier {
            Some(tier) => {
                tiers.insert(id, tier);
            }
            None => problems.push(text::rule(&id, text::APPLY_CONFIDENCE)),
        }
        answers.push(Value::Object(row));
    }
    let checked = check_answers(batch, &json!({ "answers": answers }));
    if problems.is_empty() {
        return checked.map(|found| {
            found
                .into_iter()
                .map(|(id, answer)| {
                    let tier = tiers[&id];
                    (id, (answer, tier))
                })
                .collect()
        });
    }
    let mut misfit = Misfit::new(
        problems.join("; "),
        text::apply_answers_count(problems.len()),
    );
    if let Err(other) = checked {
        misfit.told = format!("{}; {}", other.told, misfit.told);
        misfit.shown = format!("{}; {}", other.shown, misfit.shown);
    }
    Err(misfit)
}

/// Every new test kept at or after the newest kept change (every new test when none was kept),
/// oldest first.
pub fn since_kept(store: &Store) -> Result<Vec<Case>> {
    let kept = store
        .measurements()?
        .into_iter()
        .filter(|m| m.outcome == KeepOrRevert::Keep)
        .map(|m| m.at)
        .max();
    let mut tests: Vec<_> = store
        .cases()?
        .iter()
        .filter(|c| c.status == Status::New && kept.as_ref().is_none_or(|at| &c.created >= at))
        .cloned()
        .collect();
    tests.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
    Ok(tests)
}

/// `decisions` with its Calibration section (the heading line to the next `## ` heading) replaced
/// by `section`, or `None` when it has no such heading.
pub fn replace_calibration(decisions: &str, section: &str) -> Option<String> {
    let mut at = 0;
    let start = decisions.split_inclusive('\n').find_map(|line| {
        let here = at;
        at += line.len();
        (line.trim_end_matches('\n') == HEADING).then_some(here)
    })?;
    let rest = &decisions[start + HEADING.len()..];
    let end = rest
        .find("\n## ")
        .map_or(decisions.len(), |next| start + HEADING.len() + next + 1);
    let gap = if end < decisions.len() { "\n" } else { "" };
    Some(format!(
        "{}{section}{gap}{}",
        &decisions[..start],
        &decisions[end..]
    ))
}

fn tally(cell: &mut Cell, hit: bool) {
    cell.n += 1;
    cell.hits += usize::from(hit);
}

#[allow(clippy::too_many_arguments)]
pub fn backtest(
    guard: &Guard,
    session: &Session,
    home: &Path,
    store: &Store,
    member: &Member,
    member_dir: &Path,
    filters: &Filters,
    model: &str,
    now: &str,
    progress: &mut dyn FnMut(&str),
) -> Result<Backtested> {
    // Session holds the state lock for its whole life; the save reuses it, as in `propose`.
    let before = refresh::decisions(member_dir)?;
    let base = version(before.as_bytes());
    if let Some((p, approved)) = refresh::pending(store, &base)? {
        return Err(Error::Invalid(if approved {
            text::open_approved(&p.id)
        } else {
            text::open_waiting(&p.id)
        }));
    }
    if replace_calibration(&before, HEADING).is_none() {
        return Err(Error::Invalid(text::NO_CALIBRATION.into()));
    }
    let tests = since_kept(store)?;
    if tests.len() < MIN_HELD {
        return Ok(Backtested::TooFew(tests.len()));
    }
    let tests: Vec<&Case> = tests.iter().collect();
    let mut asker = Asker::new(
        guard,
        voice_build::build::SOURCES,
        session,
        home,
        &member.slug,
        model,
    )
    .with_misfit(text::backtest_misfit);
    progress(&text::backtesting(tests.len()));
    let mut found = BTreeMap::new();
    for batch in batches(&tests) {
        let (reply, _) = asker.ask(
            "backtest",
            BACKTEST_SYSTEM,
            &backtest_schema(),
            &backtest_prompt(&before, batch),
            text::BACKTESTING,
            |reply| check_backtest_answers(batch, reply),
        )?;
        found.extend(reply);
    }
    let answers: BTreeMap<_, _> = found
        .iter()
        .map(|(id, (answer, _))| (id.clone(), answer.clone()))
        .collect();
    let grades = apply::grade_answers(&mut asker, &tests, &[answers], progress)?.remove(0);
    let mut score = Score {
        overall: Cell::default(),
        baseline: 0,
        led: Cell::default(),
        took_member: 0,
        tiers: [None; 3],
        categories: BTreeMap::new(),
    };
    for case in &tests {
        let hit = grades[&case.id] == Grade::Pass;
        let tier = found[&case.id].1;
        tally(&mut score.overall, hit);
        tally(score.tiers[tier].get_or_insert_default(), hit);
        tally(
            score.categories.entry(case.area.clone()).or_default()[tier].get_or_insert_default(),
            hit,
        );
    }
    let book = book(member)?;
    let live = draft::live_categories(&book, &score);
    let day = |case: &Case| case.created.get(..10).unwrap_or_default().to_owned();
    let mut section = vec![
        HEADING.to_owned(),
        String::new(),
        text::calibration(
            &member.name,
            score.overall.n,
            &day(tests[0]),
            &day(tests[tests.len() - 1]),
            score.overall.hits,
            &percent(score.overall.accuracy()),
            &live,
        ),
        String::new(),
        "| Category | High | Medium | Low | Live |".to_owned(),
        "|---|---|---|---|---|".to_owned(),
    ];
    section.extend(draft::calibration_rows(&book, &score));
    let section = section.join("\n") + "\n";
    let after = replace_calibration(&before, &section).expect("checked heading");
    if after == before {
        return Ok(Backtested::Unchanged);
    }
    let mut right: Vec<&Case> = tests
        .iter()
        .copied()
        .filter(|case| grades[&case.id] == Grade::Pass)
        .collect();
    right.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
    right.truncate(MOST_NAMED);
    if right.is_empty() {
        return Ok(Backtested::NoneRight);
    }
    let summary = text::backtest_summary(score.overall.n, score.overall.hits, &live);
    let changes = vec![Change {
        file: "decisions.md".into(),
        diff: diff::unified("decisions.md", &before, &after)?,
        why: text::BACKTEST_WHY.into(),
    }];
    let named: Vec<Case> = right.iter().map(|case| (*case).clone()).collect();
    draft::check_private(filters, &after, "")?;
    apply::check_change(&summary, &changes, &named, &store.signals()?, filters)
        .map_err(|_| Error::Invalid(text::BACKTEST_WORDS.into()))?;
    let cases = named.into_iter().map(|case| case.id).collect();
    refresh::save_draft(
        store, member, member_dir, base, now, summary, changes, cases,
    )
    .map(Backtested::Drafted)
}

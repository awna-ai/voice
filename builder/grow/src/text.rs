pub fn shape(kind: &str, line: usize, column: usize) -> String {
    format!("the {kind} isn't in the growth loop's {kind} shape (line {line}, column {column})")
}

pub fn field(kind: &str, field: &str, rule: &str) -> String {
    format!("the {kind}'s {field} {rule}")
}

pub fn rule(id: &str, rule: &str) -> String {
    format!("{id} {rule}")
}
pub const NOT_APPROVED: &str = "must have an approved answer from the right role for this proposal";
pub const NOT_NEW: &str = "must exist and be new before it can be measured for this proposal";
pub const WRONG_MEASUREMENT: &str =
    "must name this proposal, its approval and its base, and give the outcome its results require";
pub const MISSING_REFERENCE: &str = "must refer to records kept for this persona";
pub const ALREADY_APPROVED: &str = "already has an approval";
pub const REJECTED: &str = "was rejected, so it cannot have a measurement";
pub fn base_moved(id: &str) -> String {
    format!(
        "{id} was drafted for a different version. Draft a new change with `voice refresh propose`."
    )
}
pub fn case_changed(id: &str) -> String {
    format!("case {id} was appended again with a change other than its status")
}
pub fn status_move(id: &str, before: &str, after: &str) -> String {
    format!("case {id} cannot move from {before} to {after}")
}
pub const VERSION: &str = "must name version 1";
pub const FORMAT: &str = "must follow its growth loop format";
pub const NONEMPTY: &str = "must not be empty";
pub const VERDICT: &str = "must be present exactly when kind is verdict";
pub const ONE_ENTRY: &str = "must hold at least one entry";
pub const UNIQUE: &str = "must not repeat an entry";
pub const IDS: &str =
    "must hold ids with their fixed prefix followed by lowercase letters, digits and dashes";
pub const TWO_CHOICES: &str = "must hold at least two choices";
pub const NONEMPTY_CHOICE: &str = "must not hold an empty choice";
pub const CHOICE_OPTIONS: &str = "must be present for a choice";
pub const EXPECTED_CHOICE: &str = "must be one of the options";
pub const CASE_ONCE: &str = "must name each case once";

pub const SLUG_SHAPE: &str =
    "The assistant's slug must use lowercase letters, digits and single dashes between words.";
pub fn damaged(file: &str, line: usize) -> String {
    format!("The kept file {file} is damaged at line {line}. Nothing was added.")
}
pub fn already_kept(id: &str) -> String {
    format!("{id} is already kept and cannot be replaced.")
}
pub const FILLED_IN: &str =
    "Voice fills in the answer's schema, id, persona and time. Leave those out and try again.";
pub const TOO_BIG: &str = "That answer is too big to keep. Keep it under 64 KiB and try again.";
pub const INPUT_SHAPE: &str =
    "That answer isn't in the expected shape. Check its fields and try again.";
pub fn unknown_key(key: &str) -> String {
    if key.is_empty() || key.len() > 40 || !key.bytes().all(|c| c.is_ascii_lowercase() || c == b'_')
    {
        return "The answer has a field Voice doesn't know. Leave it out and try again.".into();
    }
    format!("The answer has an unknown field: {key}. Leave it out and try again.")
}
pub fn unknown_area(categories: &[String]) -> String {
    format!(
        "Choose one of your assistant's areas: {}.",
        categories.join(", ")
    )
}
pub fn invalid_text(field: &str, limit: usize) -> String {
    let limit = limit.to_string();
    let mut grouped = String::new();
    for (index, digit) in limit.chars().enumerate() {
        if index > 0 && (limit.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    let label = match field {
        "area" => "The area",
        "situation" => "What was being decided",
        "wanted" => "The answer you wanted",
        "persona_said" => "The assistant's answer",
        _ => "Where the answer came from",
    };
    format!("{label} must be 1 to {grouped} characters long, with no hidden characters.")
}
fn field_name(field: &str) -> &str {
    match field {
        "area" => "area",
        "situation" => "what was being decided",
        "wanted" => "answer you wanted",
        "persona_said" => "assistant's answer",
        _ => "source",
    }
}
pub fn left_out(field: &str, kind: &str) -> String {
    format!(
        "That answer looks {kind} (in {}), so it wasn't kept. Leave out anything private and try again.",
        field_name(field)
    )
}
pub const NO_CASES: &str = "You haven't given your assistant any answers to learn from yet.";
pub fn cases_summary(book: &crate::rules::CaseBook, categories: &[String]) -> String {
    use crate::record::Status;
    let counts = |area: Option<&str>| {
        let mut n = [0usize; 3];
        for c in book.iter().filter(|c| area.is_none_or(|a| c.area == a)) {
            n[match c.status {
                Status::New => 0,
                Status::Active => 1,
                Status::Retired => 2,
            }] += 1;
        }
        n
    };
    let line = |n: [usize; 3]| {
        format!(
            "{} waiting for a refresh, {} in use, {} retired.",
            n[0], n[1], n[2]
        )
    };
    let total = counts(None);
    if total == [0; 3] {
        return NO_CASES.into();
    }
    let mut text = format!("Tests made from your answers: {}", line(total));
    for area in categories {
        let n = counts(Some(area));
        if n != [0; 3] {
            text.push_str(&format!("\n{area}: {}", line(n)));
        }
    }
    text
}

pub const WRONG_ROLE: &str = "must come from the approver role named by its proposal";

pub const TOO_MANY_IDS: &str =
    "There are too many kept records for this day. Try again on another day.";

pub const KEPT: &str =
    "Kept your answer. It's now one of the tests your assistant is checked on at its next refresh.";

pub const NOT_BUILT: &str = "Build your assistant first: there's nothing to refresh yet.";
pub const DIFF_DOES_NOT_FIT: &str =
    "That saved change doesn't fit your assistant's current decisions.";
pub const DIFF_TOO_BIG: &str = "That change is too large to check. Try a smaller change.";
pub const EDIT_DOES_NOT_FIT: &str = "That point doesn't fit your assistant's current decisions.";
pub const DRAFTING: &str = "Drafting a change from your answers";
pub const NOTHING_TO_REFRESH: &str =
    "There's nothing new to learn from yet. Keep answering your assistant, then try again.";
pub const NO_OPEN: &str = "No change is waiting for your answer.";
pub const NOTE_SHAPE: &str = "The note file must hold {\"note\": \"...\"} and nothing else.";
pub const NOTE_EMPTY: &str = "The note is empty.";
pub const NOTE_CONTROL: &str = "The note can't hold control characters.";
pub const DAMAGED_PROPOSAL: &str = "That saved change looks damaged, so it wasn't shown. Draft a new one with `voice refresh propose`.";
pub const DIFF_BUG: &str = "The drafted change couldn't be checked, so nothing was saved.";
pub const REPLY_SHAPE: &str = "The drafted change didn't have the expected shape.";
pub const EDIT_COUNT: &str = "The drafted change needs between 1 and 10 points.";
pub const REPLY_AREA: &str = "A point names an unknown area.";
pub const REPLY_OLD: &str = "A point doesn't name the earlier point it replaces correctly.";
pub const REPLY_CASES: &str = "A point must name your answers once each, from the ones given.";
pub const REPLY_REPLACE: &str = "Two points replace the same earlier point.";
pub const REPLY_APPLY: &str = "A point doesn't fit the current decisions.";
pub const REPLY_PRIVATE: &str = "A point looks personal or secret; leave that detail out.";
pub const REPLY_RECORDS: &str = "The saved answers couldn't be checked for repeated words.";
pub fn reply_line(place: &str) -> String {
    format!("The {place} must be a short, plain line of at most 400 characters.")
}
pub fn quoted(count: usize) -> String {
    format!("{count} runs of 6 or more of your own words")
}
pub fn open_waiting(id: &str) -> String {
    format!(
        "A change is already waiting for your answer ({id}). Look at it with `voice refresh show`, then approve or reject it."
    )
}
pub fn open_approved(id: &str) -> String {
    format!("You approved a change ({id}) that hasn't been tested and applied yet.")
}
pub fn approved(_id: &str) -> String {
    "Approved. Your assistant changes only after the change is tested on all your answers and nothing gets worse.".into()
}
pub fn rejected(_id: &str) -> String {
    "Rejected. Nothing changed. Your answers stay saved for the next refresh.".into()
}
pub fn no_proposal(id: &str) -> String {
    format!("No saved change has that id ({id}).")
}
pub fn refresh_heading(date: &str, id: &str) -> String {
    let date = date.get(..10).unwrap_or(date);
    format!("Change drafted {date} ({id})")
}
pub fn learned(count: usize) -> String {
    format!("Learned from {count} of your answers.")
}
pub fn refresh_commands(member: &std::path::Path, id: &str) -> [String; 2] {
    [
        format!("voice refresh approve {} {id}", member.display()),
        format!("voice refresh reject {} {id}", member.display()),
    ]
}

pub const DECISIONS_HIDDEN: &str = "Your assistant's decisions.md has a hidden character near the change, so nothing was saved. Remove it, then try again.";

pub fn refresh_misfit(what: &str, problem: &str, _times: usize) -> String {
    format!(
        "{what}: Claude's draft didn't fit twice, so nothing was saved ({problem}). Run `voice refresh propose` again."
    )
}

pub const DECISIONS_CHANGED: &str = "Your assistant's decisions changed while the change was being drafted, so nothing was saved. Try again.";

// Testing and applying an approved refresh.
pub const TESTING: &str = "Testing the change on your answers";
pub const GRADING: &str = "Checking the written answers";
pub const CHANGE_DOES_NOT_FIT: &str = "That saved change no longer fits your assistant, so nothing was changed. Draft a new one with `voice refresh propose`.";
pub const DECISIONS_CHANGED_TESTING: &str = "Your assistant's decisions changed while the change was being tested, so nothing was saved. Draft a new one with `voice refresh propose`.";
pub const DECISIONS_MOVED_AFTER: &str = "Your assistant's decisions.md changed after this change was tested, so it wasn't applied. Draft a new one with `voice refresh propose`.";
pub const KEEP_REASON: &str = "Every new test now passes, and none of the others got worse.";
pub const ANSWER_PROMPT: &str = "Answer each test the way the person would.\n\nHow the person decides (their decisions.md):\n\n";
pub const GRADE_PROMPT: &str = "Grade each answer against what the right answer must do.\n\nThe items, one JSON object per line:\n";
pub const APPLY_REPLY_SHAPE: &str = "must have exactly the requested fields";
pub const APPLY_REPLY_IDS: &str = "must answer each requested id exactly once";
pub const APPLY_ANSWER_CHOICE: &str = "must copy one option exactly";
pub const APPLY_ANSWER_LINE: &str = "must be a short, plain line of at most 1,000 characters";
pub const APPLY_GRADE: &str = "must be pass or fail";
pub fn apply_answers_count(n: usize) -> String {
    format!("{n} of its answers didn't fit")
}
pub fn apply_grades_count(n: usize) -> String {
    format!("{n} of its grades didn't fit")
}
pub fn not_answered(id: &str) -> String {
    format!(
        "That change ({id}) is still waiting for your answer. Look at it with `voice refresh show`, then approve or reject it."
    )
}
pub fn was_rejected(id: &str) -> String {
    format!("You rejected that change ({id}), so it can't be applied.")
}
pub fn cases_moved(_id: &str) -> String {
    "Some of the tests this change was meant to pass have changed since you approved it. Draft a new one with `voice refresh propose`.".into()
}
pub fn apply_misfit(what: &str, problem: &str, _times: usize) -> String {
    format!(
        "{what}: Claude's answers didn't fit twice, so nothing was saved ({problem}). Run `voice refresh apply` again."
    )
}
pub fn kept(_id: &str) -> String {
    "Kept. Your assistant now works this way.".into()
}
pub fn undone(_id: &str) -> String {
    "Undone. Your assistant is as it was, and your answers stay saved for the next refresh.".into()
}
pub fn finished_left(_id: &str) -> String {
    "This change was already tested; finished what was left.".into()
}
pub fn nothing_left(id: &str, kept: bool) -> String {
    format!(
        "This change ({id}) was already tested and {}.",
        if kept { "kept" } else { "undone" }
    )
}
pub fn new_failed(n: usize, m: usize) -> String {
    format!(
        "{n} of the {m} new {} still didn't pass, so the change was undone.",
        if m == 1 { "test" } else { "tests" }
    )
}
pub fn regressed(n: usize) -> String {
    format!(
        "{n} earlier {} that passed before failed with the change, so it was undone.",
        if n == 1 { "test" } else { "tests" }
    )
}
pub fn refresh_entry_heading(day: &str, id: &str) -> String {
    format!("## {day}: refresh ({id})")
}
#[allow(clippy::too_many_arguments)]
pub fn refresh_entry(
    heading: &str,
    summary: &str,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    kept: bool,
    reason: &str,
) -> String {
    let tests = if d == 0 {
        format!("{a} of {b} new tests pass; there were no earlier tests.")
    } else {
        format!("{a} of {b} new tests pass; {c} of {d} earlier tests held.")
    };
    format!(
        "{heading}\n\n- **Change:** {summary}\n- **Tests:** {tests}\n- **Outcome:** {}. {reason}\n",
        if kept { "kept" } else { "undone" }
    )
}
pub fn growth(kept: usize, undone: usize, passing: usize, of: usize) -> String {
    format!(
        "Since the build: {kept} {} kept and {undone} undone. Your assistant passes {passing} of {of} tests from your answers.",
        if kept == 1 { "change" } else { "changes" }
    )
}
pub const ANSWER_TESTS: &str = "\n\nThe tests, one JSON object per line:\n";

pub fn testing_now(n: usize) -> String {
    format!(
        "Testing your assistant as it is now on {n} {}.",
        if n == 1 { "test" } else { "tests" }
    )
}
pub fn testing_change(n: usize) -> String {
    format!(
        "Testing it with the change on {n} {}.",
        if n == 1 { "test" } else { "tests" }
    )
}
pub const CHECKING_ANSWERS: &str = "Checking the written answers against what you wanted.";
pub const BACKTEST_PROMPT: &str = "Answer each test the way the person would, and say how sure you are of each answer.\n\nHow the person decides (their decisions.md):\n\n";
pub const BACKTESTING: &str = "Backtesting your assistant on your answers";
pub const APPLY_CONFIDENCE: &str = "must give a confidence of high, medium or low";
pub const NO_CALIBRATION: &str = "Your assistant's decisions.md has no \"## Calibration (the blind backtest)\" section, so the backtest has nothing to update.";
pub const BACKTEST_UNCHANGED: &str =
    "The backtest matches the Calibration section already, so there's nothing to change.";
pub const BACKTEST_NONE_RIGHT: &str = "Your assistant got none of the tests right, so there's no test to check a new Calibration section with. Try `voice refresh propose` first.";
pub const BACKTEST_WHY: &str =
    "A fresh blind backtest of these decisions on the answers kept since the last kept change.";
pub fn backtest_misfit(what: &str, problem: &str, _times: usize) -> String {
    format!(
        "{what}: Claude's answers didn't fit twice, so nothing was saved ({problem}). Run `voice refresh backtest` again."
    )
}
pub fn backtesting(n: usize) -> String {
    format!("Backtesting your assistant blind on {n} tests from your answers.")
}
pub fn too_few_to_backtest(n: usize) -> String {
    format!(
        "There {} only {n} {} from your answers since the last kept change, fewer than 30, so there's nothing to backtest yet.",
        if n == 1 { "is" } else { "are" },
        if n == 1 { "test" } else { "tests" }
    )
}
fn live_list(live: &[String]) -> String {
    if live.is_empty() {
        "none yet".to_owned()
    } else {
        live.join(", ")
    }
}
/// The Calibration section's paragraph after a growth backtest, one sentence group a line so no
/// line is too long for `apply`'s check; the table follows it.
pub fn calibration(
    name: &str,
    n: usize,
    first: &str,
    last: &str,
    hits: usize,
    percent: &str,
    live: &[String],
) -> String {
    format!(
        "Measured on {n} growth tests ({first} → {last}), the answers kept since the last kept change, each answered blind from this decisions.md. Overall {hits} of {n} ({percent}).\nA category is **live** at 5 or more High picks with at least 90% right: {}.\n{name} decides alone only in live categories that autonomy.md opens; elsewhere High is treated as Medium.",
        live_list(live)
    )
}
pub fn backtest_summary(n: usize, hits: usize, live: &[String]) -> String {
    format!(
        "Update the Calibration section from a blind backtest on {n} newer tests: {hits} right; live: {}.",
        live_list(live)
    )
}
pub const BACKTEST_WORDS: &str =
    "The new Calibration section didn't pass the checks a change must pass, so nothing was saved.";
pub fn backtest_area(id: &str) -> String {
    format!(
        "Test {id} from your answers has a kind of question with a line break or a | in it, which can't go in the Calibration table, so nothing was backtested."
    )
}
pub const BACKTEST_LONG: &str = "A line of the new Calibration section would be too long for a change (over 512 characters) or hold a hidden character, most likely from the list of kinds of question, so nothing was backtested and nothing was sent to Claude.";
pub const BACKTEST_DECLINED: &str = "The backtest drafted the same Calibration change you said no to, or that was tested and undone, so nothing was saved. Keep more answers and run it again.";

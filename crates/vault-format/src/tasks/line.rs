//! Obsidian Tasks checklist lines: parsing with spans, span-preserving edits, and canonical
//! rendering.
//!
//! A task line keeps its exact text; parsing records where each field is so edits replace
//! only the bytes they change. Unknown text anywhere in the line is preserved.

use std::fmt;
use std::ops::Range;

use chrono::{NaiveDate, NaiveTime};

use super::recurrence::{RecurrenceNotUnderstood, RecurrenceRule, parse_recurrence};

/// The checkbox state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    /// `[ ]`
    Todo,
    /// `[x]` or `[X]`
    Done,
    /// `[-]`
    Cancelled,
    /// Any other character (e.g. `[/]` in progress), kept as is.
    Other(char),
}

impl TaskStatus {
    fn from_char(c: char) -> Self {
        match c {
            ' ' => Self::Todo,
            'x' | 'X' => Self::Done,
            '-' => Self::Cancelled,
            c => Self::Other(c),
        }
    }

    /// The character written between the brackets (`x` for done).
    pub fn to_char(self) -> char {
        match self {
            Self::Todo => ' ',
            Self::Done => 'x',
            Self::Cancelled => '-',
            Self::Other(c) => c,
        }
    }

    /// Todo or a custom status (not done/cancelled).
    pub fn is_open(self) -> bool {
        matches!(self, Self::Todo | Self::Other(_))
    }
}

/// Task priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Priority {
    /// 🔺
    Highest,
    /// ⏫
    High,
    /// 🔼
    Medium,
    /// 🔽
    Low,
    /// ⏬
    Lowest,
}

impl Priority {
    /// All priorities, highest first.
    pub const ALL: [Self; 5] = [
        Self::Highest,
        Self::High,
        Self::Medium,
        Self::Low,
        Self::Lowest,
    ];

    /// The signifier.
    pub fn emoji(self) -> &'static str {
        match self {
            Self::Highest => "🔺",
            Self::High => "⏫",
            Self::Medium => "🔼",
            Self::Low => "🔽",
            Self::Lowest => "⏬",
        }
    }
}

/// The date fields of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DateKind {
    /// ➕ created
    Created,
    /// 🛫 start
    Start,
    /// ⏳ scheduled
    Scheduled,
    /// 📅 due
    Due,
    /// ❌ cancelled
    Cancelled,
    /// ✅ done
    Done,
}

impl DateKind {
    /// All date kinds in the Tasks plugin's order.
    pub const ALL: [Self; 6] = [
        Self::Created,
        Self::Start,
        Self::Scheduled,
        Self::Due,
        Self::Cancelled,
        Self::Done,
    ];

    /// The signifier Strata writes.
    pub fn emoji(self) -> &'static str {
        match self {
            Self::Created => "➕",
            Self::Start => "🛫",
            Self::Scheduled => "⏳",
            Self::Due => "📅",
            Self::Cancelled => "❌",
            Self::Done => "✅",
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    /// Position in the Tasks plugin's field order (priority 0, recurrence 1, dates 2…7).
    fn rank(self) -> u8 {
        2 + self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sig {
    Priority(Priority),
    Recurrence,
    Date(DateKind),
}

/// Recognised signifiers, including the Tasks plugin's alternates (`⌛`, `📆`, `🗓`).
const SIGNIFIERS: &[(&str, Sig)] = &[
    ("🔺", Sig::Priority(Priority::Highest)),
    ("⏫", Sig::Priority(Priority::High)),
    ("🔼", Sig::Priority(Priority::Medium)),
    ("🔽", Sig::Priority(Priority::Low)),
    ("⏬", Sig::Priority(Priority::Lowest)),
    ("🔁", Sig::Recurrence),
    ("➕", Sig::Date(DateKind::Created)),
    ("🛫", Sig::Date(DateKind::Start)),
    ("⏳", Sig::Date(DateKind::Scheduled)),
    ("⌛", Sig::Date(DateKind::Scheduled)),
    ("📅", Sig::Date(DateKind::Due)),
    ("📆", Sig::Date(DateKind::Due)),
    ("🗓", Sig::Date(DateKind::Due)),
    ("❌", Sig::Date(DateKind::Cancelled)),
    ("✅", Sig::Date(DateKind::Done)),
];

const VS16: char = '\u{fe0f}';

/// A Reminder-plugin marker `(@YYYY-MM-DD HH:mm)` or `(@YYYY-MM-DD)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Reminder {
    /// Date.
    pub date: NaiveDate,
    /// Wall-clock time in the user's time zone; `None` uses the default reminder time.
    pub time: Option<NaiveTime>,
}

impl fmt::Display for Reminder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.time {
            Some(t) => write!(
                f,
                "(@{} {})",
                self.date.format("%Y-%m-%d"),
                t.format("%H:%M")
            ),
            None => write!(f, "(@{})", self.date.format("%Y-%m-%d")),
        }
    }
}

/// A value with the span of its whole field (signifier included).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Spanned<T> {
    /// The value.
    pub value: T,
    /// Span of the field in the line.
    pub span: Range<usize>,
}

/// A parsed task line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskLine {
    text: String,
    status: TaskStatus,
    status_span: Range<usize>,
    content: Range<usize>,
    priority: Option<Spanned<Priority>>,
    recurrence: Option<Spanned<String>>,
    dates: [Option<Spanned<NaiveDate>>; 6],
    reminders: Vec<Spanned<Reminder>>,
    block_id: Option<Spanned<String>>,
    description: String,
}

#[derive(Debug, Clone)]
struct Prefix {
    status: TaskStatus,
    status_span: Range<usize>,
    content_start: usize,
}

fn parse_prefix(text: &str) -> Option<Prefix> {
    if text.contains(['\n', '\r']) {
        return None;
    }
    let b = text.as_bytes();
    let mut i = 0;
    loop {
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        if i < b.len() && b[i] == b'>' {
            i += 1;
        } else {
            break;
        }
    }
    match b.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        b'0'..=b'9' => {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i - start > 9 || !matches!(b.get(i), Some(b'.' | b')')) {
                return None;
            }
            i += 1;
        }
        _ => return None,
    }
    let ws = i;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    if i == ws || b.get(i) != Some(&b'[') {
        return None;
    }
    let c = text[i + 1..].chars().next()?;
    let status_span = i + 1..i + 1 + c.len_utf8();
    if text.as_bytes().get(status_span.end) != Some(&b']') {
        return None;
    }
    let after = status_span.end + 1;
    let content_start = match b.get(after) {
        None => after,
        Some(b' ' | b'\t') => after + 1,
        Some(_) => return None,
    };
    Some(Prefix {
        status: TaskStatus::from_char(c),
        status_span,
        content_start,
    })
}

fn is_ws(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn ends_field(text: &str, pos: usize, end: usize) -> bool {
    pos >= end || text[pos..].starts_with(is_ws)
}

fn parse_date(text: &str, pos: usize) -> Option<NaiveDate> {
    let s = text.get(pos..pos + 10)?;
    let ok = s.bytes().enumerate().all(|(i, c)| {
        if i == 4 || i == 7 {
            c == b'-'
        } else {
            c.is_ascii_digit()
        }
    });
    if !ok {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// `(@YYYY-MM-DD[ HH:mm])` at `pos`; returns the reminder and the end offset.
fn parse_reminder(text: &str, pos: usize) -> Option<(Reminder, usize)> {
    let p = pos + 2;
    let date = parse_date(text, p)?;
    let mut q = p + 10;
    let mut time = None;
    if text[q..].starts_with(' ') {
        let t = text.get(q + 1..q + 6)?;
        if t.as_bytes().get(2) != Some(&b':') {
            return None;
        }
        time = Some(NaiveTime::parse_from_str(t, "%H:%M").ok()?);
        q += 6;
    }
    text[q..]
        .starts_with(')')
        .then_some((Reminder { date, time }, q + 1))
}

/// A trailing `^id` in `text[start..]`: (whitespace-trimmed content end, id span).
fn find_block_id(text: &str, start: usize) -> Option<(usize, Range<usize>)> {
    let trimmed_end = start + text[start..].trim_end_matches(is_ws).len();
    let caret = start + text[start..trimmed_end].rfind('^')?;
    let id = &text[caret + 1..trimmed_end];
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    if caret > start && !text[..caret].ends_with(is_ws) {
        return None;
    }
    let content_end = start + text[start..caret].trim_end_matches(is_ws).len();
    Some((content_end, caret..trimmed_end))
}

#[derive(Debug, Clone)]
struct Found {
    sig: Option<Sig>,
    span: Range<usize>,
    date: Option<NaiveDate>,
    phrase: Option<String>,
    reminder: Option<Reminder>,
}

fn scan_fields(text: &str, start: usize, end: usize) -> Vec<Found> {
    let mut out = Vec::new();
    let mut i = start;
    while i < end {
        let rest = &text[i..end];
        if rest.starts_with("(@") {
            if let Some((r, e)) = parse_reminder(text, i).filter(|(_, e)| *e <= end) {
                out.push(Found {
                    sig: None,
                    span: i..e,
                    date: None,
                    phrase: None,
                    reminder: Some(r),
                });
                i = e;
                continue;
            }
        } else if let Some(&(emoji, sig)) = SIGNIFIERS.iter().find(|(e, _)| rest.starts_with(e)) {
            let mut p = i + emoji.len();
            if text[p..end].starts_with(VS16) {
                p += VS16.len_utf8();
            }
            if let Some(f) = field_after(text, i, p, end, sig) {
                i = f.span.end;
                out.push(f);
                continue;
            }
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    out
}

fn field_after(text: &str, start: usize, mut p: usize, end: usize, sig: Sig) -> Option<Found> {
    match sig {
        Sig::Priority(_) => ends_field(text, p, end).then_some(Found {
            sig: Some(sig),
            span: start..p,
            date: None,
            phrase: None,
            reminder: None,
        }),
        Sig::Date(_) => {
            while p < end && text[p..].starts_with(' ') {
                p += 1;
            }
            let date = parse_date(text, p).filter(|_| p + 10 <= end)?;
            ends_field(text, p + 10, end).then(|| Found {
                sig: Some(sig),
                span: start..p + 10,
                date: Some(date),
                phrase: None,
                reminder: None,
            })
        }
        Sig::Recurrence => {
            while p < end && text[p..].starts_with(' ') {
                p += 1;
            }
            let len = text[p..end]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == ',' || c == ' ' || c == '!'))
                .unwrap_or(end - p);
            let phrase = text[p..p + len].trim_end_matches([' ', ',']);
            (!phrase.is_empty()).then(|| Found {
                sig: Some(sig),
                span: start..p + phrase.len(),
                date: None,
                phrase: Some(phrase.to_owned()),
                reminder: None,
            })
        }
    }
}

impl TaskLine {
    /// Parses a single line (no terminator). `None` if it is not a checklist item.
    pub fn parse(text: &str) -> Option<Self> {
        let prefix = parse_prefix(text)?;
        Some(Self::build(text.to_owned(), &prefix))
    }

    fn build(text: String, prefix: &Prefix) -> Self {
        let start = prefix.content_start;
        let (content_end, block_id) = match find_block_id(&text, start) {
            Some((end, span)) => (
                end,
                Some(Spanned {
                    value: text[span.start + 1..span.end].to_owned(),
                    span,
                }),
            ),
            None => (start + text[start..].trim_end_matches(is_ws).len(), None),
        };
        let found = scan_fields(&text, start, content_end);
        // Single-valued fields: the last occurrence wins (the Tasks plugin reads from the end);
        // earlier duplicates stay in the description.
        let key = |f: &Found| {
            f.sig.map(|sig| match sig {
                Sig::Priority(_) => 0,
                Sig::Recurrence => 1,
                Sig::Date(k) => k.rank(),
            })
        };
        let keep: Vec<bool> = found
            .iter()
            .enumerate()
            .map(|(i, f)| key(f).is_none() || !found[i + 1..].iter().any(|g| key(g) == key(f)))
            .collect();
        let mut line = Self {
            status: prefix.status,
            status_span: prefix.status_span.clone(),
            content: start..content_end,
            priority: None,
            recurrence: None,
            dates: Default::default(),
            reminders: Vec::new(),
            block_id,
            description: String::new(),
            text: String::new(),
        };
        let mut desc = String::new();
        let mut last = start;
        for (f, k) in found.iter().zip(&keep) {
            if !*k {
                continue;
            }
            desc.push_str(&text[last..f.span.start]);
            desc.push(' ');
            last = f.span.end;
            let span = f.span.clone();
            match (f.sig, f.date, &f.phrase, f.reminder) {
                (Some(Sig::Priority(p)), ..) => line.priority = Some(Spanned { value: p, span }),
                (Some(Sig::Recurrence), _, Some(phrase), _) => {
                    line.recurrence = Some(Spanned {
                        value: phrase.clone(),
                        span,
                    });
                }
                (Some(Sig::Date(kind)), Some(d), ..) => {
                    line.dates[kind.index()] = Some(Spanned { value: d, span });
                }
                (None, _, _, Some(r)) => line.reminders.push(Spanned { value: r, span }),
                _ => {}
            }
        }
        desc.push_str(&text[last..content_end]);
        line.description = desc.split_whitespace().collect::<Vec<_>>().join(" ");
        line.text = text;
        line
    }

    /// The line exactly as it is (after any edits).
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Status.
    pub fn status(&self) -> TaskStatus {
        self.status
    }

    /// The character between the brackets as written.
    pub fn status_char(&self) -> char {
        self.text[self.status_span.clone()]
            .chars()
            .next()
            .unwrap_or(' ')
    }

    /// The description: the text that is not a recognised field, whitespace collapsed.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Priority.
    pub fn priority(&self) -> Option<Priority> {
        self.priority.as_ref().map(|p| p.value)
    }

    /// A date field.
    pub fn date(&self, kind: DateKind) -> Option<NaiveDate> {
        self.dates[kind.index()].as_ref().map(|d| d.value)
    }

    /// The recurrence phrase exactly as written (after `🔁`).
    pub fn recurrence_text(&self) -> Option<&str> {
        self.recurrence.as_ref().map(|r| r.value.as_str())
    }

    /// The parsed recurrence rule; `None` when the task does not recur.
    pub fn recurrence(&self) -> Option<Result<RecurrenceRule, RecurrenceNotUnderstood>> {
        self.recurrence_text().map(parse_recurrence)
    }

    /// Reminders in line order.
    pub fn reminders(&self) -> Vec<Reminder> {
        self.reminders.iter().map(|r| r.value).collect()
    }

    /// The block ID (without `^`), e.g. `t-01j9a2`.
    pub fn block_id(&self) -> Option<&str> {
        self.block_id.as_ref().map(|b| b.value.as_str())
    }

    /// Spans of every recognised field, for highlighting: (signifier kind name, span).
    pub fn field_spans(&self) -> Vec<(&'static str, Range<usize>)> {
        let mut out: Vec<(&'static str, Range<usize>)> = Vec::new();
        if let Some(p) = &self.priority {
            out.push(("priority", p.span.clone()));
        }
        if let Some(r) = &self.recurrence {
            out.push(("recurrence", r.span.clone()));
        }
        for (kind, d) in DateKind::ALL.iter().zip(&self.dates) {
            if let Some(d) = d {
                let name = match kind {
                    DateKind::Created => "created",
                    DateKind::Start => "start",
                    DateKind::Scheduled => "scheduled",
                    DateKind::Due => "due",
                    DateKind::Cancelled => "cancelled",
                    DateKind::Done => "done",
                };
                out.push((name, d.span.clone()));
            }
        }
        out.extend(self.reminders.iter().map(|r| ("reminder", r.span.clone())));
        if let Some(b) = &self.block_id {
            out.push(("block-id", b.span.clone()));
        }
        out.sort_by_key(|(_, s)| s.start);
        out
    }

    fn prefix(&self) -> Prefix {
        Prefix {
            status: self.status,
            status_span: self.status_span.clone(),
            content_start: self.content.start,
        }
    }

    fn apply(&self, mut edits: Vec<(Range<usize>, String)>) -> Self {
        edits.sort_by_key(|(r, _)| (r.start, r.end));
        let mut out = String::with_capacity(self.text.len() + 32);
        let mut last = 0;
        for (r, s) in edits {
            out.push_str(&self.text[last..r.start]);
            out.push_str(&s);
            last = r.end;
        }
        out.push_str(&self.text[last..]);
        Self::build(out, &self.prefix())
    }

    /// Tasks-plugin fields (priority, recurrence, dates) as (rank, span).
    fn ranked_fields(&self) -> Vec<(u8, Range<usize>)> {
        let mut v = Vec::new();
        if let Some(p) = &self.priority {
            v.push((0, p.span.clone()));
        }
        if let Some(r) = &self.recurrence {
            v.push((1, r.span.clone()));
        }
        for kind in DateKind::ALL {
            if let Some(d) = &self.dates[kind.index()] {
                v.push((kind.rank(), d.span.clone()));
            }
        }
        v
    }

    /// Where to insert a new field of `rank`, and the text to insert.
    fn insertion(&self, rank: u8, field: &str) -> (Range<usize>, String) {
        let fields = self.ranked_fields();
        if let Some((_, s)) = fields
            .iter()
            .filter(|(r, _)| *r > rank)
            .min_by_key(|(_, s)| s.start)
        {
            return (s.start..s.start, format!("{field} "));
        }
        if let Some((_, s)) = fields
            .iter()
            .filter(|(r, _)| *r < rank)
            .max_by_key(|(_, s)| s.end)
        {
            return (s.end..s.end, format!(" {field}"));
        }
        self.insert_at_end(field)
    }

    fn insert_at_end(&self, field: &str) -> (Range<usize>, String) {
        let at = self.content.end;
        let sep = if at == self.content.start && self.text[..at].ends_with(is_ws) {
            ""
        } else {
            " "
        };
        (at..at, format!("{sep}{field}"))
    }

    /// The span to delete to remove a field: the field plus one adjacent space.
    fn removal(&self, span: &Range<usize>) -> Range<usize> {
        if self.text[..span.start].ends_with(is_ws) && span.start > self.content.start {
            span.start - 1..span.end
        } else if self.text[span.end..].starts_with(is_ws) {
            span.start..span.end + 1
        } else {
            span.clone()
        }
    }

    fn set_field(
        &self,
        existing: Option<&Range<usize>>,
        rank: u8,
        new_text: Option<String>,
    ) -> Self {
        let edit = match (existing, new_text) {
            (Some(span), Some(t)) => (span.clone(), t),
            (Some(span), None) => (self.removal(span), String::new()),
            (None, Some(t)) => self.insertion(rank, &t),
            (None, None) => return self.clone(),
        };
        self.apply(vec![edit])
    }

    /// Sets the checkbox.
    #[must_use]
    pub fn with_status(&self, status: TaskStatus) -> Self {
        let mut text = self.text.clone();
        let c = status.to_char();
        text.replace_range(self.status_span.clone(), c.encode_utf8(&mut [0; 4]));
        let prefix = Prefix {
            status,
            status_span: self.status_span.start..self.status_span.start + c.len_utf8(),
            content_start: self.content.start - self.status_span.len() + c.len_utf8(),
        };
        Self::build(text, &prefix)
    }

    /// Sets, replaces or removes a date field.
    #[must_use]
    pub fn with_date(&self, kind: DateKind, date: Option<NaiveDate>) -> Self {
        let existing = self.dates[kind.index()].as_ref().map(|d| &d.span);
        let text = date.map(|d| format!("{} {}", kind.emoji(), d.format("%Y-%m-%d")));
        self.set_field(existing, kind.rank(), text)
    }

    /// Sets, replaces or removes the priority.
    #[must_use]
    pub fn with_priority(&self, priority: Option<Priority>) -> Self {
        let existing = self.priority.as_ref().map(|p| &p.span);
        self.set_field(existing, 0, priority.map(|p| p.emoji().to_owned()))
    }

    /// Sets, replaces or removes the recurrence phrase (written verbatim after `🔁 `).
    #[must_use]
    pub fn with_recurrence(&self, phrase: Option<&str>) -> Self {
        let existing = self.recurrence.as_ref().map(|r| &r.span);
        self.set_field(existing, 1, phrase.map(|p| format!("🔁 {p}")))
    }

    /// Sets or replaces the block ID.
    #[must_use]
    pub fn with_block_id(&self, id: &str) -> Self {
        let edit = if let Some(b) = &self.block_id {
            (b.span.clone(), format!("^{id}"))
        } else {
            let at = self.text.trim_end_matches(is_ws).len();
            let sep = if self.text[..at].ends_with(is_ws) {
                ""
            } else {
                " "
            };
            (at..at, format!("{sep}^{id}"))
        };
        self.apply(vec![edit])
    }

    /// Replaces the reminders. The same number of reminders are replaced in place; otherwise
    /// all are removed and the new ones written together where the first one was (or before
    /// the Tasks fields, or at the end of the content).
    #[must_use]
    pub fn with_reminders(&self, reminders: &[Reminder]) -> Self {
        if reminders.len() == self.reminders.len() {
            let edits = self
                .reminders
                .iter()
                .zip(reminders)
                .map(|(old, new)| (old.span.clone(), new.to_string()))
                .collect();
            return self.apply(edits);
        }
        let mut edits: Vec<(Range<usize>, String)> = self
            .reminders
            .iter()
            .map(|r| (self.removal(&r.span), String::new()))
            .collect();
        if !reminders.is_empty() {
            let group = reminders
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            let (at, text) = if let Some(first) = self.reminders.first() {
                let at = self.removal(&first.span).start;
                (at..at, format!(" {group}"))
            } else if let Some(s) = self.ranked_fields().iter().map(|(_, s)| s.start).min() {
                (s..s, format!("{group} "))
            } else {
                self.insert_at_end(&group)
            };
            edits.push((at, text));
        }
        self.apply(edits)
    }

    /// The structured form of this line.
    pub fn to_spec(&self) -> TaskSpec {
        TaskSpec {
            prefix: self.text[..self.status_span.start - 1].to_owned(),
            status: self.status,
            description: self.description.clone(),
            priority: self.priority(),
            recurrence: self.recurrence_text().map(str::to_owned),
            created: self.date(DateKind::Created),
            start: self.date(DateKind::Start),
            scheduled: self.date(DateKind::Scheduled),
            due: self.date(DateKind::Due),
            cancelled: self.date(DateKind::Cancelled),
            done: self.date(DateKind::Done),
            reminders: self.reminders(),
            block_id: self.block_id().map(str::to_owned),
        }
    }
}

impl fmt::Display for TaskLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// A task as data, for creating new lines.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskSpec {
    /// Everything before `[` (indentation, quote markers, list marker, spaces), e.g. `- `.
    pub prefix: String,
    /// Status.
    pub status: TaskStatus,
    /// Description (may contain links and tags).
    pub description: String,
    /// Priority.
    pub priority: Option<Priority>,
    /// Recurrence phrase.
    pub recurrence: Option<String>,
    /// ➕
    pub created: Option<NaiveDate>,
    /// 🛫
    pub start: Option<NaiveDate>,
    /// ⏳
    pub scheduled: Option<NaiveDate>,
    /// 📅
    pub due: Option<NaiveDate>,
    /// ❌
    pub cancelled: Option<NaiveDate>,
    /// ✅
    pub done: Option<NaiveDate>,
    /// Reminders.
    pub reminders: Vec<Reminder>,
    /// Block ID without `^`.
    pub block_id: Option<String>,
}

impl Default for TaskSpec {
    fn default() -> Self {
        Self {
            prefix: "- ".into(),
            status: TaskStatus::Todo,
            description: String::new(),
            priority: None,
            recurrence: None,
            created: None,
            start: None,
            scheduled: None,
            due: None,
            cancelled: None,
            done: None,
            reminders: Vec::new(),
            block_id: None,
        }
    }
}

impl TaskSpec {
    /// Renders the canonical line: description, reminders, then the Tasks fields in the
    /// plugin's order (priority, 🔁, ➕, 🛫, ⏳, 📅, ❌, ✅) so the Tasks plugin can read them
    /// (it only reads fields at the end of a line), then the block ID.
    pub fn render(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.description.is_empty() {
            parts.push(self.description.clone());
        }
        parts.extend(self.reminders.iter().map(ToString::to_string));
        if let Some(p) = self.priority {
            parts.push(p.emoji().to_owned());
        }
        if let Some(r) = &self.recurrence {
            parts.push(format!("🔁 {r}"));
        }
        let dates = [
            (DateKind::Created, self.created),
            (DateKind::Start, self.start),
            (DateKind::Scheduled, self.scheduled),
            (DateKind::Due, self.due),
            (DateKind::Cancelled, self.cancelled),
            (DateKind::Done, self.done),
        ];
        for (kind, d) in dates {
            if let Some(d) = d {
                parts.push(format!("{} {}", kind.emoji(), d.format("%Y-%m-%d")));
            }
        }
        if let Some(id) = &self.block_id {
            parts.push(format!("^{id}"));
        }
        format!(
            "{}[{}] {}",
            self.prefix,
            self.status.to_char(),
            parts.join(" ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or_default()
    }

    fn p(s: &str) -> TaskLine {
        TaskLine::parse(s).unwrap_or_else(|| panic!("not a task: {s}"))
    }

    #[test]
    fn prefixes() {
        for ok in [
            "- [ ] a",
            "* [x] a",
            "+ [-] a",
            "1. [ ] a",
            "12) [/] a",
            "  - [ ] a",
            "> - [ ] a",
            "\t- [ ]",
            "- [ ] ",
        ] {
            assert!(TaskLine::parse(ok).is_some(), "{ok:?}");
        }
        for bad in [
            "-[ ] a",
            "- [] a",
            "- [ ]a",
            "- [  ] a",
            "a - [ ] b",
            "- [ ] a\nb",
            "1234567890. [ ] a",
            "- x",
        ] {
            assert!(TaskLine::parse(bad).is_none(), "{bad:?}");
        }
        assert_eq!(p("- [X] a").status(), TaskStatus::Done);
        assert_eq!(p("- [X] a").status_char(), 'X');
        assert_eq!(p("- [/] a").status(), TaskStatus::Other('/'));
    }

    #[test]
    fn spec_example_line() {
        let t = p(
            "- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2",
        );
        assert_eq!(t.description(), "Make Watanya's ETA invoice [[Watanya]]");
        assert_eq!(t.recurrence_text(), Some("every month on the 1st"));
        assert_eq!(t.date(DateKind::Due), Some(d("2026-10-01")));
        assert_eq!(
            t.reminders(),
            vec![Reminder {
                date: d("2026-10-01"),
                time: NaiveTime::from_hms_opt(9, 0, 0)
            }]
        );
        assert_eq!(t.block_id(), Some("t-01j9a2"));
        let names: Vec<_> = t.field_spans().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["recurrence", "due", "reminder", "block-id"]);
    }

    #[test]
    fn every_signifier() {
        let t = p(
            "- [ ] x ⏫ 🔁 every day ➕ 2026-01-01 🛫 2026-01-02 ⏳ 2026-01-03 📅 2026-01-04 ❌ 2026-01-05 ✅ 2026-01-06",
        );
        assert_eq!(t.priority(), Some(Priority::High));
        assert_eq!(t.recurrence_text(), Some("every day"));
        let got: Vec<_> = DateKind::ALL.iter().map(|k| t.date(*k)).collect();
        let want: Vec<_> = (1..=6).map(|i| Some(d(&format!("2026-01-0{i}")))).collect();
        assert_eq!(got, want);
        assert_eq!(t.description(), "x");
    }

    #[test]
    fn alternates_and_variation_selectors() {
        let t = p("- [ ] x ⏳\u{fe0f} 2026-01-03 📆2026-01-04 ⌛ 2026-01-05 🔺\u{fe0f}");
        assert_eq!(t.date(DateKind::Due), Some(d("2026-01-04")));
        // last duplicate wins; the earlier one stays in the description
        assert_eq!(t.date(DateKind::Scheduled), Some(d("2026-01-05")));
        assert_eq!(t.description(), "x ⏳\u{fe0f} 2026-01-03");
        assert_eq!(t.priority(), Some(Priority::Highest));
    }

    #[test]
    fn invalid_fields_are_text() {
        let t = p("- [ ] x 📅 2026-02-30 📅 soon 🔺x (@2026-01-01 25:00) 🔁 ^no_pe");
        assert_eq!(t.date(DateKind::Due), None);
        assert_eq!(t.priority(), None);
        assert!(t.reminders().is_empty());
        assert_eq!(t.recurrence_text(), None);
        assert_eq!(t.block_id(), None);
        assert_eq!(
            t.description(),
            "x 📅 2026-02-30 📅 soon 🔺x (@2026-01-01 25:00) 🔁 ^no_pe"
        );
    }

    #[test]
    fn edits_preserve_other_text() {
        let line = "- [ ] Pay  rent 🔁 every month 📅 2026-10-01 (@2026-10-01 09:00) #home ^t-1";
        let t = p(line);
        let t2 = t.with_date(DateKind::Due, Some(d("2026-11-01")));
        assert_eq!(
            t2.as_str(),
            "- [ ] Pay  rent 🔁 every month 📅 2026-11-01 (@2026-10-01 09:00) #home ^t-1"
        );
        let t3 = t2
            .with_date(DateKind::Done, Some(d("2026-10-02")))
            .with_status(TaskStatus::Done);
        assert_eq!(
            t3.as_str(),
            "- [x] Pay  rent 🔁 every month 📅 2026-11-01 ✅ 2026-10-02 (@2026-10-01 09:00) #home ^t-1"
        );
        let t4 = t3.with_date(DateKind::Scheduled, Some(d("2026-10-30")));
        assert_eq!(
            t4.as_str(),
            "- [x] Pay  rent 🔁 every month ⏳ 2026-10-30 📅 2026-11-01 ✅ 2026-10-02 (@2026-10-01 09:00) #home ^t-1"
        );
        let t5 = t4
            .with_date(DateKind::Due, None)
            .with_recurrence(None)
            .with_priority(Some(Priority::Low));
        assert_eq!(
            t5.as_str(),
            "- [x] Pay  rent 🔽 ⏳ 2026-10-30 ✅ 2026-10-02 (@2026-10-01 09:00) #home ^t-1"
        );
        let t6 = t5.with_block_id("t-2").with_reminders(&[]);
        assert_eq!(
            t6.as_str(),
            "- [x] Pay  rent 🔽 ⏳ 2026-10-30 ✅ 2026-10-02 #home ^t-2"
        );
    }

    #[test]
    fn inserting_into_bare_lines() {
        let t = p("- [ ] ");
        assert_eq!(
            t.with_date(DateKind::Due, Some(d("2026-01-01"))).as_str(),
            "- [ ] 📅 2026-01-01"
        );
        let t = p("- [ ]");
        assert_eq!(
            t.with_date(DateKind::Due, Some(d("2026-01-01"))).as_str(),
            "- [ ] 📅 2026-01-01"
        );
        let t = p("- [ ] call");
        assert_eq!(t.with_block_id("t-9").as_str(), "- [ ] call ^t-9");
        let r = [Reminder {
            date: d("2026-01-01"),
            time: None,
        }];
        assert_eq!(t.with_reminders(&r).as_str(), "- [ ] call (@2026-01-01)");
        let t = p("- [ ] call 📅 2026-01-01");
        assert_eq!(
            t.with_reminders(&r).as_str(),
            "- [ ] call (@2026-01-01) 📅 2026-01-01"
        );
        let t = p("- [ ] call (@2026-01-01) x");
        let two = [
            r[0],
            Reminder {
                date: d("2026-01-02"),
                time: NaiveTime::from_hms_opt(8, 5, 0),
            },
        ];
        assert_eq!(
            t.with_reminders(&two).as_str(),
            "- [ ] call (@2026-01-01) (@2026-01-02 08:05) x"
        );
    }

    #[test]
    fn status_edits_with_multibyte_status() {
        let t = p("- [→] moved 📅 2026-01-01");
        let t2 = t.with_status(TaskStatus::Todo);
        assert_eq!(t2.as_str(), "- [ ] moved 📅 2026-01-01");
        assert_eq!(t2.date(DateKind::Due), Some(d("2026-01-01")));
        assert_eq!(t2.description(), "moved");
    }

    #[test]
    fn spec_renders_canonically() {
        let spec = TaskSpec {
            description: "Petrol Arrows invoice [[Petrol Arrows]]".into(),
            priority: Some(Priority::Medium),
            recurrence: Some("every week on Sunday".into()),
            due: Some(d("2026-09-27")),
            reminders: vec![Reminder {
                date: d("2026-09-27"),
                time: NaiveTime::from_hms_opt(10, 0, 0),
            }],
            block_id: Some("t-01j9a3".into()),
            ..TaskSpec::default()
        };
        let line = spec.render();
        assert_eq!(
            line,
            "- [ ] Petrol Arrows invoice [[Petrol Arrows]] (@2026-09-27 10:00) 🔼 🔁 every week on Sunday 📅 2026-09-27 ^t-01j9a3"
        );
        assert_eq!(p(&line).to_spec(), spec);
    }
}

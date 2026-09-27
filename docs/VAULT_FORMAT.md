# Vault format

Authoritative description of the Strata vault format (PLAN §6). Every rule here is implemented
in `crates/vault-format` (shared by the backend and the client core, PLAN L16) and covered by
its tests. Where this document is more precise than PLAN §6, this document wins; the points
that go beyond or differ from PLAN's illustrative examples are listed in
[Decisions beyond PLAN](#decisions-beyond-plan).

All offsets/spans the crate returns are byte ranges into the UTF-8 text passed in; body spans
are relative to the body (add `Document::body_offset()` for file offsets).

---

## 1. Files and paths

- Notes are `.md` files. Paths are vault-relative with `/` separators: `notes/Sub/Name.md`.
- **Title = file name without `.md`** (`title_from_path`). The optional `title:` property is a
  display title only.
- Unicode is allowed, including Arabic: `people/أحمد سمير.md`.

### Forbidden characters and names (`filename::validate_file_name`)

A path segment is rejected if it:

| Rule | Error |
|---|---|
| is empty or only whitespace | `Empty` |
| contains any of `* " \ / < > : \| ? # ^ [ ]` | `ForbiddenChar(c)` |
| contains a control character | `ControlChar(u32)` |
| starts with `.` (hidden from Obsidian and the tree) | `LeadingDot` |
| starts/ends with whitespace or ends with `.` (not portable to Windows) | `EdgeWhitespaceOrDot` |
| has a Windows device stem (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, any case) | `Reserved` |
| is longer than 255 bytes | `TooLong` |

`validate_vault_path` applies this to every segment and rejects absolute paths.

### Sanitising (`filename::sanitize_file_name`)

Turns any title into a valid name (without `.md`):

1. `/ \ |` → `-`; `:` → ` -`; other forbidden and control characters → space.
2. Collapse whitespace runs to one space; strip leading dots and trailing dots/spaces.
3. Cut to 200 bytes on a character boundary.
4. Windows device stems get `_` appended (`CON` → `CON_`, `aux.notes` → `aux_.notes`).
5. Empty result → `Untitled`.

| Input | Output |
|---|---|
| `Q3: plan / review` | `Q3 - plan - review` |
| `What? #1 [draft]` | `What 1 draft` |
| `عقد وطنية: نسخة` | `عقد وطنية - نسخة` |

`unique_name(base, taken)` returns `base`, `base 2`, `base 3`, … (case-insensitive).

---

## 2. Document structure

A file is an optional frontmatter block followed by the body. **Parsing never fails**, and
**rendering an unmodified document returns exactly the bytes it was parsed from** (any text,
including invalid YAML, CRLF, a BOM, control characters).

Frontmatter is recognised when the file (after an optional UTF-8 BOM) starts with a line that
is `---` and a later line is `---` (trailing spaces/tabs allowed on both). Otherwise the whole
file is body.

```text
---            ← open delimiter (its line ending decides the ending of lines Strata writes)
id: 01J8…
---            ← close delimiter (may be the last line without a terminator)
body…          ← everything after the close line, byte-for-byte
```

The line ending (`LF` or `CRLF`) of the file's first line is used for every line Strata adds
to frontmatter or body sections. A frontmatter created for a note that had none (e.g. to add
an `id`) uses the body's line ending and is not written if it stays empty.

---

## 3. Frontmatter

YAML 1.2, Obsidian "Properties"-compatible. Values Strata writes are flat: a scalar or a flat
list of scalars; relations are lists of wikilink strings.

### 3.1 Known keys, shapes and canonical order

The canonical order is the table order. Unknown keys follow, in their original order.

| # | Key | Shape | Notes |
|---|---|---|---|
| 1 | `id` | text | ULID, assigned by the backend, never changes |
| 2 | `kind` | text | `concept` \| `person` \| `company` \| `document` \| `place`; absent for ordinary notes |
| 3 | `title` | text | display title |
| 4 | `aliases` | list | every spelling/script |
| 5 | `tags` | list | without `#` (a leading `#` is tolerated on read) |
| 6 | `created` | text | RFC 3339, e.g. `2026-09-27T14:32:00+03:00` |
| 7 | `updated` | text | RFC 3339 |
| 8 | `source` | link | `"[[attachments/2026/09/x.m4a]]"` |
| 9 | `lang` | text | `ar` \| `en` \| `mixed` |
| 10 | `role` | text | person, user-editable |
| 11 | `industry` | text | company |
| 12 | `website` | text | company, user-entered only |
| 13 | `phone` | text | person, user-entered only |
| 14 | `email` | text | person, user-entered only |
| 15 | `address` | text | place, user-entered only |
| 16 | `doc-type` | text | `contract` \| `id` \| `licence` \| `deed` \| `invoice` \| `certificate` \| free text |
| 17 | `copy` | text | `original` \| `certified copy` \| `copy` \| `digital` |
| 18 | `location` | link | a place; `""` when unknown |
| 19 | `holder` | link | a person/party; `""` when none |
| 20 | `last-holder` | link | |
| 21 | `expires` | text | `YYYY-MM-DD` |
| 22 | `status` | text | `stored` \| `checked-out` \| `with-third-party` \| `lost` \| `destroyed` |
| 23–31 | `related`, `part-of`, `supports`, `contradicts`, `follows-up`, `duplicates`, `concepts`, `people`, `companies` | link list | note relations (§6.4); `part-of` also nests places |
| 32–36 | `works-at`, `worked-at`, `reports-to`, `knows`, `introduced-by` | link list | person relations |
| 37–41 | `client-of`, `supplier-of`, `partner-of`, `competitor-of`, `subsidiary-of` | link list | company relations |
| 42 | `copy-of` | link list | document copies |

Keys are case-sensitive (`Title` is an unknown key). Link-list keys are `RelationKey`s; the
same kebab-case names are the sidecar `type` values.

### 3.2 Canonical value syntax

```yaml
id: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H
aliases: [أحمد سمير, Ahmed S., A. Samir]
created: 2026-09-27T14:32:00+03:00
phone: ""
location: "[[Safe — Nasr City office]]"
related: ["[[Churn notes]]", "[[Discount policy|policy]]"]
supports: []
empty-value:
```

- One entry per line: `key: value`, a single space after the colon.
- Lists are flow lists `[a, b]` with `, ` separators; an empty list is `[]`.
- A scalar is written **plain** when it reads back as the identical string, and
  **double-quoted** otherwise. Always quoted: empty strings, wikilinks, strings that would
  read as numbers/booleans/null (`"2024"`, `"true"`, `"yes"`, `"null"`, `"~"`), leading or
  trailing whitespace, `: `, ` #`, leading indicator characters (`- ? : , [ ] { } # & * ! | > ' " % @` and backtick),
  and inside lists `,` `[` `]` `{` `}`. Dates and timestamps stay plain so Obsidian types them.
- Double-quoted escapes: `\"`, `\\`, `\n`, `\r`, `\t`, `\0`, other controls as `\xNN`/`\uNNNN`.
- A null value is `key:`.

### 3.3 Reading

- Values are read with a YAML 1.2 parser. Numbers and booleans keep their source spelling
  (`title: 007` reads as `"007"`).
- `aliases: foo` reads as `[foo]`; a legacy `tags: a, b` string is split on commas/spaces.
- Unquoted wikilinks, which YAML parses as nested lists (`related: [[X]]`,
  `- [[X]]`), read as the wikilink `"[[X]]"`.
- Nested mappings/lists read as `PropertyValue::Other` (raw YAML only).
- A frontmatter that is not valid YAML (duplicate keys, bad indentation, …), is not a
  mapping, contains a bare `\r`, or whose layout the entry splitter cannot map 1:1 to the
  parsed keys is **read-only**: `error()` explains why, typed getters return nothing, every
  edit fails, and it renders verbatim.

### 3.4 Preservation and editing rules

The block is split into top-level entries: a key line plus its continuation lines, with the
comment/blank lines directly above it attached to it. Each entry keeps its raw text.

1. **Untouched document → identical bytes.**
2. **Editing** (`set`, `remove`, typed setters) re-renders only the changed entry, in
   canonical syntax. Setting a value equal to the current one changes nothing.
3. After any edit the block is written in **canonical key order**; untouched entries
   (known or unknown) keep their raw text, comments travel with the entry below them, and
   trailing comments stay at the end.
4. Every edit is **verified**: the resulting YAML must read back to exactly the expected keys
   and values. If canonical order would change a value (e.g. an alias `*a` moved above its
   anchor `&a`), the original order is kept (new keys appended). If no layout reads back
   correctly the edit is refused with `Unsupported` and nothing changes.
5. `render_canonical()` additionally rewrites every known key in canonical syntax (e.g.
   block lists become flow lists). Unknown keys are never rewritten. A canonical document
   renders byte-identically under both `render` and `render_canonical`.

Example — adding `concepts` to an Obsidian-written note:

```yaml
---                                        ---
# Written by Obsidian                      id: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H
tags:                                      aliases: pricing tests
  - pricing                                # Written by Obsidian
cssclasses: [wide]                         tags:
related:                                     - pricing
- "[[Churn notes]]"               →        related:
id: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H             - "[[Churn notes]]"
aliases: pricing tests                     concepts: ["[[Pricing]]"]
---                                        cssclasses: [wide]
                                           ---
```

---

## 4. Body syntax

The body is parsed as CommonMark + GFM (tables, strikethrough, task lists, footnotes) +
`$…$`/`$$…$$` math. **Fenced and indented code blocks, inline code and math are inert**:
nothing inside them is a link, tag, block ID or task.

### 4.1 Wikilinks and embeds

| Form | Parsed as |
|---|---|
| `[[Note]]` | path `Note` |
| `[[Note\|alias]]` | alias `alias` |
| `[[Note#Heading]]` | heading anchor `Heading` |
| `[[Note#H1#H2]]` | heading anchor `H1#H2` (nested) |
| `[[Note#^a1b2]]` | block anchor `a1b2` |
| `[[#Heading]]` | anchor in the current note (empty path) |
| `[[folder/Note]]`, `[[Note.md]]` | path with folder / extension |
| `![[Note]]`, `![[image.png]]`, `![[file.pdf]]`, `![[Note#^id\|x]]` | embeds |
| `[[Note\|alias]]` inside a table | the escaped pipe `\|` is recognised and preserved |

Rules: a link is on one line; it closes at the first `]]`; the target may not contain `[` or
`]`; `\[[x]]` is not a link. `WikiLink` gives spans for the whole link, the path, the anchor
and the alias.

### 4.2 Tags

`#tag` is a tag when the `#` is at the start of a line or after whitespace, and is followed by
tag characters: letters and digits of any script (Arabic included), Arabic/Latin combining
marks, ZWNJ/ZWJ, `_`, `-`, `/` (nested tags: `#project/strata`). A tag must contain at least
one character that is not a digit, `_`, `-` or `/` (`#123` is not a tag). `# Heading` is a
heading, not a tag; tags inside heading text count. Tags inside wikilinks, URLs (`/#frag`),
code and math do not.

### 4.3 Headings

ATX (`## Title ##` → `Title`) and setext headings. Each heading has a level, its text and its
**path** (texts of enclosing headings plus its own).

### 4.4 Blocks and block IDs

Blocks are the citable units: paragraphs, headings, list items (each item without its nested
lists; nested items are blocks of their own), whole blockquotes/callouts, code blocks, tables,
HTML blocks, footnote definitions. Each has a span and its heading path.

A block ID is `^id` (read as `[A-Za-z0-9-]+`; **written** as `[a-z0-9-]+`):

- at the end of the block's last line, preceded by whitespace: `Prefers weekly invoicing ^a1b2`;
- or, for any block, as a paragraph consisting only of `^id` right after it (Obsidian's form
  for tables, quotes and code).

`price^2` is not a block ID.

### 4.5 Appending block IDs (`blocks::append_block_id`)

The only automated edit to prose. It inserts ` ^id` after the last character of a paragraph,
list item or footnote (before any trailing whitespace), or `<eol><eol>^id` after a quote,
table, code or HTML block. Nothing else changes. Refused for headings, invalid IDs, blocks
that already have an ID, and IDs already used in the note.

---

## 5. Link resolution (`PathIndex::resolve`)

Case-insensitive, like Obsidian:

1. Empty path (`[[#H]]`) → `CurrentNote`.
2. `./x`, `../x` → relative to the linking note's folder (exact match only).
3. A path with `/` → exact vault path (with or without `.md`), else **path-suffix** match
   (`[[2026/Meeting]]` matches `notes/2026/Meeting.md`).
4. A bare name → files whose link name matches: the file name without `.md` for notes, the
   full file name for attachments (`[[scan.pdf]]`). `[[v1.2 plan]]` finds `v1.2 plan.md`.
5. Several matches are narrowed to exact-case matches, then to files in the linking note's
   folder. If more than one remains → `Ambiguous(sorted paths)`; none → `Unresolved`.

**Writing links** (`link_text_for`): the bare link name when no other file shares it,
otherwise the full vault path without `.md` ("shortest path when possible").

## 6. Rename/move rewriting (`rewrite::MoveSet`)

Given the path index before and after, and the old → new path of every moved file (a folder
move lists each file):

- A link is rewritten only if it **resolved to exactly one file before** and would not resolve
  to that file's new path afterwards (its target moved, its source note moved, or a new file
  now shares its name). Unresolved and ambiguous links are left for the user.
- The new path is `link_text_for(new target)`; `.md` is kept if the link had it; embed marker,
  anchor, alias and `\|` are preserved: `![[Old#^b1|x]]` → `![[New#^b1|x]]`.
- Links in code are untouched.
- Frontmatter: values of link-holding known keys (all relation lists, `source`, `location`,
  `holder`, `last-holder`) are rewritten the same way. **Unknown keys are never touched.**
- Canvas file nodes follow with `Canvas::rename_file`.

---

## 7. AI-owned sections (PLAN §6.6, §6.7, §6.12)

| Note kind | AI sections, in order | User-owned |
|---|---|---|
| concept | `## Summary` | everything else |
| person, company | `## Summary`, `## Insights`, `## Open items`, `## Timeline` | `## Notes` and any other heading |
| document | `## Summary`, `## Custody` | `## Notes` and any other heading |

`sections::replace_ai_sections(body, profile, updates)`:

- An AI section's **own content** runs from the line after its heading to the next heading of
  **any** level; only that range is replaced, so user sub-headings under an AI section, `## Notes`
  and all other text are preserved byte-for-byte. The heading line itself is kept as written.
- Missing sections are inserted in profile order: before the next existing AI section, else
  after the previous one, else before the first `#`/`##` heading, else at the end.
- Written content ends with the body's line ending, plus a blank line when a heading follows.
- Headings are matched case-insensitively at level 2. A body containing the same AI heading
  twice is refused (`Duplicate`).

**Validation** (`validate_content`), all violations reported with 1-based line numbers:

- No headings in AI content.
- Insights, Open items, Timeline, Custody: every non-blank line is a `- ` bullet with at least
  one non-embed wikilink citation: `- Prefers weekly invoicing [[Call 2026-09-12#^a1b2]]`.
- Timeline and Custody bullets start with a `YYYY-MM-DD` date and are newest first:
  `- 2026-09-12 — call about invoicing [[Call 2026-09-12]]`.
- Custody bullets must parse as custody events (§8).
- Summary is prose; citations are not required.

---

## 8. Custody events (PLAN §6.12)

One bullet per event in `## Custody`, newest first:

```text
- <YYYY-MM-DD> — <type>[ <primary>][ at|to|in <place>][ by <person>][ with|from <party>] — <citation> [<citation> …]
```

```markdown
- 2026-09-20 — returned-by [[Shady]] to [[Safe — Nasr City office]] — [[Capture 2026-09-20#^c1d2]]
- 2026-09-10 — handed-to [[Shady]] — [[Capture 2026-09-10#^b7c8]]
- 2026-01-15 — stored-at [[Safe — Nasr City office]] — [[Capture 2026-01-15]]
```

| Type | Primary argument (required) | Resulting frontmatter |
|---|---|---|
| `stored-at` | place | location = place, holder = —, status `stored`; last-holder = `by` if given |
| `moved-to` | place | same as `stored-at` |
| `handed-to` | person | holder = last-holder = person, location = `at` place or —, `checked-out` |
| `returned-by` | person | holder = —, last-holder = person, location = `to` place or —, `stored` |
| `sent-to` | party | holder = last-holder = party, location = —, `with-third-party` |
| `received-from` | party | location = place, holder = `by` person, last-holder = person or party; `checked-out` if a person holds it, else `stored` |
| `lost` | — | location = holder = —, `lost`; last-holder = `by` if given |
| `found` | — | as `received-from` |
| `destroyed` | — | location = holder = —, `destroyed` |

- Separators are ` — ` (em dash); ` – ` and ` - ` are accepted on read. Dashes inside
  wikilinks (`[[Safe — Nasr City office]]`) are not separators.
- The canonical writer uses `to` for the place of `returned-by`, `at` otherwise, `by` for
  persons and `with` for parties.
- `CustodyState::derive(events)` folds events oldest → newest (same-date events in file order,
  bottom to top) and `write_to(frontmatter)` writes `location`, `holder`, `last-holder`,
  `status` (empty values as `""`). Frontmatter always equals the result of the newest event.

---

## 9. Tasks (PLAN §6.11)

### 9.1 Line syntax

```markdown
- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2
```

- Prefix: indentation and `>` quote markers, a list marker (`-`, `*`, `+`, `1.`, `1)`), space,
  `[c]`, then a space or end of line.
- Status: `[ ]` todo, `[x]`/`[X]` done, `[-]` cancelled, any other character (e.g. `[/]`)
  kept as a custom open status.
- Fields (recognised **anywhere** in the line, not only at the end):

| Field | Syntax |
|---|---|
| priority | `🔺` highest, `⏫` high, `🔼` medium, `🔽` low, `⏬` lowest; followed by whitespace/end |
| recurrence | `🔁 <phrase>`; the phrase is the run of `[A-Za-z0-9, !]` after it (trailing spaces/commas trimmed) |
| created | `➕ YYYY-MM-DD` |
| start | `🛫 YYYY-MM-DD` |
| scheduled | `⏳ YYYY-MM-DD` (also `⌛`) |
| due | `📅 YYYY-MM-DD` (also `📆`, `🗓`) |
| cancelled | `❌ YYYY-MM-DD` |
| done | `✅ YYYY-MM-DD` |
| reminder | `(@YYYY-MM-DD HH:mm)` or `(@YYYY-MM-DD)` (default time); several allowed |
| block ID | `^id` at the end of the line, e.g. `^t-01j9a2` (task identity) |

  An emoji may carry U+FE0F. A date must be a real date followed by whitespace or end.
  If a single-valued field appears twice, the **last** wins (the Tasks plugin reads from the
  end); the earlier one stays text.
- Everything else (words, links, tags, invalid fields) is the description and is preserved.
- **Edits are span edits**: changing a field rewrites only its bytes; the rest of the line is
  untouched. New fields are inserted in the Tasks plugin's order relative to existing fields
  (priority, 🔁, ➕, 🛫, ⏳, 📅, ❌, ✅); if an insertion would not read back (e.g. a recurrence
  phrase that would absorb following words) it goes at the end of the content, before the
  block ID. Removing a field also removes earlier duplicates.
- **Canonical new lines** (`TaskSpec::render`): description, reminders, Tasks fields in plugin
  order, block ID:

```markdown
- [ ] Petrol Arrows invoice [[Petrol Arrows]] (@2026-09-27 10:00) 🔼 🔁 every week on Sunday 📅 2026-09-27 ^t-01j9a3
```

### 9.2 Recurrence grammar → RRULE

Case-insensitive; list items separated by `,`, `and` or `, and`. Weekdays and months may be
abbreviated (`mon`, `jan`); ordinals `1st`…`31st`, bare numbers, or `first`…`fifth`.

| Phrase | RRULE |
|---|---|
| `every day` / `every 3 days` / `every other day` | `FREQ=DAILY` / `FREQ=DAILY;INTERVAL=3` / `INTERVAL=2` |
| `every weekday` | `FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR` |
| `every week` / `every 2 weeks on Monday, Thursday` | `FREQ=WEEKLY` / `FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,TH` |
| `every Monday and Friday` | `FREQ=WEEKLY;BYDAY=MO,FR` |
| `every month on the 1st` / `on the 1st and 15th` | `FREQ=MONTHLY;BYMONTHDAY=1` / `BYMONTHDAY=1,15` |
| `every month on the last` / `on the last day` | `FREQ=MONTHLY;BYMONTHDAY=-1` |
| `every month on the 31st` | `FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1` |
| `every month on the 2nd Wednesday` / `last Friday` / `2nd last Friday` | `BYDAY=2WE` / `BYDAY=-1FR` / `BYDAY=-2FR` |
| `every 6 months on the 2nd Wednesday` | `FREQ=MONTHLY;INTERVAL=6;BYDAY=2WE` |
| `every year` / `every 2 years` | `FREQ=YEARLY` / `FREQ=YEARLY;INTERVAL=2` |
| `every year on March 31` (`on 31 March`, `on the 31st of March`) | `FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=31` |
| `every year on February 29` | `FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=28,29;BYSETPOS=-1` |
| `every January on the 15th` | `FREQ=YEARLY;BYMONTH=1;BYMONTHDAY=15` |
| `every April and December on the 1st and 24th` | `FREQ=YEARLY;BYMONTH=4,12;BYMONTHDAY=1,24` |
| … `when done` | same RRULE; next date counts from the completion date |

- `to_rrule()` omits parts RFC 5545 takes from `DTSTART`; `to_rrule_anchored(date)` spells
  them out (`every week` from a Sunday → `BYDAY=SU`; `every month` from the 31st →
  `BYMONTHDAY=28,29,30,31;BYSETPOS=-1`).
- **Month-end clamping:** a day that does not exist in a month is that month's last day
  ("31st" is 28/29 February, 30 April). A single such day compiles to the exact RFC 5545
  equivalent with `BYSETPOS=-1`. (A list containing such a day, e.g. `on the 15th and 31st`,
  compiles to a plain `BYMONTHDAY` list; `next_after` still clamps.)
- Impossible dates are rejected (`every year on February 30`, `every April on the 31st`).
- **Anything else is `RecurrenceNotUnderstood { phrase, reason }`.** The phrase is never
  rewritten; the task is flagged.

### 9.3 Next occurrence

`RecurrenceRule::next_after(reference)` (= `tasks::next_occurrence`) is the first occurrence
**strictly after** the reference in the series starting at the reference (Tasks plugin
semantics). Weekly intervals count ISO weeks (Monday start) from the reference's week. Plain
`every month` from the 31st drifts after a short month (31 Jan → 28 Feb → 28 Mar), exactly as
the plugin does; use `on the last day` for month ends.

Times: reminders are wall-clock times in the user's time zone. `resolve_local(naive, tz)` maps
a wall-clock time to an instant: a time repeated by a DST fall-back → its **first**
occurrence; a time skipped by a spring-forward → the same distance after the transition
(London 01:30 on 29 Mar 2026 → 02:30 BST; Cairo 00:30 on 24 Apr 2026 → 01:30 EEST).
`next_occurrence_at(rule, reference, tz)` advances the date and keeps the wall-clock time.

### 9.4 Completing a recurring task (`tasks::complete_recurring`)

Returns the two lines that replace the task, **new occurrence first** (it goes above):

```markdown
- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-11-01 (@2026-11-01 09:00) [[Watanya]] ^t-01j9b0
- [x] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 ✅ 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2
```

1. Reference date = 📅 due, else ⏳ scheduled, else 🛫 start (error `NoReferenceDate` if none).
2. Next reference = `next_after(reference)`, or `next_after(done_date)` for `when done`.
3. Offset = next reference − reference (days). Start/scheduled/due move by the offset;
   reminders move by the offset **and keep their wall-clock time**.
4. New line: status `[ ]`, ✅/❌ removed, ➕ set to the completion date if present, block ID
   replaced by the given new ID; every other byte kept.
5. Done line: `[x]` and `✅ <done date>` (inserted after the last Tasks field), block ID kept.

Errors: `NotATask`, `NotOpen`, `NotRecurring`, `RecurrenceNotUnderstood`, `NoReferenceDate`,
`NoNextOccurrence`, `InvalidBlockId`. `complete`, `cancel` (`[-]` + `❌`) and `reopen`
(`[ ]`, ✅/❌ removed) handle the other transitions.

---

## 10. Sidecar metadata

### 10.1 `.meta/notes/<id>.json` (`sidecar::NoteSidecar`)

```json
{
  "id": "01J8ZK3M4X7Q9W2E5R6T8Y0V1H",
  "summary": "Ahmed runs operations at Acme Logistics.",
  "relations": [
    {
      "type": "works-at",
      "target_id": "01J8ZKACME00000000000000AB",
      "by": "ai",
      "confidence": 0.91,
      "reason": "Signs as Operations manager, Acme Logistics.",
      "model": "claude-code/opus",
      "created": "2026-09-27T15:12:00+03:00"
    }
  ],
  "rejected": [
    { "type": "related", "target_id": "01J8ZKB0000000000000000000", "at": "2026-09-27T16:00:00+03:00" }
  ],
  "keep_both": [
    { "other_id": "01J9…", "at": "2026-09-28T10:00:00+03:00" }
  ],
  "content_hash": "sha256:…",
  "last_linked_hash": "sha256:…"
}
```

- Written with two-space indentation and a trailing newline, fields in the order above;
  optional fields (`summary`, `confidence`, `reason`, `model`, hashes) are omitted when absent,
  `keep_both` when empty. `type` is a relation key (§3.1); `by` is `ai` | `user`.
- Unknown fields are preserved (sorted by name, after the known ones).
- `is_blocked(type, target)`: the AI may not add a rejected (type, target) pair, and may not
  add `related` to a target for which any type was rejected.

### 10.2 `.meta/clusters.json` (`clusters::Clusters`)

```json
{
  "version": 1,
  "generated": "2026-09-27T03:00:00+03:00",
  "algorithm": "leiden",
  "clusters": [
    { "id": 1, "name": "Logistics", "named_by": "ai", "notes": [] },
    { "id": 2, "name": "التسعير", "named_by": "user", "notes": ["01J…", "01J…"] }
  ]
}
```

Clusters are written sorted by `id`, members sorted. A `named_by: user` name is never replaced
by the AI.

---

## 11. Canvas layouts (`maps/*.canvas`, JSON Canvas 1.0)

- Nodes: `text` (`text`), `file` (`file` = vault path, optional `subpath` starting with `#`),
  `link` (`url`), `group` (`label`, `background`, `backgroundStyle` = `cover`|`ratio`|`repeat`);
  all have `id`, integer `x`, `y`, `width`, `height`, optional `color` (`"1"`–`"6"` or
  `#RRGGBB`). Nodes are in z-order.
- Edges: `id`, `fromNode`, `fromSide`, `fromEnd` (`none` default), `toNode`, `toSide`,
  `toEnd` (`arrow` default), `color`, `label` (Strata writes the relation type).
- Written in Obsidian's layout: tab indentation, one object per line, no trailing newline:

```json
{
	"nodes":[
		{"id":"n1","type":"file","file":"notes/Pricing experiments.md","x":-300,"y":-200,"width":400,"height":300},
		{"id":"t1","type":"text","text":"Pinned by the user","x":200,"y":50,"width":250,"height":60}
	],
	"edges":[
		{"id":"e1","fromNode":"n1","fromSide":"right","toNode":"t1","toSide":"left","label":"related"}
	]
}
```

- Unknown fields are preserved. `validate()` reports duplicate IDs, dangling edges, bad
  colours, subpaths without `#`, and non-positive sizes.

---

## Decisions beyond PLAN

These fill gaps in PLAN §6 or depart from its illustrative examples; the owner may revisit
them (see `docs/DECISIONS.md`).

1. **One global canonical key order.** PLAN's person and document examples place
   `companies`/`people` between entity fields; Strata writes all entity fields before all
   relation keys (§3.1), so every kind shares one order.
2. **Canonical task field order is the Tasks plugin's** (fields at the end of the line,
   reminders before them), because the plugin only reads fields at the end of a line. PLAN's
   example with `(@…)` and `[[…]]` after `📅` is fully supported on read and preserved on edit.
3. **Custody line grammar** (§8) is Strata's; PLAN gave a prose example.
4. **Derived custody state rules** (§8 table) for types whose effect PLAN does not spell out.
5. **`keep_both`** in the sidecar mirrors duplicate "keep both" decisions (PLAN §7.4 says they
   are mirrored in `.meta/` without naming a place). `clusters.json` schema is Strata's.
6. **Month-end clamping** instead of RFC 5545's skipping of non-existent days; drift of plain
   `every month` matches the Tasks plugin.
7. **Ambiguous names** prefer exact case, then the linking note's folder; otherwise they are
   reported and never rewritten.
8. **Links in unknown frontmatter keys are not rewritten** on rename (unknown keys are
   preserved exactly, PLAN §6.4).
9. **Recurring completion sets ➕ to the completion date** on the new line when the task has
   a created date.
10. Markdown-style links (`[text](Note.md)`) are not treated as vault links.

## Robustness notes

- `pulldown-cmark` 0.13.4 panics on some inputs (a list item starting with a link reference
  definition followed by certain whitespace-only lines, e.g. `"- [a]: b\n        "`). Strata
  parses a copy with whitespace-only lines emptied (offsets mapped back) and treats any
  remaining parser panic as "plain text body" rather than failing.
- Fuzz targets (`crates/vault-format/fuzz`: `document`, `frontmatter`, `task_line`, `body`)
  run with `cargo +nightly fuzz run <target>`; every crash they found is a regression test
  in `tests/fuzz_regressions.rs`.

# Strata screen design spec (shared by every screen artboard)

Strata is a personal, AI-powered knowledge app. Notes are Obsidian-compatible markdown. AI files captures, links notes with typed relations, extracts concepts, and keeps first-class **People** and **Companies** pages (summary, insights, open items, timeline — every AI bullet cites a source note). Ask = chat over the vault with clickable citations. Content is mixed **Arabic (Egyptian) and English**; direction is per paragraph (`dir="auto"`).

The client is **one Flutter app** for Android, iOS, macOS, Windows, Linux. Layouts are **adaptive** (different structure per window size class), not just responsive stretching. All platforms are offline-first (local cache + outbox), so a **sync indicator** is always visible.

## Size classes (artboard sizes)

| Class | Artboard | Navigation | Panes |
|---|---|---|---|
| Compact (phone, <600) | 390 × 844 | Bottom navigation bar (5 items: Home, Inbox, Notes, People, Ask). Top app bar with title, search icon button, sync pill. | One pane. Detail opens full-screen with back button. Context (backlinks, relations) in bottom sheets / tabs. |
| Medium (tablet, narrow laptop window, 600–1199) | 1024 × 768 | Navigation rail, 80 px wide, icon + label, 6 items (Home, Inbox, Notes, Map, People, Ask), capture button at top of rail, Settings at bottom. | Two panes: list (320 px) + detail. Context panel toggles as an overlay drawer from the right. |
| Expanded (desktop/laptop, ≥1200) | 1440 × 900 | Sidebar 248 px: wordmark row, "New capture" button, nav items with counts, "Pinned" notes, folder tree, sync status block at the bottom. | Up to three panes: list (320) + main + context panel (340) with backlinks / relations / local graph / history. Desktop gets keyboard hints (⌘K search, ⌘N capture) and hover states. |

Nothing is ever just a stretched phone layout. At expanded size use the extra room for a third pane, denser lists, and side-by-side views.

## Tokens

Light (default):
- bg `#F1F5F7` (app background, "mist")
- surface `#FFFFFF` (cards, panes)
- surface-2 `#E8EFF2` (sidebar, rail, hover rows, inputs)
- border `#CBD8DE` (1px hairlines)
- text `#0F1B26`, text-2 `#52616B` (secondary, min for small text), text-3 `#7C8C96` (only ≥ 16px or icons)
- accent `#2477B3` (tide — the mark, icons, focus rings, selected indicators, large graphics); **accent fill for filled buttons/FAB `#1D5C8C` with white text** (light) and surf `#6CB4DD` with abyss text (dark), so button text stays ≥ 4.5:1 incl. hover/pressed overlays, accent text `#1D5C8C` (links, small accent text), accent tint `#DCEAF4` (selected row / nav pill background)
- sand `#D8B47E` (only the thin seam in strata-band decoration; do not use elsewhere)
- danger `#B3412E`, warning `#9A6A12`, success `#2F7A55`

Dark (used for a few artboards):
- bg `#0F1B26`, surface `#15283A`, surface-2 `#1B3044`, border `#23384A`
- text `#F1F5F7`, text-2 `#93A3AD`
- accent `#2477B3` (mark, icons, large graphics), accent fill for filled buttons `#6CB4DD` with abyss `#0F1B26` text, accent text `#6CB4DD` (surf), accent tint `#1B3A55`

Radii: 8 (inputs, chips 999), 12 (cards), 16 (sheets). Spacing on a 4 px grid; common gaps 8/12/16/24. Elevation: prefer hairline borders; one soft shadow `0 8px 24px rgba(15,27,38,0.12)` only for sheets/popovers.

Type (load in each artboard's `<helmet>`):
`<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Cairo:wght@400;500;600;700&amp;family=IBM+Plex+Mono:wght@400;500&amp;family=Quicksand:wght@700&amp;display=swap">`
- UI and all content: `'Cairo', sans-serif` (Latin + Arabic). Sizes: 12 caption, 14 body-sm, 15/16 body, 18 title-sm, 22 title, 28 display. Line-height 1.5 for body (Arabic needs it).
- Technical (IDs, timestamps, markdown source, frontmatter keys, keyboard hints): `'IBM Plex Mono', monospace`, 12–14.
- Wordmark "strata": `'Quicksand', sans-serif` 700, in text colour. The symbol (below) in tide.

Symbol (inline SVG; stroke tide `#2477B3`):
`<svg width="24" height="24" viewBox="0 0 64 64" fill="none" aria-hidden="true"><path d="M50 13 H24 A9.5 9.5 0 0 0 24 32 H40 A9.5 9.5 0 0 1 40 51 H14" stroke="#2477B3" stroke-width="11" stroke-linecap="round" stroke-linejoin="round"></path></svg>`

Icons: inline stroke SVGs only, 20×20 viewBox 0 0 24 24, stroke-width 1.75, round caps/joins, `stroke="currentColor"`, clean precise geometric lines (no hand-drawn look, no emoji, no filled clip-art). Icon-only buttons need `aria-label`.

## Relation types (typed chips + graph edges) — meaning never by colour alone

Chip = pill with a small line-sample glyph + label. Colours:
| type | colour | line |
|---|---|---|
| related | `#52616B` | solid 1.5 |
| part-of | `#0F1B26` | solid 2, arrow |
| supports | `#2F7A55` | solid 2, arrow |
| contradicts | `#B3412E` | dashed 6-4, arrow, small × at midpoint |
| follows-up | `#1D5C8C` | dotted 2-3, arrow |
| duplicates | `#7C8C96` | double parallel lines |
| similarity (AI, ephemeral) | `#7C8C96` at 40% opacity | dotted 1-4, thin |
| body link | `#9AAAB3` | hairline 1 |
| mention (note→person/company) | same colour as entity kind, 1px |

AI-made relations show a small "AI" tag and confidence (e.g. `AI · 0.82`); user-made show nothing extra. Every AI chip/edge offers reject / retype on hover (desktop) or long-press sheet (phone).

## Node kinds (graph)

| kind | shape | colour (light) |
|---|---|---|
| note | circle, radius by degree | fill `#0F1B26` (dark: `#F1F5F7`) |
| concept | diamond (rotated square) | fill `#DCEAF4`, stroke `#2477B3` 2px |
| person | circle with outer ring (2 px gap) | `#2F7A55` |
| company | rounded square | `#9A6A12` |
| cluster | soft region: rounded blob/ellipse fill `#2477B3` at 7% + dashed `#7C8C96` 1px outline, label in text-2 caps 12 |

Labels: shown for hovered/selected/high-degree nodes; Cairo 12–13 with a mist halo. Selected node: 3px tide ring. Everything clean vector — straight or smoothly curved edges, no wobble.

## Sample content (use consistently; realistic, no lorem ipsum)

Today is Sunday 27 Sep 2026. Owner: Shawket.
Notes: "Pricing experiments" (notes/sales), "Churn notes", "Subscription tiers", "Discount policy", "Call 2026-09-12 — Acme", "Weekly invoicing proposal", "Q4 hiring plan", "تجارب التسعير — ملخص", "Onboarding checklist v2".
Concepts: Pricing, Loyalty, Churn, Invoicing.
People: Ahmed Samir (أحمد سمير) — Operations manager, Acme Logistics; Mona Hassan (منى حسن) — Finance lead, Nile Freight; Karim Adel — Founder, Delta Foods; Sara Nabil — Designer (contractor).
Companies: Acme Logistics (أكمي) — client, logistics; Nile Freight — supplier; Delta Foods — prospect.
Inbox captures (mix scripts), e.g.
- "كلمت أحمد النهارده، عايزين invoicing أسبوعي بدل شهري ابتداءً من أكتوبر" → AI proposes title "Weekly invoicing request — Acme", folder notes/clients/acme, tags [invoicing, acme], links → Ahmed Samir, Acme Logistics, follows-up [[Call 2026-09-12 — Acme]].
- "Idea: loyalty tier for customers > 12 months, 5% off renewals" → contradicts [[Discount policy]] (caps discounts at 3%), part-of [[Subscription tiers]].
- "Mona said Nile Freight rates go up 8% in November" → links Mona Hassan and Nile Freight; adds ONLY a Timeline entry to their pages ("Nov 2026 — rates +8%"). A one-line capture never produces insights, open items, or speculative relations.
- "بابا عايز يشوف الأرقام بكرة" (a nickname/kinship term) → NOT auto-linked. Shown as an entity **link-or-create suggestion**: "Who is “بابا”?" with candidate people to link (e.g. none match) or "Create person…"; accepting adds "بابا" to that person's aliases so future mentions resolve. Timeline date resolves "بكرة" to Mon 28 Sep 2026 (the capture was created 27 Sep).
Sync states: "Synced · 14:32", "Offline · 3 changes queued", "Syncing 12/40", "1 conflict".
AI states: provider online; "AI paused — daily budget reached" banner.

## .dc.html format — rules that fail silently if broken

Each artboard is one file `project/<Name>.dc.html`. Skeleton (copy exactly, change title/size/content):

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Home · compact</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Cairo:wght@400;500;600;700&amp;family=IBM+Plex+Mono:wght@400;500&amp;family=Quicksand:wght@700&amp;display=swap">
<style>
body{margin:0;font-family:'Cairo',sans-serif;background:#F1F5F7;color:#0F1B26}
a{color:#1D5C8C}a:hover{color:#164A72}
</style>
</helmet>
<div style="width: 390px; height: 844px; box-sizing: border-box; overflow: hidden; position: relative; display: flex; flex-direction: column; background: #F1F5F7; color: #0F1B26; font-family: 'Cairo', sans-serif;">
  ... content ...
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{"$preview":{"width":390,"height":844}}'>
class Component extends DCLogic {
  renderVals() { return {}; }
}
</script>
</body>
</html>
```

- Keep `<script src="./support.js"></script>` exactly. Close every non-void element; quote every attribute. Root element has a FIXED width/height equal to the artboard, same as `$preview`.
- All styling inline `style="…"` (the helmet `<style>` only for body basics and `a` colours). Layout with flex/grid + gap. Grid tracks: `repeat(N, minmax(0, 1fr))`.
- Static mockups: copy is literal markup (no `{{holes}}`), `renderVals() { return {}; }`. No script-built UI, no innerHTML, no iframes, no emoji, no external images (draw avatars as initials in circles; graph drawings as inline SVG).
- Accessible markup: real `<button>`, `<a href>`, `<input>` with `<label>` (or aria-label), `<nav>`, `<main>`, `<aside>`, headings. Arabic text blocks: `lang="ar" dir="rtl"` (or `dir="auto"` for mixed paragraphs). Text contrast ≥ 4.5:1 (use text-2 `#52616B` minimum for small grey text on light; `#93A3AD` on dark).
- No fake OS status bars. Phone artboards may leave ~24px top padding instead.
- `<svg>` elements need closing tags for every child (`<path ...></path>`, `<circle ...></circle>`).
- Write files with the Write tool directly (no shell, no scripts that generate files). Write each artboard once, completely.

## Screens and file names (who builds what is in your task)

Foundations page:
- `LayoutSystem.dc.html` 1440×900 — the three size classes side by side (scaled-down wireframes of compact / medium / expanded with their nav patterns and pane counts labelled), breakpoints, and a short rules list.
- `GraphLanguage.dc.html` 1440×900 — node kinds legend, edge types legend (line samples + labels), states (hover, selected, AI edge with confidence, rejected), and a small sample cluster graph.

Screens page:
- Home & capture: `HomeCompact` 390×844, `HomeMedium` 1024×768, `HomeExpanded` 1440×900
- Inbox (AI filing suggestions: accept / edit / reject, bulk): `InboxCompact` 390×844, `InboxExpanded` 1440×900
- Note editor (markdown editor with live styling, properties panel with typed relation chips, backlinks grouped by type, history, local mini-graph): `NoteCompact` 390×844, `NoteMedium` 1024×768, `NoteExpanded` 1440×900, `NoteExpandedDarkAr` 1440×900 (dark theme, Arabic-content note "تجارب التسعير — ملخص", UI still English, RTL paragraphs)
- Local mind map (focused note centre, radial relations, edge labels, hover reason, edit edge): `MindMapCompact` 390×844, `MindMapExpanded` 1440×900
- Global map (clusters as labelled regions, filters panel, search-to-focus, zoom controls): `GlobalMapMedium` 1024×768, `GlobalMapExpanded` 1440×900
- People & companies directory: `PeopleCompact` 390×844, `PeopleExpanded` 1440×900
- Entity page (Ahmed Samir: properties & aliases, Summary, Insights, Open items, Timeline — each bullet cites a source; mentioning notes; related entities; mini entity graph; merge action): `EntityCompact` 390×844, `EntityExpanded` 1440×900
- Ask (chat with streaming answer + citations `[[Note#^block]]` as chips, "save as note"): `AskCompact` 390×844, `AskExpandedDark` 1440×900 (dark)
- Accounts (multi-user, self-signup with admin approval): `LoginCompact` 390×844 (username, password, device name; link to sign up; no server field, the address is fixed per build), `LoginExpanded` 1440×900 (split: brand panel with strata bands + form), `SignupCompact` 390×844, `PendingApprovalCompact` 390×844 ("Waiting for approval" state after signup; also the "not approved" variant as a note), `AccountSheetCompact` 390×844 (account sheet over Settings: user, role, device, Sign out; the sign-out warning dialog shown: "3 changes haven't synced yet — Sync now / Sign out anyway / Cancel"), `AccountDisabledCompact` 390×844 and `AccountDisabledExpanded` 1440×900 ("This account was disabled by an administrator"; local data will be removed; export of unsynced changes offered), `AdminUsersExpanded` 1440×900 (Admin → Users: pending approvals queue at top with Approve / Reject, then users table with role, status, devices, last active, actions: disable, reset password, delete with export), `AdminUsersCompact` 390×844.
- Sync & conflicts: `SyncCompact` 390×844 (sync status sheet: offline, 3 queued ops, last sync, 1 conflict), `ConflictExpanded` 1440×900 (side-by-side 3-way conflict resolution for a note body), `SyncMedium` 1024×768 (sync status as the context drawer over the notes list: syncing 12/40 progress, queued ops, last sync), `SyncExpanded` 1440×900 (desktop: sidebar sync block expanded into a popover with outbox list and conflict row; "Offline · 3 queued")

## Additions (tasks, reminders, duplicates, documents & places)

Navigation update:
- Compact bottom bar stays at 5: Home, Inbox, Notes, Directory (was "People"; holds People / Companies / Documents / Places tabs), Ask. Tasks live on **Home** as Today / Upcoming / Recurring sections.
- Medium rail and expanded sidebar add **Tasks** (after Inbox) and rename People to **Directory**. Rail and sidebar scroll when short.

Node kinds (add): document = page shape with folded corner, fill `#3F4C55` (dark `#B9C8D0`); place = map-pin outline, stroke `#1D5C8C` (dark `#6CB4DD`).
Dark-theme entity colours: person `#6CC495`, company `#C99A3E`.
Tints: success `#EAF4EE`, warning `#F5ECDA` (text `#6E4B0C`), danger `#F6E3DF` (text `#9A3624`), info = accent tint `#DCEAF4`.

Tasks (Obsidian Tasks lines; the UI never shows the raw emoji syntax except in the markdown editor):
- "Make Watanya's ETA invoice" — every month on the 1st, due Thu 1 Oct 2026, reminder 09:00, linked company Watanya; history: done 1 Sep, done 1 Aug (late, 3 Aug).
- "Petrol Arrows invoice" — every week on Sunday, due today Sun 27 Sep 10:00, linked company Petrol Arrows.
- "Send weekly invoicing proposal to Ahmed" — one-off, due Tue 29 Sep, linked Ahmed Samir / Acme Logistics.
- "Renew Watanya contract" — due 31 Mar 2027 (from document expiry), reminder 30 days before.
- Overdue: "Pay Nile Freight September invoice" — due Thu 24 Sep.
Reminder notification copy: "Petrol Arrows invoice · due today 10:00" with actions Done / Snooze.
Duplicate example: capture/create "remind me to make watanya's invoice" → "Already exists: Make Watanya's ETA invoice · monthly on the 1st · next Thu 1 Oct" (match: near, 0.91) → Open existing / Create anyway / Cancel. Notes example: new note "Pricing experiment" → matches "Pricing experiments" (exact after normalisation). People example: "Ahmed Sameer" → matches Ahmed Samir (alias near match).

Documents & places:
- Places: Nasr City office (مكتب مدينة نصر) › Safe — Nasr City office; Nasr City office › Cabinet B; Home › Desk drawer; Accountant's office (Hany Youssef).
- Documents: "Watanya contract" (original, contract, company Watanya) — location Safe — Nasr City office, holder none, last holder Shady (شادي), status stored, expires 31 Mar 2027; custody: 20 Sep 2026 returned to the safe by Shady (cited capture), 14 Sep 2026 handed to Shady for signing (cited), 2 Mar 2026 stored at Safe (cited). "Watanya contract — copy" (copy, with Accountant's office). "Petrol Arrows commercial register" (original, Cabinet B, expires 15 Jan 2027). "Car licence" (original, holder Shawket, status checked-out).
- AI custody examples: capture "عقد وطنية في الخزنة في مكتب مدينة نصر، آخر واحد كان معاه شادي" → applied automatically (confidence 0.93) with an "AI · undo" affordance in the activity feed; capture "gave the contract to Shady" → suggestion (ambiguous: which contract? Watanya contract / Petrol Arrows contract).

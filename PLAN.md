# Strata — Design & Implementation Plan

> Handoff document for Claude Code. Self-contained: everything needed to run the design phase and then implement the system end to end.
> Product name: **Strata** (locked, L17).

---

## 0. How to use this document (instructions for Claude Code)

1. Read the whole file before doing anything.
2. Work **phase by phase** (§17). Each phase ends with a **stop gate**: summarise what was built, show how to verify it, and wait for the owner's approval before starting the next phase.
3. **Phase 0 is design only.** Write no production code in Phase 0.
4. **Open decisions (§4)** are not yours to settle. Before the phase that needs one, present the options neutrally (trade-offs only, no recommended favourite) and wait for the owner's pick. Record every pick in `docs/DECISIONS.md`.
5. **Locked decisions (§3)** are not to be revisited unless the owner raises them.
6. Keep this file as the spec. If implementation reveals a gap or contradiction, stop and ask rather than improvising a new direction; then update `docs/DECISIONS.md`.
7. Communication with the owner: concise, no filler. The owner writes in English and Arabic.

---

## 1. Product summary

A personal, AI-powered knowledge system for capturing thoughts on the fly and keeping documents and work insights. Everything is stored as **Obsidian-compatible markdown** in a vault on a VPS. A Rust (Actix) backend is the **only** way to read or write the vault. Cloud AI does the heavy lifting: filing new captures, linking notes, extracting concepts, clustering, and answering questions with citations. The vault is visualised as a live, AI-maintained **mind map / graph**.

Client:
- **One Flutter app** for Android, iOS, macOS, Windows, and Linux, with **adaptive layouts** (distinct navigation and pane structures per window size class, not a stretched phone UI). It logs in to the API as a device.
- **Offline-first on every platform**: a full local SQLite cache and an outbox. **All client logic lives in a Rust client core**; Flutter is a pure UI layer that talks to the core only through flutter_rust_bridge (frb).
- There is **no web/browser client**.

### Goals
- Capture fast (text first; dictation via Wispr Flow into any text field; no backend speech work in MVP).
- AI automatically files, links, and relates notes; relations are visible as a graph.
- **Multiple isolated user accounts**: each user has a private vault, index, AI jobs, settings, and devices. No user can see or reach another user's data.
- **People and companies are first-class objects**: each has its own page aggregating every note that mentions it, AI-maintained insights, a timeline, and its relations to other people/companies.
- **Documents and places are first-class**: where every document is (places nested inside places), who holds it or last had it, and its full custody history — kept current by AI from captures, each change cited and revertible.
- **Tasks with recurrence and reminders**, written as Obsidian Tasks checklist lines inside notes, with reminders pushed to the user's devices.
- **Duplicates are caught everywhere**: every create (note, capture, task, entity, concept, alias) is checked; the user can open the existing item or force-create.
- Markdown is the single source of truth; all derived data is rebuildable.
- Vault stays fully Obsidian-compatible in format.
- Mixed Arabic/English content works correctly everywhere.
- **Engineered to production quality**: contract-first API, compact binary wire format, and exhaustive automated tests at every layer (§16).

### Non-goals (MVP)
- Sharing, collaboration, or linking between users (accounts are fully isolated).
- A web/browser client (Flutter native apps only).
- Phone voice/file capture (deferred, §18).
- Real-time collaborative editing.

---

## 2. Core principles (non-negotiable)

1. **Markdown files are the only source of truth.** The database index, embeddings, clusters, and graph layouts are caches that can be deleted and rebuilt from the vault.
2. **All reads and writes go through the Actix API.** No client, tool, or person touches vault files directly: no SSH editing, no git clone/pull by clients, no file sync, no direct Obsidian access to the server folder. Obsidian compatibility is a *format* guarantee delivered through API export/import (§6.10).
3. **The AI never edits the user's prose.** AI output goes into frontmatter relation keys, sidecar metadata, concept notes, and AI-owned folders only.
4. **Every write is a git commit** (local repo on the VPS), prefixed `user:` or `ai:`. Any AI change is revertible via the API.
5. **Raw input is saved before processing.** A capture hits disk first; AI jobs run afterwards against the saved file. A failed AI job never loses data.
6. **The non-AI path must work alone.** Browsing, editing, search, backlinks, and graph all work if the AI provider is down or disabled.
7. **Tenant isolation.** Every request, job, query, event, and file operation is scoped to the authenticated user. No API can address another user's data; foreign IDs return `404`, never `403` (don't reveal existence).
8. **Contract first, tested always.** Every endpoint is defined in the OpenAPI contract and carried as MessagePack (L21). No feature is done until it has the tests §16 requires, passing in CI. Untested code does not merge.

---

## 3. Locked decisions

| # | Decision |
|---|----------|
| L1 | Vault lives on a VPS; backend is Rust with **Actix-web**. |
| L2 | Backend is **API only**: no static file serving, no server-rendered markdown. |
| L3 | The client is **one Flutter app** for Android, iOS, macOS, Windows, and Linux, with **adaptive layouts** per window size class (§11). There is no web/browser client. *(Replaces the Angular PWA, dropped by the owner 2026-09-27.)* |
| L4 | Each installed app logs in to the API as its own **device**, one signed-in account at a time (§12.7). |
| L5 | AI runs via **cloud APIs**, called only from the backend. Keys never reach clients. |
| L6 | All vault reads/writes via the Actix API only (principle 2). |
| L7 | Vault format is **Obsidian-compatible** (§6). |
| L8 | Relations are **AI-generated automatically** and written to the vault; the user can delete/retype them. |
| L9 | MVP is **text-first**. Voice = Wispr Flow dictation into text fields. Phone capture is deferred (§18) but its conventions (`inbox/`, `source:`) exist from day one. |
| L10 | Git history is local to the VPS; exposed to clients only through API endpoints. |
| L11 | Illustrations/icons/graph rendering use **clean, precise vector lines**. No hand-drawn, sketchy, or wobbly styles anywhere. |
| L12 | **People and companies are first-class entities** stored as Obsidian-compatible notes (§6.7), extracted and linked automatically by AI. |
| L13 | **API contract is generated with utoipa** (OpenAPI 3.1 from backend code) and is the single definition of every endpoint and payload. From it: a **generated Rust client crate** used by the client core. No handwritten request/response types or endpoint code, except the streaming transport (§7.6, D24). |
| L14 | **The client is offline-first on every platform**: the full vault text and all metadata tables are cached in a local SQLite database; every mutation goes to a local **outbox** first and syncs when online. |
| L15 | **All client logic is in a Rust client core** exposed via **flutter_rust_bridge**. Dart contains widgets, navigation, adaptive layout selection, theming, and the frb init only: no HTTP, persistence, parsing, validation, sorting/filtering, sync, or business rules. Enforced in CI (§14). |
| L16 | Markdown/frontmatter/wikilink parsing and Arabic text normalisation live in **shared Rust crates** used by both the backend and the client core, so the two never disagree on the vault format. |
| L17 | Product name is **Strata**. Use it for the app name, bundle/package IDs, binary names, and branding. |
| L18 | **Provisional (revisit before Phase 4):** LLM calls go through **`claude -p`** (Claude Code headless mode) on the VPS, on the owner's Max subscription, behind the `LlmProvider` trait (§9.1). Embeddings are local (L19). |
| L19 | Embedding model is **IBM granite-embedding-97m-multilingual-r2** (Apache 2.0, 97M params, 384-dim vectors, 32K-token context, Arabic among its enhanced languages), run locally on the VPS CPU. Runtime per D9. |
| L20 | **Multi-user with fully isolated accounts.** Each user gets their own vault (folder + git repo), index, embeddings, jobs, AI budget, settings, sessions/devices, and change log. Roles: `admin` (manages accounts) and `member`. No sharing between users. Storage per D21 / L22. |
| L22 | **Server database is PostgreSQL** (the existing instance on the VPS) with **row-level security**. One shared `strata` database; every user-owned row carries `user_id`; RLS policies enforced by the database for the application role (§5.2, §7.4). Clients keep a local SQLite cache (L14). |
| L21 | **Wire format is MessagePack everywhere.** Every request and response body, error, sync payload, and streamed event is MessagePack (`application/msgpack`), described by the OpenAPI contract. No JSON on the wire. JSON remains only where a file format requires it on disk (`.canvas`, `.meta/*.json`, the OpenAPI document itself). (§7.7) |

---

## 4. Decisions (all resolved 2026-09-27; new ones must be presented to the owner neutrally)

| # | Topic | Options | Needed by |
|---|-------|---------|-----------|
| D2 | Note editor (Flutter) | (a) Raw markdown in a Flutter text field with live styling from highlight spans computed in the Rust core; zero format drift; per-line RTL. (b) Rich-text editor package (e.g. super_editor, appflowy_editor, fleather — verify maintenance and desktop support) with a markdown round-trip; richer editing, risk of formatting drift. **Decided 2026-09-27: (b)** — rich-text editor package with a markdown round-trip; round-trip fidelity is guarded by golden tests (§16.5). | Phase 0 |
| D3 | Global graph rendering (Flutter) | (a) Custom `CustomPainter` renderer; layout (force-directed) computed in the Rust core and streamed as positions; scales furthest, most work. (b) An existing Flutter graph package (e.g. graphview — verify maintenance and performance at 10k nodes) with layout in Dart widgets; faster to build, layout logic sits in Dart (conflicts with L15 unless layout still comes from the core). (c) Hybrid: package for interaction, positions from the core. **Decided 2026-09-27: (a).** | Phase 0 |
| D4 | Local mind-map rendering (Flutter) | (a) Same renderer as D3 with a radial layout; one code path, simpler nodes. (b) Widget-based canvas (each node a Flutter widget card, edges painted beneath); richer cards and editing, heavier at scale. **Decided 2026-09-27: (b).** | Phase 0 |
| D6 | Session model | (a) Opaque DB-backed session tokens: trivial revocation, per-device list. (b) Short-lived signed access tokens + refresh tokens: stateless requests, still needs a refresh-token table. **Decided 2026-09-27: (b)** — see §8 for revocation. | Phase 1 |
| D7 | Network exposure | (a) Public HTTPS with app auth only. (b) Behind Tailscale (all clients run Tailscale). (c) Cloudflare Access (awkward for native apps: needs service tokens). **Decided 2026-09-27: (a)**; a domain will be added later. | Phase 1 |
| D9 | Embedding runtime (model is locked, L19) | (a) In-process in `stratad`: ONNX model via the `ort` crate + `tokenizers`, int8 quantized; no extra service. (b) Separate local service (e.g. Ollama or a small ONNX server) called over HTTP; verify the model is available there. **Decided 2026-09-27: (a).** | Phase 4 |
| D10 | Clustering algorithm | (a) Louvain. (b) Leiden. Both over petgraph data in Rust. **Decided 2026-09-27: (b) Leiden.** | Phase 5 |
| D11 | Flutter UI binding | Binding (logic-free either way): (a) `StreamBuilder`/`ValueListenableBuilder` directly on frb view-model streams; (b) a thin reactive package used purely for rebuilds. **Decided 2026-09-27: (b) Riverpod** (rebuilds and dependency injection only; §11.1). | Phase 3 |
| D13 | New entity creation | (a) AI auto-creates a person/company when confident; ambiguous matches become merge suggestions. (b) AI always proposes new entities as suggestions; only links to existing entities apply automatically. **Decided 2026-09-27: (b)**, plus the correction loop in §9.8. | Phase 4 |
| D14 | Client token storage | (a) Rust keychain/keystore crate called from the core (verify Android, iOS, macOS, Windows, Linux support). (b) Platform secure storage via a minimal plugin the core invokes through an frb callback (plumbing only). (c) Token encrypted in local SQLite with a key held in the platform keystore. **Decided 2026-09-27: tokens stored in the account's local database, unencrypted** (owner's choice; relies on OS app sandboxing and device security). | Phase 2 |
| D15 | Rust client generation (must emit MessagePack, L21) | (a) progenitor for types and endpoints, with its JSON body handling replaced by an `rmp-serde` codec layer (verify it can be swapped cleanly). (b) openapi-generator `rust` with custom templates for the MessagePack codec. (c) Own build-time generator: `typify` for types from the schema + a small template for endpoint functions using `rmp-serde`. Check OpenAPI 3.1 support against utoipa's output for each. **Decided 2026-09-27: (c) own generator.** | Phase 1 |
| D19 | Sync conflict resolution for note bodies | (a) 3-way merge against the base version (from git) when edits don't overlap, conflict copy otherwise. (b) Always keep server version and save the client edit as a conflict copy for manual resolution. **Decided 2026-09-27: (a).** | Phase 1 |
| D20 | Final LLM transport (replaces provisional L18) | (a) Claude Code `claude -p` on the Strata VPS (current). (b) Direct Messages API from `stratad` with an API key (lightweight, pay per token). (c) Relay service on another server that runs `claude -p` and returns JSON. All sit behind the same `LlmProvider` trait. Measure `claude -p` RAM/CPU on the VPS (`/usr/bin/time -v`) before deciding. **Decided 2026-09-27: (a) `claude -p` as default, with the Anthropic API provider also built and selectable by config.** | Phase 4 |
| D21 | Per-user storage isolation | (a) Per-user directory holding that user's vault + git + its own index database; a separate system database for users, sessions, devices. (b) Per-user vault directories + one shared database with `user_id` on every table and enforced scoping. **Decided 2026-09-27: (b) with database-enforced row-level security, on PostgreSQL (L22).** | Phase 1 |
| D22 | Account creation | (a) Admin creates accounts / sends invite links only. (b) Open self-signup with admin approval. **Decided 2026-09-27: (b).** | Phase 1 |
| D23 | AI for users other than the owner | The owner's `claude -p` subscription must serve only the owner. For other users: (a) direct API backend with the server's API key and a per-user budget; (b) each user supplies their own API key (stored encrypted, server-side only); (c) AI disabled for non-owner accounts. **Decided 2026-09-27:** for now `claude -p` serves every account (two users); the provider is chosen per user in config so the API provider can take over later without code changes. Per-user budgets still apply. | Phase 4 |
| D25 | How a deleted user receives their export | (a) In-app pickup during a grace period (restricted export-only sign-in). (b) Emailed single-use link. (c) Either, with the export encrypted by a key derived from the user's password. **Decided 2026-09-27: (a).** | Phase 1 |
| D26 | Task storage format | (a) One note per task in `tasks/`. (b) Obsidian Tasks checklist lines inside notes. (c) Both. **Decided 2026-09-27: (b)** (§6.11). | Phase 1 |
| D27 | Reminder delivery | (a) Local notifications scheduled by each device. (b) Server push (FCM / APNs / WNS; event stream on Linux). (c) Both. **Decided 2026-09-27: (b), extending to (c) later** (§12.5b). | Phase 1 |
| D28 | Tasks in navigation | (a) Replace Notes in the bottom bar. (b) "More" tab. (c) On Home only on compact. **Decided 2026-09-27: (c), plus a Tasks destination in the rail and sidebar.** | Phase 0 |
| D29 | Documents model | (a) Documents as an entity kind with free-text location. (b) Documents plus places as entity kinds. **Decided 2026-09-27: (b), with nested places, separate location / holder / last-holder, copies, and a custody history** (§6.12). | Phase 1 |
| D30 | Who updates document location/holder | (a) AI automatically from notes. (b) AI suggestion, user accepts. **Decided 2026-09-27: (a), falling back to (b) below the custody confidence threshold or on ambiguity/conflict** (§6.12). | Phase 4 |
| D24 | Streaming transport for `/events` and `/ask` (MessagePack frames) | (a) WebSocket with one binary MessagePack frame per event/token batch. (b) Long-lived HTTP response streaming length-prefixed MessagePack frames. (c) Server-Sent Events carrying base64-encoded MessagePack (keeps SSE semantics and reconnection behaviour; ~33% size overhead from base64). **Decided 2026-09-27: (a) WebSocket.** | Phase 1 |

Withdrawn with the Angular PWA (2026-09-27): D5 (nginx `/api` proxy vs subdomain — no browser client, CORS no longer applies), D16 (TypeScript client generator), D17 (Angular toolchain), D18 (Angular data layer). The old D2–D4 (web editor and web graph libraries) are replaced by the Flutter versions above.

---

## 5. Architecture

```
 ┌────────────────────────────────────────────────────┐
 │ Strata app — Flutter UI (Android · iOS · macOS ·   │
 │ Windows · Linux), adaptive layouts                 │
 │   │ frb (the only bridge)                          │
 │ Rust client core: local SQLite · outbox · sync ·   │
 │ search · graph layout · auth · view-models         │
 └───────────────────────┬────────────────────────────┘
                         │ HTTPS · MessagePack · auth
                         ▼
 ┌─────────────────────────────────────────── VPS ─┐
 │  nginx (TLS termination, reverse proxy)         │
 │                    │                            │
 │  ┌─────────────────▼──────────────────────────┐ │
 │  │ Actix API (OpenAPI contract, MessagePack)  │ │
 │  │  auth · accounts · notes · search · graph  │ │
 │  │  inbox · suggestions · ask · history ·     │ │
 │  │  sync · events (stream, D24)               │ │
 │  │  ┌──────────────┐   ┌───────────────────┐  │ │
 │  │  │ Vault store  │   │ Job runner (AI)   │──┼─┼──► Cloud LLM
 │  │  │ writer/user  │   │ fair, per user    │  │ │
 │  │  └──────┬───────┘   └───────────────────┘  │ │
 │  └─────────┼──────────────────────────────────┘ │
 │     ┌──────▼──────┐        ┌──────────────────┐ │
 │     │ users/<id>/ │        │ PostgreSQL       │ │
 │     │ vault/ (.md │        │ (existing, RLS)  │ │
 │     │ + git repo) │        │ full-text ·      │ │
 │     └─────────────┘        │ pgvector · app   │ │
 │                            └──────────────────┘ │
 └─────────────────────────────────────────────────┘
```

- The diagram shows one user's vault and index; with multiple users (L20) there is one of each **per user** (§5.2).
- **vault/** contains only Obsidian-compatible content plus a hidden `.meta/` sidecar folder. It is a git repository with no remotes that clients can reach.
- **PostgreSQL** (outside the vault) holds every user's derived index, jobs, suggestions, settings, and change log, isolated per user by row-level security, plus the global account tables. Deleting a user's derived rows and running `reindex --user` must fully rebuild them.

### 5.1 Repository layout (monorepo)

```
/crates         shared Rust crates used by backend AND client core
  vault-format/   markdown, frontmatter, wikilinks, Obsidian rules, canonical serialisation
  text-normalize/ Arabic/Latin normalisation for search and alias matching
  domain/         shared domain enums/constants (relation types, kinds) — not API DTOs
/backend        Rust workspace (Actix API, vault store, indexer, jobs, AI)
/api
  openapi.json    generated by utoipa (committed; CI checks it is up to date)
  rust-client/    generated Rust client crate (D15, MessagePack codec) + streaming module (D24)
/client
  core/           Rust crate: ALL client logic, exposed via flutter_rust_bridge
  app/            Flutter UI only (widgets, navigation, adaptive layouts, theme) + frb-generated bindings
/e2e            full-stack end-to-end suites (spawned stratad + real app via integration_test)
/design         Phase 0 outputs (tokens, flows, prototypes)
/deploy         nginx config, systemd units, backup scripts, env templates
/docs           DECISIONS.md, ARCHITECTURE.md, VAULT_FORMAT.md, WIRE_FORMAT.md, TESTING.md, RUNBOOK.md
PLAN.md         this file
```

### 5.2 Multi-user layout (L20, D21)
```
/srv/strata/
  users/<user_id>/
    vault/                  that user's Obsidian-compatible vault + .git
PostgreSQL database `strata` (existing server instance)
  global tables             users, invites, audit_log (no RLS; reachable only by the account-management role)
  user-owned tables         everything else, each with user_id + RLS
```
- **Database roles:** `strata_owner` owns the schema and runs migrations only. `strata_app` is used for all request and job work: not a superuser, `NOBYPASSRLS`, not the owner of any table. `strata_accounts` is used only by account endpoints (login, signup, admin user management) and can reach the global tables but has **no grants on user-owned tables**, so admins cannot read vault data even through a bug.
- **RLS on every user-owned table:** `ENABLE` + `FORCE ROW LEVEL SECURITY`, one policy `USING (user_id = strata_current_user()) WITH CHECK (user_id = strata_current_user())`, where `strata_current_user()` reads the transaction-local setting `strata.user_id` and returns NULL when unset — so a query with no scope sees **zero rows**, never all rows.
- **`UserScope` = a transaction.** The middleware resolves the user once; every data-layer call runs inside a transaction that first executes `SELECT set_config('strata.user_id', $1, true)` (transaction-local, so pooled connections can't leak a scope). Data-layer functions take `UserScope`, never a raw user ID. Job workers open the same kind of scoped transaction per job.
- A CI test enumerates every table in the schema and fails if a user-owned table lacks `user_id`, RLS enabled + forced, or the standard policy.
- Every request resolves `user_id` from the session **once**, in auth middleware, and passes a `UserScope` handle down; data-layer functions take `UserScope`, never a raw user ID from the request.
- One vault-store writer actor per user; users never block each other's writes.
- The job runner schedules **fairly across users** (round-robin per-user queues), keeping global concurrency within the VPS limits (§9.1, §9.1b).
- Deleting an account (D25 = a): the admin schedules it; the account becomes `deletion_pending` for a grace period (config, default 14 days) and all its sessions are revoked. The user may still sign in, but only into an **export-only session**: `GET /me/export` (read-only vault zip, produced under the user's own scope), `POST /me/confirm-deletion`, `GET /me`, and logout; every other endpoint returns `403 account_deletion_pending`. When the period ends or the user confirms, a job purges the user directory and all of the user's rows in one transaction and writes one audit-log entry. The admin can cancel before then; the admin never receives or sees the export.

---

## 6. Vault format (Obsidian compatibility spec)

Write `docs/VAULT_FORMAT.md` from this section and keep it authoritative. Every rule here must be covered by tests (§16).

### 6.1 Folder conventions

```
vault/
  inbox/                 raw captures awaiting AI filing
  notes/                 user notes (free-form subfolders allowed)
  concepts/              AI-created concept notes (ideas, topics)
  people/                person entities (one note per person)
  companies/             company/organisation entities (one note per company)
  documents/             document entities: contracts, IDs, licences, deeds (one note per document or copy) (§6.12)
  places/                place entities: offices, rooms, safes, drawers, people's homes (nestable) (§6.12)
  attachments/YYYY/MM/   raw files (deferred capture, uploads)
  tasks/                 default home for tasks not attached to another note (`tasks/Tasks.md`); tasks may live in any note (§6.11)
  maps/                  saved mind-map layouts (.canvas, JSON Canvas)
  _ai/digests/           AI-written synthesis notes (weekly digests etc.)
  .meta/                 sidecar metadata (hidden; Obsidian ignores dot-folders)
  .trash/                soft-deleted notes (git-tracked)
```

The user may create any other folders; the system must not assume notes live only in `notes/`.

### 6.2 File names
- `.md` files; Unicode (Arabic) allowed.
- Forbidden characters (Obsidian-unsafe): `* " \ / < > : | ? # ^ [ ]`. The API rejects or sanitises them.
- File name (without extension) = note title for wikilink resolution.

### 6.3 Links and embeds (Obsidian syntax only)
- Links: `[[Note]]`, `[[Note|alias]]`, `[[Note#Heading]]`, `[[Note#^blockid]]`.
- Embeds: `![[Note]]`, `![[image.png]]`, `![[file.pdf]]`.
- Block IDs: `^[a-z0-9-]+` at the end of a block; the backend may **append** block IDs to blocks it needs to cite (this is the only permitted automated body edit, and it doesn't change the prose).
- Resolution follows Obsidian's rule: shortest unique path; ambiguous names resolve by path.
- Tags: inline `#tag` and frontmatter `tags:`.

### 6.4 Frontmatter schema

YAML, Obsidian "Properties"-compatible (flat lists of strings/links only, so Obsidian's graph and backlinks see relations).

```yaml
---
id: 01J8ZK3M4X7Q...        # ULID, assigned by backend on create, never changes
title: Pricing experiments  # optional display title; file name remains canonical
aliases: [pricing tests]
tags: [pricing, pos]
created: 2026-09-27T14:32:00+03:00
updated: 2026-09-27T15:10:00+03:00
source: "[[attachments/2026/09/01J8ZK....m4a]]"   # only for captures from raw files
lang: ar                    # dominant language detected by AI (ar | en | mixed)
# Relation keys — flat lists of wikilinks (Obsidian graph-visible)
related:     ["[[Churn notes]]"]
part-of:     ["[[Subscription tiers]]"]
supports:    []
contradicts: ["[[Discount policy]]"]
follows-up:  []
duplicates:  []
concepts:    ["[[Loyalty]]", "[[Pricing]]"]
people:      ["[[Ahmed Samir]]"]          # people this note is about or mentions meaningfully
companies:   ["[[Acme Logistics]]"]       # companies this note is about or mentions meaningfully
---
```

Rules:
- Relation keys are fixed: `related`, `part-of`, `supports`, `contradicts`, `follows-up`, `duplicates`, `concepts`, `people`, `companies`. Empty keys may be omitted.
- Relations are stored **on the source note only** (directed). Reverse edges are derived in the index.
- **Unknown keys are preserved** exactly (users/Obsidian plugins may add properties).
- Backend uses a canonical key order when it writes: `id, kind, title, aliases, tags, created, updated, source, lang`, entity fields (§6.7), relation keys in the order above, then unknown keys in their original order.
- The user may add relation entries by hand (via the API/editor); entries without sidecar provenance are treated as `by: user`.

### 6.5 Sidecar metadata (`.meta/`)

Per-note JSON for AI provenance that doesn't fit flat Obsidian properties. Lives in the vault (so it's rebuild-safe and git-tracked) but is hidden from Obsidian and from the API tree listing.

`vault/.meta/notes/<id>.json`:
```json
{
  "id": "01J8ZK3M4X7Q...",
  "summary": "One-paragraph AI summary used for linking and graph hover.",
  "relations": [
    {
      "type": "contradicts",
      "target_id": "01J8ZK9...",
      "by": "ai",
      "confidence": 0.72,
      "reason": "States a flat 10% discount, while target caps discounts at 5%.",
      "model": "<provider/model>",
      "created": "2026-09-27T15:12:00+03:00"
    }
  ],
  "rejected": [
    { "type": "related", "target_id": "01J8ZKB...", "at": "2026-09-27T16:00:00+03:00" }
  ],
  "content_hash": "sha256:...",
  "last_linked_hash": "sha256:..."
}
```

- `rejected` edges must never be re-added by AI for that (source, target, type) triple. Rejecting a type also blocks re-adding the same pair under `related`.
- Sidecar references targets by **ID**; frontmatter uses wikilinks. The backend keeps them consistent on every write and rename.

`vault/.meta/clusters.json`: latest cluster assignment and AI cluster names (derived but persisted so names are stable across rebuilds).

### 6.6 Concept notes
- Path `concepts/<Concept Name>.md`, frontmatter includes `kind: concept`.
- Body: AI-maintained short definition section under a `## Summary` heading, followed by free space the user may write in. The AI rewrites only the `## Summary` section and never touches user text below it.
- Notes point to concepts via the `concepts:` key.

### 6.7 People and companies (first-class entities)

Entities are ordinary Obsidian notes with a `kind` and structured properties, so they appear in Obsidian's graph and can be queried by Dataview-style plugins after export.

**Person** — `people/<Full Name>.md`
```yaml
---
id: 01J...
kind: person
aliases: [أحمد سمير, Ahmed S., A. Samir]   # every spelling/script the person appears under
tags: [client]
role: Operations manager                     # free text, user-editable
companies: ["[[Acme Logistics]]"]            # current/primary affiliations
phone: ""                                    # optional contact fields, user-entered only
email: ""
---
```

**Company** — `companies/<Company Name>.md`
```yaml
---
id: 01J...
kind: company
aliases: [أكمي, Acme]
tags: [supplier]
industry: Logistics
website: ""
---
```

**Entity-to-entity relation keys** (frontmatter, flat wikilink lists, provenance in `.meta/` like other relations):
- Person: `works-at`, `worked-at`, `reports-to`, `knows`, `introduced-by`.
- Company: `client-of`, `supplier-of`, `partner-of`, `competitor-of`, `subsidiary-of`.
- `people:` / `companies:` on a note mean "this note is about or meaningfully involves this entity".

**Entity note body** (sections in this order):
```markdown
## Summary        ← AI-maintained: who/what this is, 2–4 sentences
## Insights       ← AI-maintained bullets, each citing its source: "- Prefers weekly invoicing [[Call 2026-09-12#^a1b2]]"
## Open items     ← AI-maintained: unresolved asks, promises, follow-ups, each cited
## Timeline       ← AI-maintained: dated one-liners of interactions/events, each cited, newest first
## Notes          ← user-owned; AI never edits anything under this heading
```
- AI rewrites only the four AI sections; user text under `## Notes` and any user-added headings are preserved byte-for-byte.
- Every AI bullet must cite a note/block. Uncited claims are rejected by the backend validator.
- **No speculation:** Insights and Open items contain only what the cited notes actually state. A one-line capture may add a Timeline entry and nothing else.
- **Timeline dates:** use an explicit date in the note text if present; resolve relative dates ("tomorrow", "next Sunday", "بكرة") against the note's `created` timestamp; otherwise use `created`. Store the resolved date, not the relative phrase.
- The aggregated list of notes mentioning the entity is **not** written into the file (it's derived from backlinks and served by the API) to avoid churn.
- Contact fields are only ever filled by the user, never by AI.

**Entity resolution rules:**
- Matching uses name, all `aliases` (Arabic and Latin spellings), company context, and embeddings of the entity summary.
- A mention that matches one entity above threshold → linked automatically.
- A mention matching several entities, or none with enough context → handled per D13 (auto-create vs. suggestion); ambiguous merges are always suggestions.
- Merging two entities (`POST /entities/{id}/merge`) rewrites all links/relations to the survivor, unions aliases, moves the loser's `## Notes` content under a dated sub-heading, soft-deletes the loser — one commit, revertible.
- **Nicknames and kinship terms** ("baba", "mama", "بابا", "my brother", "the boss") are never auto-created as entities, regardless of D13. Unless an existing entity already has that alias, the mention becomes a suggestion: link to an existing person or create a new one.
- **Accepting an entity-link suggestion adds the exact mention text (both scripts if the user provides them) to that entity's `aliases`**, so future mentions resolve automatically.
- Rejected entity links are recorded in `.meta/` like rejected relations and never re-added.

### 6.8 Canvas / mind-map layouts
- Saved layouts use **JSON Canvas** (`.canvas`) in `maps/`, with file nodes referencing vault notes, so they open in Obsidian.
- Ephemeral layouts (auto-positioned) are not saved; only layouts the user pins/saves are written.

### 6.9 Inbox and capture conventions (present from day one)
- A capture creates `inbox/YYYY-MM-DD-HHmmss.md` with frontmatter `id`, `created`, and (for raw-file captures, deferred) `source:`.
- AI filing proposes title, tags, destination folder, relations, and concepts (§9.3).

### 6.10 Obsidian round-trip via API
- `GET /api/export` → zip of the vault (including `.meta/`, excluding `.git/`), plus a minimal `.obsidian/app.json` setting the attachment folder, so it opens cleanly in Obsidian.
- `POST /api/import` → accepts a zip of an Obsidian vault; assigns IDs to notes lacking them, preserves unknown properties, indexes, and queues linking jobs. Import is a single git commit and is revertible.
- There is **no** other path between Obsidian and the server vault.

---

### 6.12 Documents and places (D29 = b)

Physical and digital documents, and the places they're kept, are entities like people and companies: ordinary notes with a `kind` and structured properties.

**Place** — `places/<Name>.md`
```yaml
---
id: 01J...
kind: place
aliases: [مكتب مدينة نصر, Nasr City]
part-of: ["[[Nasr City office]]"]   # nesting: safe → office → (optionally) city/building
address: ""                          # user-entered only
---
```
e.g. `places/Nasr City office.md`, and `places/Safe — Nasr City office.md` with `part-of: ["[[Nasr City office]]"]`.

**Document** — `documents/<Name>.md`
```yaml
---
id: 01J...
kind: document
aliases: [عقد وطنية]
doc-type: contract                  # contract | id | licence | deed | invoice | certificate | other (free text allowed)
copy: original                      # original | certified copy | copy | digital
companies: ["[[Watanya]]"]          # whose document it is / who it concerns
people: []
location: "[[Safe — Nasr City office]]"   # where it is now (a place), or empty if unknown
holder: ""                          # person who has it right now, if anyone (can be set together with location: "with Shady at the office")
last-holder: "[[Shady]]"            # derived from custody history; kept in frontmatter so Obsidian can query it
expires: 2027-03-31                 # optional
status: stored                      # stored | checked-out | with-third-party | lost | destroyed
---
```

**Body** (AI-maintained sections, same citation rules as §6.7):
```markdown
## Summary
## Custody            ← dated events, newest first, each cited: "2026-09-20 — returned to Safe — Nasr City office by [[Shady]] [[Capture 2026-09-20#^c1d2]]"
## Notes              ← user-owned
```
- Custody event types: `stored-at`, `moved-to`, `handed-to`, `returned-by`, `sent-to` (third party), `received-from`, `lost`, `found`, `destroyed`. The frontmatter (`location`, `holder`, `last-holder`, `status`) is always the result of the newest event; the index recomputes it and the backend keeps them consistent.
- Several copies of the same document are separate notes linked with `copy-of: ["[[Watanya contract]]"]`, so "the original is in the safe, a copy is with the accountant" works.
- **Queries this must answer** (Ask and the directory): where is X; what's in a place **including everything in places nested inside it** ("what's at the Nasr City office?"); what does a person hold now; what did a person last handle; which documents expire soon; the custody history of X.
- Dates in custody events follow the §6.7 timeline-date rules (explicit date, else relative date resolved against the note's `created`, else `created`).
- No speculation: a custody event is only recorded when a note states it.

**AI updates (D30 = a, falling back to b):** captures like "Watanya's contract is at the Nasr City office in the safe, last with Shady" or "gave the Watanya contract to Shady" are resolved to a document, places, and people, and produce a custody event.
- Confidence **≥ custody threshold** (setting, default 0.85, stricter than relations) and every referenced entity resolved unambiguously → applied automatically: custody line + frontmatter update in one `ai:` commit, revertible, shown in the activity feed.
- Confidence below the threshold, an ambiguous or unknown document/place/person, or a statement that conflicts with a newer recorded event → **suggestion** in the inbox (accept / edit / reject). Unknown places and documents are proposed as new entities (nickname rules from §6.7 apply to people).
- Location and holder fields are never guessed from context; they change only through custody events.

### 6.11 Tasks (D26 = b: Obsidian Tasks checklist lines)

Tasks are checklist lines inside any note, in the syntax of the Obsidian **Tasks** plugin, so they keep working in Obsidian after export. Tasks created without a home note (e.g. accepted from a capture) go to `tasks/Tasks.md`, under a heading per month of creation.

```markdown
- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2
- [ ] Petrol Arrows invoice 🔁 every week on Sunday 📅 2026-09-27 (@2026-09-27 10:00) [[Petrol Arrows]] ^t-01j9a3
- [x] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-09-01 ✅ 2026-09-01 ^t-01j8z7
```
- Recognised signifiers (Tasks plugin): `📅` due, `⏳` scheduled, `🛫` start, `🔁` recurrence (the plugin's "every …" language), `✅` done date, `❌` cancelled, `➕` created, priority `🔺⏫🔼🔽⏬`. Reminders use the Obsidian **Reminder** plugin form `(@YYYY-MM-DD HH:mm)`; several are allowed.
- **Identity:** every task line carries a block ID `^t-<ulid>` (the backend may append block IDs, §6.3), so tasks survive edits, moves, and renames.
- **Parsing** lives in `/crates/vault-format` (shared with the client core). The recurrence phrase is kept verbatim in the file and compiled to an RRULE (RFC 5545) for scheduling. A phrase outside the supported grammar is preserved untouched and the task is flagged "recurrence not understood" — never rewritten.
- **Completing a recurring task** follows the Tasks plugin: the current line gets `[x]` and `✅ <date>`, and a new line for the next occurrence (new block ID, due date advanced by the rule, reminders shifted by the same offset) is inserted directly above it. Completed lines are the task's history. This is a user-initiated edit, one `user:` commit.
- Overdue occurrences stay open until completed or cancelled; the next occurrence is only created on completion (Tasks plugin semantics), so nothing is silently skipped.
- **AI** proposes tasks from captures ("remind me to make Watanya's invoice every 1st") as suggestions with title, due, recurrence, reminders, and entity links; the line is only written when the user accepts. Tasks can always be created and edited by hand without AI (principle 6).
- The user's timezone (settings) anchors due dates, recurrence, and reminder times.

## 7. Backend (Rust / Actix)

### 7.1 Crate layout (Cargo workspace)
```
backend/
  crates/
    api/        Actix app: routes, auth middleware (UserScope), MessagePack extractor/responder, streaming, OpenAPI (utoipa)
    vault/      file store: atomic writes, git, link rewriting (parsing via /crates/vault-format)
    index/      PostgreSQL schema, migrations, RLS policies, full-text, pgvector, graph queries, reindex
    jobs/       job queue, fair per-user runner, scheduling (debounce, nightly)
    ai/         provider traits, prompts, structured-output parsing, budget
    graph/      graph assembly, clustering (petgraph)
    common/     ids (ULID), errors (problem details), config, clock (injectable), time
    testkit/    test-only: temp vault/user builders, fake clock, fake LLM provider, fixture vaults, spawned server
  bin/stratad    main binary; subcommands: serve, reindex [--user], verify [--user], create-user [--admin], openapi (writes /api/openapi.json)
```

Suggested crates (verify current maintenance before adopting): `actix-web`, `tokio`, `sqlx` (PostgreSQL), `pgvector`, `comrak` or `pulldown-cmark` (parse; custom wikilink extension), a maintained YAML crate, `gix` or `git2`, `argon2`, `ulid`, `utoipa`, `rmp-serde`, `petgraph`, `reqwest`, `serde`, `tracing`. Test tooling: `proptest`, `insta` (snapshots), `cargo-fuzz`, `cargo-llvm-cov`, `cargo-mutants`, `criterion`, `jsonschema` (contract validation).

### 7.2 Vault store (write path)
- **Single writer**: all writes go through one async actor/queue per vault, so no two writes race. Reads are concurrent.
- **Optimistic concurrency**: every note has a `version` (content hash). Updates must send `If-Match`; mismatch → `409` with the current version.
- **Atomic writes**: write temp file → fsync → rename.
- **Commit** after each logical operation with message `user: <op> <path>` or `ai: <job-kind> <path>`. Batch AI writes from one job into one commit.
- **Rename/move**: rewrite all wikilinks and relation entries pointing at the note across the vault in the same commit.
- **Delete**: move to `.trash/` (soft); hard delete only via an explicit purge endpoint.
- After each write: update index synchronously (text, links, tags, blocks) and enqueue AI jobs (embedding, linking) asynchronously.

### 7.3 Startup reconciliation & integrity
Since only the API writes, out-of-band changes indicate tampering or a crash. On startup (and via `stratad verify`):
- Compare every file's hash with the index; reindex any mismatch; log it as an integrity warning surfaced in the UI.
- Detect uncommitted changes in git; commit them as `system: recovered changes` and warn.
- Validate sidecar ↔ frontmatter consistency; repair from frontmatter (frontmatter wins for existence of an edge; sidecar supplies provenance).

### 7.4 PostgreSQL schema (derived tables marked *)

Every table below except the global block has `user_id` as the first column of its primary key and the RLS policy from §5.2. IDs are ULIDs stored as `uuid`.
```
notes*        (id PK, path UNIQUE, title, kind, lang, created, updated, content_hash, word_count)
aliases*      (note_id, alias)
tags*         (note_id, tag)
links*        (src_id, dst_id NULL, dst_raw, kind[link|embed], anchor, block_id)   -- body wikilinks
relations*    (src_id, dst_id, type, by[user|ai], confidence, reason, created)
rejected*     (src_id, dst_id, type, at)
blocks*       (note_id, block_id, heading_path, text, start_offset, end_offset)
notes.search* tsvector column (`simple` config) built from title/body/tags **after** Arabic/Latin normalisation in Rust (`/crates/text-normalize`), GIN index; `pg_trgm` GIN index on normalised aliases for fuzzy entity matching
chunks*       (id, note_id, block_id, text, token_count)
chunks.embedding* vector(384) (pgvector), HNSW index; model id stored per row
entities*     (note_id PK, kind[person|company], display_name, role, industry)
entity_aliases* (note_id, alias, alias_normalized)   -- normalized: Arabic normalisation + lowercase + transliteration key
mentions*     (entity_id, note_id, block_id, first_seen, last_seen)   -- from people:/companies: keys + AI mention spans
clusters*     (note_id, cluster_id), cluster_names* (cluster_id, name)
places*       (note_id PK, parent_id NULL)       -- part-of nesting; queries use recursive CTEs
documents*    (note_id PK, doc_type, copy, copy_of NULL, location_id NULL, holder_id NULL, last_holder_id NULL, status, expires NULL)
custody_events* (id, document_id, type, at, place_id NULL, person_id NULL, counterparty_id NULL, by[user|ai], confidence, source_note_id, source_block_id)
-- global tables (no user_id; strata_accounts role only)
users         (id, username, display_name, password_hash, role[admin|member], status[pending|active|disabled|rejected|deletion_pending], created, approved_by, approved_at, deletion_at, export_downloaded_at)
invites       (id, token_hash, role, created_by, expires, used_at)
audit_log     (id, actor_id, action, target, at)
-- per-user app state (not derived; user_id + RLS)
devices       (id, user_id, name, platform, created, last_seen, push_provider[fcm|apns|wns|none], push_token)
sessions      (id/token_hash, device_id, expires, revoked)
jobs          (id, kind, note_id, payload, status, attempts, run_after, last_error, created, updated)
suggestions   (id, note_id, kind, payload, status[pending|accepted|rejected], created)   -- payload stored as MessagePack blob
ai_usage      (day, provider, model, input_tokens, output_tokens, est_cost)
settings      (key, value)
change_log    (seq INTEGER PK AUTOINCREMENT, epoch, entity_type, entity_id, op[upsert|delete], version, at)
              -- app state; every committed change appends a row. A full rebuild that can't preserve seq bumps `epoch`, forcing clients to re-bootstrap.
idempotency   (op_id PK, device_id, result, created)   -- result stored as MessagePack; replayed pushes return it
tasks*        (id = block id, note_id, text, status[open|done|cancelled], due, scheduled, start, recurrence_raw, rrule, priority, done_at, line_start, line_end)
task_reminders* (task_id, remind_at)              -- derived from the (@…) markers
notification_log (id, task_id, remind_at, device_id, provider, sent_at, result)   -- idempotent delivery: one send per (task, remind_at, device)
dedupe_keys*  (kind, item_id, exact_key, trigram_text)   -- normalised keys for exact/near duplicate lookup (§9.7)
dedupe_keep_both (kind, a_id, b_id, at)           -- user chose "create anyway"; that pair is never flagged again (also mirrored in .meta/ so it survives rebuilds)
```

Arabic search: normalise alef/ya/ta-marbuta variants and strip tashkeel at index and query time.

Schema changes go through versioned, forward-only migrations, each with a test that migrates a fixture database from the previous version and asserts the resulting schema and data.

### 7.5 API (all under `/api/v1`, MessagePack, auth required except login/refresh)

**Auth & devices**
- `POST /auth/signup` {username, password, display_name} → account in `pending` state (D22 = b); no session until approved
- `POST /auth/login` {username, password, device_name, platform} → session (`403 account_pending` / `account_disabled` problem types before approval or after disabling)
- `POST /auth/refresh`, `POST /auth/logout`
- `GET /devices`, `DELETE /devices/{id}`

**Account & admin**
- `GET /me` — current user, role, settings
- `PATCH /me` — change password, UI language, preferences
- `GET /admin/users?status=pending|active|disabled`, `POST /admin/users` (admin-created account), `POST /admin/users/{id}/approve`, `POST /admin/users/{id}/reject`, `PATCH /admin/users/{id}` (disable/enable, role, reset password), `DELETE /admin/users/{id}` (schedules deletion, D25), `POST /admin/users/{id}/cancel-deletion` — admin only. Admins see deletion status and date and whether the export was downloaded, never the export itself.
- `GET /me/export` — the caller's own vault as a zip; the only data endpoint available to a `deletion_pending` account. `POST /me/confirm-deletion` — the user ends the grace period early.
- All other endpoints are implicitly scoped to the session's user (principle 7).

**Notes**
- `GET /tree` — folders + notes (excludes `.meta`, `.trash`, `.git`)
- `GET /notes/{id}` — content, parsed frontmatter, version
- `GET /notes/by-path?path=` — lookup
- `POST /notes` {path, content} → created note (ID assigned)
- `PUT /notes/{id}` {content} + `If-Match` → updated
- `POST /notes/{id}/move` {new_path} → rewrites inbound links
- `DELETE /notes/{id}` → soft delete; `POST /trash/{id}/restore`; `DELETE /trash/{id}` purge
- `GET /notes/{id}/backlinks` — body links + relations in, grouped by type
- `GET /notes/{id}/history` — git log for the file
- `GET /notes/{id}/history/{commit}` — file at revision
- `POST /notes/{id}/revert` {commit}
- `POST /commits/{commit}/revert` — revert a whole AI commit

**Search**
- `GET /search?q=&mode=keyword|semantic|hybrid&limit=`

**Inbox & capture**
- `POST /capture` {text} → creates inbox note, enqueues filing
- `GET /inbox` — inbox notes with their pending suggestions

**Suggestions**
- `GET /suggestions?status=pending`
- `POST /suggestions/{id}/accept` (optional edits), `POST /suggestions/{id}/reject`

**Relations**
- `POST /relations` {src_id, dst_id, type} — manual edge (by: user)
- `PATCH /relations` {src_id, dst_id, type, new_type} — retype
- `DELETE /relations` {src_id, dst_id, type} — removes and records in `rejected` if by: ai

**People & companies**
- `GET /entities?kind=person|company&q=&tag=` — list/search (matches aliases in both scripts)
- `POST /entities` {kind, name, aliases?, fields?} — manual create
- `GET /entities/{id}` — aggregated view: properties, AI sections, mentioning notes (paged, newest first), entity relations, open items, timeline
- `PATCH /entities/{id}` — edit properties/aliases (user fields only)
- `POST /entities/{id}/merge` {into_id}
- `GET /entities/{id}/notes` — notes mentioning the entity, with the mention snippet
- `POST /entities/{id}/refresh` — force insights refresh

**Graph**
- `GET /graph?types=&include_similarity=` — nodes, edges, clusters (no positions)
- `GET /graph/local/{id}?depth=1..3&types=` — neighbourhood
- `GET /maps`, `GET /maps/{id}`, `PUT /maps/{id}` — JSON Canvas layouts (the `.canvas` file is JSON on disk; carried inside a MessagePack envelope on the wire)

**AI**
- `POST /ask` {question, scope?} → stream (D24): token frames, then citation frames `[[Note#^block]]`
- `POST /notes/{id}/relink` — force linking job
- `GET /ai/status` — provider health, queue depth, today's usage vs budget

**Sync (client offline-first)**
- `GET /sync/bootstrap?cursor=` — paged full snapshot of notes (content), entities, relations, tags, links, inbox, suggestions, cluster assignments; returns `epoch` and a starting `seq`
- `GET /sync/changes?since=<seq>&epoch=&limit=` — changed records with full payloads, tombstones for deletes, next `seq`; `410` if the epoch changed (client re-bootstraps)
- `POST /sync/push` {ops[]} — each op: `{op_id (ULID, idempotency key), kind, entity_id, base_version, payload}`; kinds mirror the granular mutations (note.create/update/move/delete, capture, relation.add/remove/retype, suggestion.accept/reject, entity.create/patch/merge, relink.request). Per-op result: `applied{new_version}` | `conflict{server_version, resolution}` | `duplicate{candidates}` (create ops without `force`) | `rejected{problem}`. Task ops: task.create/update/complete/cancel/reopen/delete. Ops apply in order; results are stored under `op_id` so replays are safe.
- Creates accept **client-generated ULIDs** so offline-created notes keep their IDs.

**Documents & places**
- `GET /documents?q=&place=&holder=&status=&expiring_before=` — `place` includes nested places
- `GET /documents/{id}` — properties, custody history (cited), copies
- `POST /documents`, `PATCH /documents/{id}` (user fields), `POST /documents/{id}/custody` {type, at, place_id?, person_id?} — manual custody event
- `GET /places`, `GET /places/{id}` — nested places and every document inside (recursively), `POST /places`, `PATCH /places/{id}`
- `GET /entities/{id}/documents` — documents a person holds now / last handled, or a company's documents
- All create endpoints run the duplicate check (§9.7); a custody-event suggestion is a normal suggestion (§7.5 Suggestions).

**Tasks & reminders**
- `GET /tasks?view=today|upcoming|overdue|recurring|done&entity=&note=` — task list (parsed from checklist lines)
- `POST /tasks` {text, due?, recurrence?, reminders?, note_id? (default `tasks/Tasks.md`), force?} → task (duplicate check §9.7)
- `PATCH /tasks/{id}` {text?, due?, recurrence?, reminders?, priority?} + `If-Match`
- `POST /tasks/{id}/complete`, `POST /tasks/{id}/cancel`, `POST /tasks/{id}/reopen`
- `PUT /devices/{id}/push` {provider, token} — register this device for reminders (D27)

**Duplicates (all create endpoints)**
- Every create (`POST /notes`, `/capture`, `/tasks`, `/entities`, entity alias edits, concept creation) runs the duplicate check (§9.7). A likely duplicate returns `409` with problem type `duplicate_candidates` and the candidates (id, kind, title, snippet, match level, score). Resending with `force: true` creates it anyway and records "keep both" for each listed candidate. `POST /capture` never refuses: the capture is saved and the duplicate flag rides on its inbox suggestion (principle 5).

**Vault ops**
- `GET /export`, `POST /import` (zip bodies, `application/zip`)
- `GET /integrity` — warnings from reconciliation

**Events**
- `GET /events` (stream, D24): `note.created|updated|moved|deleted`, `relation.added|removed`, `suggestion.created`, `job.completed|failed`, `cluster.updated`, `entity.created|updated|merged`, `integrity.warning`, `account.disabled`. Every event carries IDs and the new version; clients refetch as needed.

Errors: RFC 7807 problem details encoded as MessagePack (`application/problem+msgpack`). `409` for version conflicts, `422` for invalid names/format, `404` for anything outside the caller's scope.

### 7.6 API contract & generated client
- utoipa annotations on every handler and DTO; `stratad openapi` writes `/api/openapi.json`. The contract declares `application/msgpack` as the only request/response media type for every operation (plus `application/zip` for export/import).
- From that file: the **Rust client crate** (`/api/rust-client`, D15) with a MessagePack codec. Regenerated by one script (`api/generate.sh`); CI fails if generated output differs from what's committed.
- The client core depends only on the generated Rust client for HTTP. There is no Dart API client: Dart never talks to the network.
- **Streaming exception:** OpenAPI can't describe streams well. `/events` and `/ask` use the transport chosen in D24, implemented in a small hand-written `streaming` module in the client crate, reusing the generated DTO types for every frame payload. Frame schemas are still listed in the contract (as named component schemas) so they're versioned and validated like everything else.
- **Contract conformance is tested, not assumed:** every endpoint's integration tests decode the MessagePack response and validate it against the OpenAPI schema (§16); a schema-driven fuzzer exercises every operation with generated inputs.
- **Versioning:** additive changes only within `/api/v1`; a breaking change needs a new version and a migration note in `docs/DECISIONS.md`. CI diffs the committed contract against the previous release and fails on unannounced breaking changes.

### 7.7 Wire format (MessagePack, L21)
- Serialisation via `rmp-serde` on both sides, structs encoded **as maps with field names** (not positional arrays) so optional fields can be added without breaking old clients; unknown fields are ignored on decode.
- Enums are tagged (`{"type": ..., ...}` in schema terms) and documented as `oneOf` with a discriminator in the contract.
- Timestamps are RFC 3339 strings (`format: date-time`); IDs are ULID strings; raw bytes use MessagePack `bin`.
- Request bodies are size-limited per route; decoding rejects trailing bytes, excessive nesting, and oversized strings/arrays with `422`.
- `docs/WIRE_FORMAT.md` documents the conventions with examples (hex dumps) and is covered by golden tests.
- Media type `application/msgpack` (verify the currently registered media type at implementation time and use it consistently).

---

## 8. Auth & security

- Multiple isolated users (L20). The first admin is created with `stratad create-user --admin`; further accounts self-register and wait in `pending` until an admin approves them (D22 = b). A pending or rejected account has no vault, no sessions, and no API access beyond login's error. The user directory is created on approval. Roles: `admin`, `member`. Admins manage accounts but **cannot read other users' vaults** through the API.
- Disabling a user revokes all their sessions immediately.
- Passwords hashed with Argon2id. Login rate-limited per IP and per username; capture/ask rate limits are per user.
- Every login creates a **device** row; sessions belong to devices and are revocable individually.
- Tokens (D6 = b): short-lived access tokens (15 min, EdDSA-signed, carrying user id, device id, session id, role) + rotating one-time refresh tokens stored hashed in PostgreSQL with reuse detection (a replayed refresh token revokes the whole device session). Immediate revocation: every request checks the session id against an in-memory revocation set kept current from the database (disable user, sign out, device removal, deletion scheduling), so "disabling revokes immediately" holds within one request. Sent as a bearer token by the client core; stored per D14. No cookies (no browser client).
- AI provider keys live only in backend env/secret file (0600), never logged, never sent to clients.
- Request logging redacts note content by default.
- Network exposure per D7. TLS terminated by nginx.
- Upload limits and MIME checks on import/attachments; MessagePack decode limits (§7.7).

---

## 9. AI subsystem

### 9.1 Provider abstraction

**Primary backend: `ClaudeCliProvider`** (L18). The backend spawns the `claude` binary in non-interactive mode for each LLM call.
- Runs as a dedicated OS user (e.g. `strata-ai`) that is logged in to Claude Code and has **no read or write access to the vault or data directories**, with an empty scratch working directory. This keeps principle 2 intact: the model gets text in and returns text out, and only the Actix vault store writes files.
- All Claude Code tools are disabled for these calls (no file read/edit, no shell, no web). Pure prompt → JSON/text.
- `ANTHROPIC_API_KEY` must be absent from its environment, and bare mode must not be used, so billing stays on the subscription login.
- Structured output: request JSON-format output, validate it against the schema, retry on invalid output (same rules as below). Ask uses the CLI's streaming output, relayed to clients over the D24 transport as MessagePack frames. (JSON here is the internal backend↔CLI format only; it never reaches the wire to clients.)
- Subscription usage is shared with the owner's interactive Claude Code work. Treat usage-limit errors as a **pause** state (jobs wait and resume), expose it in `/ai/status`, and make daily job volume configurable so Strata can't crowd out interactive use.
- Concurrency is low by default (e.g. 1–2 concurrent processes), configurable.
- Verify exact CLI flags against the current Claude Code docs at implementation time; they change between releases.

**Optional backend: `AnthropicApiProvider`** (Messages API with an API key), same trait, selectable in config. Not required for the MVP.

```rust
trait LlmProvider { async fn complete_json(&self, req: JsonRequest) -> Result<serde_json::Value>;
                    async fn stream(&self, req: ChatRequest) -> Result<TokenStream>; }
trait Embedder    { async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>; fn dims(&self) -> usize; }
```
- Structured outputs: every classification-style call requests JSON matching a schema; the backend validates it and rejects/retries invalid output (max 2 retries).
- Prompts live in `backend/crates/ai/prompts/*.md` with versioned names; the prompt version is recorded in sidecar provenance.
- Budget guard: configurable daily token/cost cap **per user** (plus a global cap). When exceeded, that user's jobs pause (not fail) and `/ai/status` reports it.
- Provider per user per D23: the `claude -p` backend (L18) serves **only the owner's account**; other users use the backend chosen in D23.
- All prompts must state that content may be Arabic, English, or mixed, and that outputs (titles, summaries, concept names) use the note's dominant language.
- A `FakeLlmProvider` (in `testkit`) replays recorded fixture responses keyed by prompt version and input hash; all AI tests use it, never the network.

### 9.1b Embeddings on the VPS (L19)
- VPS budget: 4 GB RAM, 1 CPU core. The model is the smallest multilingual option chosen for this reason; use the int8 quantized ONNX weights.
- Embedding jobs run **one at a time, at low process priority**, and never concurrently with a `claude -p` call; API request handling always has priority.
- First import of an existing vault runs as a resumable low-priority background job with progress in `/ai/status`.
- Every vector row stores the embedding model ID. Changing models triggers a full resumable re-embed.
- Before Phase 4 sign-off, benchmark retrieval on 20–30 of the owner's real notes (Egyptian Arabic, English, mixed) and report results.

### 9.2 Jobs (queue in PostgreSQL, `FOR UPDATE SKIP LOCKED`)

| Job | Trigger | Does |
|-----|---------|------|
| `embed` | note write (content hash changed) | one note-level vector (whole note, fits the 32K context) + block/heading chunks (~300–500 tokens) for citation-level retrieval; upsert `chunks`/`chunk_vec` (384-dim) with the model ID stored per vector |
| `summarize` | after `embed` | write `summary` into sidecar |
| `link` | after `summarize`, debounced 30s after last edit | §9.4 |
| `file_inbox` | capture into `inbox/` | §9.3 |
| `concepts` | inside `link` call | extract concepts; create/update concept notes |
| `custody` | inside `link` call | extract document custody statements, resolve document/place/person, apply or suggest per §6.12 (D30) |
| `entities` | inside `link` call | extract person/company mentions, resolve (§6.7), write `people:`/`companies:` and entity-to-entity relations |
| `entity_insights` | debounced (≈5 min) after any note mentioning the entity changes; nightly sweep | regenerate Summary / Insights / Open items / Timeline from mentioning notes, with citations; skip if inputs unchanged |
| `cluster` | nightly + manual | community detection, name clusters, write `.meta/clusters.json` |
| `dedupe` | nightly | semantic sweep across all kinds (§9.7); propose `duplicates` merges as suggestions (never auto-merge), skipping keep-both pairs |
| `reminders` | every minute | send due reminders via push (D27), idempotent per (task, time, device), in the user's timezone |
| `digest` | weekly (configurable) | write `_ai/digests/YYYY-Www.md` summarising new notes, open questions, contradictions |

- Idempotency: skip `link` if `content_hash == last_linked_hash`.
- Retries with exponential backoff; failures visible via `/ai/status` and events.
- Concurrency limit on provider calls (configurable).

### 9.3 Inbox filing
Input: the inbox note text, and a compact list of existing folders, top tags, and candidate notes/concepts (from embeddings + FTS).
Output JSON: `{title, tags[], destination_folder, relations[{target_id,type,confidence,reason}], concepts[{name, existing_id?}], people[{name, existing_id?, confidence}], companies[{name, existing_id?, confidence}]}`.
- Relations and concepts at or above the threshold are applied automatically (same as §9.4).
- **Title/move/tags are applied automatically** only if the owner enables "auto-file" in settings; otherwise they become a `suggestion`. Default: suggestion. (Owner may switch in settings; both modes must be implemented.)

### 9.4 Automatic linking (core of L8)
1. Candidates: top-k (≈15–20) by vector similarity over note summaries/chunks ∪ FTS keyword hits, excluding `rejected` pairs and the note itself.
2. LLM call with the note (full or truncated) and candidate summaries. Output: relations with `type ∈ {related, part-of, supports, contradicts, follows-up, duplicates}`, `confidence 0..1`, one-line `reason`; plus concepts.
3. Apply: relations with `confidence ≥ threshold` (setting, default 0.7) are written to frontmatter + sidecar. `duplicates` is never auto-applied; it becomes a suggestion.
4. AI-owned edges from a previous run that are no longer returned are removed **only if** they were `by: ai` (never remove user edges).
5. Entities: people/company mentions resolved per §6.7 and written in the same job; new entities per D13.
6. Single `ai:` commit per job; emit `relation.added/removed` and `entity.updated` events.

### 9.5 Ask (RAG)
- Hybrid retrieval (vector + FTS) → rerank by LLM or score fusion → top chunks with block IDs.
- Ensure cited blocks have block IDs (append `^id` if missing; one `ai:` commit).
- Stream answer over the D24 transport as MessagePack frames; citations as `[[Note#^block]]` resolved to note IDs for client navigation.
- Answer is not saved unless the user requests "save as note" (creates a note in `notes/` with citations as links).

### 9.6 Similarity edges
Computed on request (not stored) from note-level embeddings: top-n neighbours above a similarity floor. Returned only when `include_similarity=true`.

---

### 9.7 Duplicate detection (all kinds)

One engine checks every kind of item: notes, captures, tasks, people, companies, concepts, and aliases.

| Level | How | Needs AI |
|---|---|---|
| Exact | normalised key equal (`/crates/text-normalize`: Arabic letter variants, tashkeel, case, spacing, punctuation; for tasks also the recurrence rule and entity links) | no |
| Near | trigram similarity on normalised text (`pg_trgm`) above a per-kind threshold | no |
| Semantic | embedding cosine above a per-kind threshold; borderline scores confirmed by one LLM call | yes |

- **Synchronous check on create** (exact + near, plus semantic if an embedding for the new text is cheap to compute) → `409 duplicate_candidates` unless `force`. With AI down, exact and near still run (principle 6).
- **Captures are never blocked** (principle 5): saved first, then the duplicate flag ("Already exists: Make Watanya's ETA invoice · monthly · next Thu 1 Oct") appears on the inbox item with Open existing / Create anyway / Discard.
- **Offline:** the client core runs exact + near checks against its local cache before queuing a create and shows the same prompt; the server re-checks on push and may return `duplicate{candidates}`, which the app surfaces without losing the item.
- **Keep both** is remembered per pair (`dedupe_keep_both` + `.meta/`), so the same pair is never flagged again by the create check or the nightly sweep.
- Thresholds are per-kind settings with tested defaults.

### 9.8 Correcting the AI (D13 loop)
- Every AI decision (link, entity mention, relation, custody event, task suggestion, filing) has an ID and appears in the activity feed with its source.
- **Repoint in place:** any AI link can be repointed ("this Ahmed is Ahmed Fathy"), retyped, or rejected from the note, entity page, inbox, or activity feed.
- **Correct in words:** a capture or an Ask message like "the Ahmed in yesterday's Acme call is Ahmed Fathy" is recognised as a correction. The model receives the user's recent AI decisions (last ~50, with IDs, sources, and targets) and the candidate entities, resolves the reference to a specific decision, and proposes the fix as a suggestion (applied automatically only when confidence ≥ the relation threshold and the reference is unambiguous).
- **Threaded suggestions:** the user can reply to a suggestion ("no, the Petrol Arrows one"); the AI re-proposes with the reply in context.
- **Memory:** a correction records the rejected link (never re-added, §6.5) and a **disambiguation hint** on the entities involved ("Ahmed at Acme = Ahmed Samir; Ahmed at Petrol Arrows = Ahmed Fathy"), stored in `.meta/` and fed to future resolution prompts.



- **Node kinds:** `note`, `concept`, `person`, `company`, `document`, `place`, `attachment` (deferred), `tag` (optional toggle), `cluster` (virtual, for region labels).
- **Edge kinds:** `link` (body wikilink), `embed`, `relation:<type>` (with `by`, confidence, reason), `similarity` (ephemeral), `concept` (note→concept), `mention` (note→person/company), `entity:<type>` (works-at, client-of, …), `custody:<location|holder|last-holder>` (document→place/person), `part-of-place` (place→place).
- **Entity lens:** the graph API accepts `lens=people|companies` to return an entity-centred graph (entities as nodes, edges = entity relations + co-mention strength), so the owner can see the network of people and companies directly.
- Node payload: id, title, kind, cluster_id, degree, lang, updated, short summary (for hover).
- Positions are computed client-side; only user-pinned layouts are saved (JSON Canvas in `maps/`).
- Clustering per D10 on the undirected graph of links + relations (weights: user links > high-confidence AI relations > low-confidence). Cluster names generated by LLM from top titles/concepts, kept stable across runs by matching new clusters to old ones by member overlap.

---

## 11. Strata app (Flutter, all platforms, adaptive)

**Stack:** Flutter (stable channel) targeting Android, iOS, macOS, Windows, Linux; Material 3 base themed with the Strata tokens (`/design`); UI binding per D11; editor per D2; graph renderers per D3/D4 — each wrapped behind its own widget so the implementation can be swapped. Dart holds no logic (L15).

### 11.1 Flutter stack
- **Workspace:** Dart pub workspaces managed with **melos** (bootstrap, scripts, versioning, per-package CI filters).
- **Packages:** `apps/strata` (app shell, routing, platform setup) · `packages/strata_bridge` (flutter_rust_bridge v2 bindings, built per platform with cargokit) · `packages/strata_ui` (design tokens as `ThemeExtension`s, typography with bundled fonts, adaptive scaffold, shared widgets) · `packages/strata_l10n` (ARB files, `gen-l10n`, English + Arabic) · one package per feature under `packages/features/` (home, inbox, notes, editor, tasks, directory, documents, maps, ask, sync, settings, accounts, admin) — each UI only.
- **State & DI:** **Riverpod 3** with `riverpod_annotation` + `riverpod_generator` + `riverpod_lint`, and `hooks_riverpod`/`flutter_hooks` for ephemeral widget state (text controllers, animations). Providers only adapt frb streams and futures into rebuilds and inject the bridge (overridden with a fake bridge in tests); they hold no logic (L15).
- **Pattern:** MVVM where the view-model lives in the Rust core; feature-first packages; unidirectional flow (widget → intent → core → view-model stream → widget); sealed classes and pattern matching for view states; immutable frb-generated types (with `freezed`-style equality from frb).
- **Navigation:** **go_router** with typed routes (`go_router_builder`), `StatefulShellRoute` for the adaptive shells (bottom bar / rail / sidebar chosen by size class), deep links for notifications.
- **Adaptive layout:** own size-class breakpoints and shells in `strata_ui` (no discontinued scaffold packages); keyboard shortcuts via `Shortcuts`/`Actions`, context menus, hover and focus handling on desktop.
- **Editor (D2 = b):** a maintained rich-text editor package with markdown serialisation (candidate: `super_editor` + its markdown package; verified for desktop, mobile, RTL, and round-trip fidelity before adoption), with custom components for wikilinks, @mentions, tags, block IDs, and task lines.
- **Graphs:** global map on a single `CustomPainter` with positions streamed from the Rust core (D3 = a); local mind map as a widget-based canvas inside `InteractiveViewer` (D4 = b).
- **Notifications (D27):** `firebase_messaging` (FCM on Android, APNs on iOS/macOS), Windows push via WNS (plugin verified before adoption), `flutter_local_notifications` to display event-stream reminders on Linux and desktop.
- **Quality:** `very_good_analysis` + `riverpod_lint` + `custom_lint`, strict analyzer settings, `dart format` enforced; tests with `flutter_test`, **alchemist** golden tests, `mocktail`, `integration_test` (+ `patrol` where native dialogs/notifications are involved).

**Adaptive layouts (not just responsive):** the app picks a distinct layout per window size class, re-evaluated live on resize:

| Class | Width | Navigation | Panes |
|---|---|---|---|
| Compact | < 600 | Bottom navigation bar (Home, Inbox, Notes, People, Ask — five is the platform limit for a bottom bar; Tasks live on Home, D28 = c); top app bar with search and sync pill | One pane; detail pushes full-screen; context (backlinks, relations, history) in sheets/tabs |
| Medium | 600–1199 | Navigation rail with capture button, destinations Home, Inbox, Tasks, Notes, Map, People, Ask (scrolls if the window is short), Settings at bottom | List + detail; context panel as an overlay drawer |
| Expanded | ≥ 1200 | Scrollable sidebar (wordmark, New capture, nav with counts incl. Tasks, pinned, folder tree, sync status) | List + main + context panel (backlinks / graph / history); keyboard shortcuts (⌘/Ctrl-K search, ⌘/Ctrl-N capture), hover states, context menus, multi-window-friendly |

- Input adapts too: touch targets ≥ 48 dp on touch platforms; pointer/hover affordances, right-click menus and full keyboard navigation on desktop; focus traversal order defined per screen.
- Global map is available at medium and expanded; compact shows local mind maps only.
- Light/dark themes; system text scaling respected up to 200% without clipped or overlapping UI.

**Screens:**
1. **Login & sign-up** — server URL, username, password, device name; sign-up form; "waiting for approval" and "not approved" states.
2. **Home / Capture** — prominent capture box (works well with Wispr Flow dictation), recent notes, inbox count.
3. **Inbox** — captures with AI filing suggestions: accept / edit / reject per item and bulk; entity link-or-create suggestions (nicknames, ambiguous mentions).
4. **Note view/editor** — editor, frontmatter properties panel (relations shown as typed chips), backlinks panel grouped by relation type, history panel with diff and revert, local mini-graph.
5. **Global map** — whole vault, clusters as coloured labelled regions, zoom-dependent labels, filters (edge types, node kinds, similarity toggle, cluster focus), search-to-focus.
6. **Local mind map** — focused note centre, relations radial, edge labels by type, tap/click to recentre, drag node onto node to create relation (AI proposes type), hover/long-press edge for AI reason, delete/retype edge, "save layout" → `.canvas`.
7. **Directory** — People, Companies, Documents, Places as tabs, with search (both scripts), filters (tag/role/industry; document type, status, place, holder, expiring), recently active.
7a. **Document page** — properties, where it is now (place breadcrumb, e.g. Nasr City office › Safe), holder / last holder, custody timeline (cited), copies, expiry; "Record a move" action.
7b. **Place page** — nested places, everything stored here and in sub-places, recent movements.
8. **Entity page** — header with properties and aliases; Summary, Insights, Open items, Timeline (each bullet opens its source block); mentioning notes feed; related entities; local entity graph; merge action; user `## Notes` editor.
9. **Search** — keyword/semantic/hybrid, results with snippets.
10. **Ask** — chat-like Q&A over the vault with tappable citations; "save as note"; unavailable offline.
11. **Suggestions** — duplicates, low-confidence proposals, new-entity and entity-merge proposals.
12. **Tasks** — compact: on Home as Today / Upcoming / Recurring sections (no bottom-bar tab); medium and expanded: a **Tasks** destination in the rail and sidebar. Task detail and editor: text, due, recurrence (presets + custom), reminders, linked note and entities, history of completed occurrences.
12a. **Duplicate prompt** — shared "Already exists" sheet/dialog used by every create flow: candidates with match reason, Open existing / Create anyway / Cancel.
12b. **Sync status & Conflicts** — online/offline, pending outbox ops, last sync, conflict resolution per D19.
13. **Settings** — account (password, language), AI thresholds, auto-file toggle, budget, digests schedule, devices (revoke), export/import, integrity warnings, AI status, sign out.
14. **Admin → Users** (admins only) — pending approvals queue (approve / reject), list, create, disable, reset password, schedule deletion (with grace period, cancel, export-downloaded status).
15. **Deletion pending** — restricted screen for an account scheduled for deletion: days remaining, download export, delete now.

**Live updates:** the core subscribes to `/events` (D24) and pulls changes; view-model streams update the UI; graph node/edge additions animate.

**Arabic/English:**
- UI localisation for both languages (UI direction follows UI language).
- Content direction is determined **per paragraph/line** by the first strong character (per-paragraph `TextDirection` in rendered views; per-line direction in the editor).
- Test cursor movement, selection, and wikilink autocomplete in mixed lines.
- Typefaces from the brand system: Cairo (UI, Arabic and Latin content), IBM Plex Mono (technical), wordmark in Quicksand; bundled with the app, not fetched at runtime.

**Editor features:** wikilink autocomplete (`[[`), `@` mention autocomplete for people/companies (inserts a link and adds to `people:`/`companies:`), tag autocomplete, block reference picker, paste-image (deferred until attachments), keyboard shortcuts, conflict UI on `409` (show diff, choose merge).

---

## 12. Client core (Rust, offline-first, all platforms)

The same core runs on every platform; one app install, same API, own login (device row).

### 12.1 Layering (hard rule, L15)
```
Flutter widgets (Dart)      render view-models, forward user intents. Nothing else.
        │  frb-generated bindings (the only bridge)
Rust client core (/client/core)
  api/      frb facade: the ONLY surface Dart can call (async fns + Streams)
  view/     view-models: screen state structs streamed to Dart (lists, note, entity, inbox, sync status)
  store/    local SQLite (schema, migrations, FTS5)
  sync/     outbox, push, pull, bootstrap, conflicts, retry/backoff, connectivity
  search/   local FTS with /crates/text-normalize
  graph/    local neighbourhood from cached relations
  auth/     login, refresh, device, token storage (D14: in the account's local DB)
  format/   uses /crates/vault-format (parse, render hints, highlight spans for the editor)
  net/      depends on /api/rust-client (generated) + its streaming module
```
- Dart may hold only ephemeral widget state (scroll position, text controller contents while typing, animation). Any derived or persistent state comes from a core stream.
- Errors reach Dart as typed enums from the core, already localised-ready (message keys).
- CI enforces: pubspec dependency allowlist (no http/dio/sqflite/drift/etc.), and a lint rule that Dart files outside `lib/ui/` and `lib/bridge/` fail the build.

### 12.2 Local database (full cache)
Tables mirror the server data the app needs, **fully cached**: `notes` (full markdown + parsed frontmatter + version), `entities`, `entity_aliases`, `relations`, `rejected`, `links`, `tags`, `inbox`, `suggestions`, `clusters`, `notes_fts` (FTS5, Arabic-normalised). Plus:
```
outbox      (op_id PK ULID, kind, entity_id, base_version, payload (MessagePack), status[pending|inflight|done|conflict|rejected], attempts, last_error, created)
sync_state  (epoch, cursor_seq, last_pull_at, last_push_at, bootstrap_complete)
conflicts   (op_id, entity_id, local_payload, server_version, resolution, created)
```
Attachments (deferred capture) are cached lazily, not in the initial bootstrap.

### 12.3 Write path (offline or online, identical)
1. User intent → core function (e.g. `update_note(id, content)`).
2. Core applies the change to the local tables immediately (optimistic), updates local FTS/links using `vault-format`, and appends an outbox op with the current `base_version`.
3. View-model streams emit the new state instantly.
4. Sync pushes the outbox when online (§12.4).

### 12.4 Sync engine
- **Bootstrap** on first login or epoch change: page through `/sync/bootstrap` into the local DB, then switch to incremental.
- **Push:** send pending ops in order in batches; mark `done` with the server version, `conflict` → apply D19 policy and surface in the Conflicts screen, `rejected` → roll back the optimistic change and notify.
- **Pull:** `/sync/changes?since=` after each push, on app start/resume, on connectivity regained, on a timer while foregrounded, and whenever an `/events` message arrives (events only trigger pulls; data always comes from `changes`).
- Replays are safe (`op_id` idempotency). Backoff on failure; outbox survives app restarts.
- AI results (relations, concepts, entity insights, filing suggestions) arrive through normal pulls.

### 12.5 Offline behaviour
- Works offline: capture, browse, edit, search, backlinks, entity pages, local mind map, accept/reject suggestions, create/remove relations, request relink (queued).
- Online only: Ask (shown as unavailable offline), history/revert, export/import.
- A persistent sync indicator shows: online/offline, pending op count, last sync, conflicts.

### 12.5b Reminders on the device (D27 = b)
- Reminders are sent by the server (§9.2 `reminders` job): **FCM** for Android, **APNs** for iOS and macOS, **WNS** for Windows. Linux has no platform push service, so on Linux reminders arrive over the live event stream while the app is running and are shown as desktop notifications.
- On sign-in the core registers the device's push token (`PUT /devices/{id}/push`); sign-out and account deletion remove it.
- Tapping a reminder opens the task; "Done" and "Snooze" notification actions go through the outbox like any other mutation.
- **Future (D27 → c):** devices will also schedule local notifications from the synced task list as an offline fallback, de-duplicated with pushes by (task, remind_at).

### 12.6 Offline availability by screen
Works offline: capture, browse, edit, search, backlinks, entity pages, local mind map, global map (from cached data), accept/reject suggestions, create/remove relations, request relink (queued). Online only: Ask, history/revert, export/import, Admin → Users.

### 12.7 Accounts on the device
- One signed-in account at a time. Each account gets its **own local database file**, keyed by user ID; switching accounts never mixes data.
- Sign-out: if the outbox has pending ops, warn and offer to sync first; then delete that account's local DB and tokens.
- Account disabled on the server → next sync returns `401`; the core wipes that account's local data after warning.
- Account scheduled for deletion → next sync returns `403 account_deletion_pending`; the app shows the deletion-pending screen (days remaining, "Download your export" via `GET /me/export` saved to a user-chosen location, unsynced outbox ops listed and exportable as a file, "Delete now"). Local data stays read-only until the user finishes or the grace period ends, then it is wiped.

**Deferred:** voice/photo/file capture (§18), share-sheet target.

---

## 13. Phase 0 — Design session deliverables

Run as a design session (Claude Design) before any code. Output to `/design`:

1. **Resolve D2–D4** with the owner (present options neutrally).
2. **Product flows:** capture → inbox → filed; note → auto-links appear; explore map → open note; ask → cite → open block; reject an AI edge; rename a note; nickname mention → link-or-create suggestion → alias added.
3. **Screen inventory** with states, at every size class: empty vault, loading, AI offline, budget exhausted, conflict (409), integrity warning, offline, syncing, pending outbox, sync conflict, first-time bootstrap progress, account disabled.
4. **Design tokens:** colour (light/dark), typography (Latin + Arabic pairing), spacing, radii, elevation, motion. Export as JSON + a Flutter `ThemeData`/`ThemeExtension` mapping of the same tokens.
5. **Graph visual language:**
   - Node shape/size/colour per kind (person and company nodes clearly distinct from notes/concepts); cluster region styling.
   - Edge styles per relation type (solid/dashed/arrowed; colour + pattern so meaning isn't colour-only).
   - Similarity edges visibly weaker.
   - Label rules at zoom levels; selection/hover/focus states.
   - Clean, precise vector lines only (L11).
6. **Adaptive layout system:** size classes, navigation per class, pane rules, input adaptations (touch vs pointer/keyboard).
7. **Key screen designs** at compact, medium, and expanded sizes, light + dark, one Arabic-content example: Home/Capture, Inbox, Note editor with relations/backlinks, Global map, Local mind map, People & Companies directory, Entity page, Ask, Sync status & Conflicts.
8. **Login, account/sign-out flows, and Admin → Users** at every size class.
9. **View-model shapes** alongside screens, since the Rust core streams exactly what each screen renders.
10. **Stop gate:** owner approves the design before Phase 1.

---

## 14. Deployment

- **VPS**: Linux, systemd service `stratad` running as an unprivileged user; data root `/srv/strata` laid out per §5.2, permissions 0700 for that service user only.
- **nginx**: TLS (Let's Encrypt), reverse proxy to Actix with streaming-friendly settings for D24 (`proxy_buffering off`, long read timeout, WebSocket upgrade headers if D24 = a), security headers (HSTS). No static site.
- **Config**: `stratad.toml` + env for secrets. Settings: data root, bind address, AI provider/model/keys, embedding config, thresholds, budgets, schedules.
- **Database**: the existing PostgreSQL instance on the VPS; `stratad` gets its own database and the three roles from §5.2. Connection pool kept small (VPS: 4 GB RAM, 1 core).
- **Backups**: out of scope for this project. The owner already runs WAL archiving and full backups for PostgreSQL and handles vault-directory backups with their own DevOps scripts.
- **Observability**: structured logs (`tracing`), `/api/v1/health` (unauthenticated liveness only, no data), AI usage stats.
- **App distribution**: Android (Play / APK), iOS (App Store / TestFlight), macOS (signed + notarised), Windows (signed MSIX), Linux (AppImage or Flatpak); release builds per platform in CI.
- **CI** (all required, no retries — a flaky test is a bug): Rust fmt/clippy (deny warnings)/tests with coverage gates for backend, shared crates, client core; fuzz smoke runs; mutation testing on the critical crates; OpenAPI generation + Rust client regeneration diff check + breaking-change diff; frb codegen diff check; Flutter analyze (strict) + widget/golden tests at every size class + integration tests; full-stack E2E on Linux desktop and an Android emulator; **Dart logic guard** (pubspec allowlist + folder rule from §12.1); builds for every target platform.

---

## 15. Security checklist
- No endpoint returns raw filesystem paths outside the vault; path traversal tests on every path parameter.
- Import zip: reject symlinks, absolute paths, `..`, oversized entries.
- Rate limits on login, signup, capture, ask. Signup is additionally limited per IP and globally, with a cap on pending accounts; usernames are normalised (case, Unicode confusables) before uniqueness checks.
- Content never included in logs or error messages by default.
- Secrets file permission check at startup.
- MessagePack decoding limits enforced and fuzzed (§7.7).
- Tenant isolation: every endpoint tested with another user's IDs/paths → `404`; search, graph, ask, similarity, entity resolution, embedding candidates, events, and sync never return another user's data.

---

## 16. Testing strategy

The bar is a polished product, not a demo. Tests are part of every feature, written with it, and gate every merge. `docs/TESTING.md` expands this section with conventions and examples.

### 16.1 Rules for every test
- **Assert exact outcomes**: specific values, full payloads, file bytes, event sequences, error codes and problem types. A test that only checks "no error" or "status 200" is incomplete.
- **Deterministic**: injectable clock, seeded RNG, fake LLM provider, temp directories; no network, no sleeps for timing (use fake time and explicit synchronisation).
- **Isolated**: each test builds its own users, vaults, and databases via `testkit`; tests run in parallel safely.
- **No retries in CI.** A flaky test is a bug and is fixed, not re-run.
- **Every bug fix starts with a failing regression test.**
- **Definition of done** for any task: code + tests at every layer it touches + contract updated + docs updated + CI green.

### 16.2 Shared crates (`vault-format`, `text-normalize`, `domain`)
- **Golden tests:** parse → serialise round-trip preserves unknown frontmatter keys, body bytes, and link syntax; canonical order applied; Arabic filenames; forbidden chars.
- **Property tests (proptest):** round-trip identity over generated documents (random frontmatter, links, embeds, block IDs, mixed scripts, CRLF/LF); normalisation is idempotent; alias matching is symmetric where specified.
- **Fuzzing (cargo-fuzz):** parser and frontmatter targets; short runs on every PR, long scheduled runs; any crash becomes a committed regression case.
- **Arabic normalisation cases:** alef forms, ta marbuta, ya/alef maqsura, tashkeel, tatweel, mixed-direction strings.
- **Mutation testing (cargo-mutants):** surviving mutants in these crates fail CI unless explicitly justified.

### 16.3 Backend
- **Unit tests** for every module; **integration tests** against a real PostgreSQL (each test gets its own database cloned from a migrated template) and real temp git repositories (no mocks of storage).
- **RLS at the database level:** connected as `strata_app`, a transaction with no scope sees zero rows in every user-owned table; a transaction scoped to user A sees none of B's rows and cannot insert or update rows with B's `user_id` (policy `WITH CHECK` violation asserted); `strata_accounts` gets permission errors on every user-owned table; the schema-enumeration test from §5.2.
- **API tests** through the generated Rust client against an in-process server, MessagePack on the wire, asserting full decoded payloads.
- **Contract conformance:** every response in every API test is validated against its OpenAPI schema; a schema-driven fuzzer (e.g. Schemathesis with a MessagePack serializer, or an in-repo proptest generator from the contract) hits every operation and asserts no 5xx, and that every response conforms to the contract.
- **Wire-format goldens:** byte-exact MessagePack fixtures for representative payloads; decode limits (depth, size, trailing bytes) tested.
- **Obsidian compatibility fixture:** a sample vault (links, aliases, headings, block refs, embeds, properties, `.canvas`) must export → import → export identically.
- **Link rewriting:** move/rename updates all inbound links and relations; ambiguous-name resolution.
- **Concurrency:** parallel writes serialised; stale `If-Match` returns 409; per-user writers don't block each other.
- **Reconciliation:** simulate out-of-band file change and uncommitted git state; verify detection and repair.
- **AI (FakeLlmProvider):** threshold application, rejected-edge suppression, removal of stale AI edges only, invalid JSON retry, budget pause (per user and global), fair scheduling across users.
- **Entities:** alias matching across Arabic/Latin spellings; nickname/kinship mention → suggestion, never auto-created; accepting adds the alias and the next mention resolves automatically; relative dates in Timeline resolved against `created`; ambiguous match → suggestion; merge rewrites links and preserves user `## Notes`; AI sections regenerate without touching user sections; uncited insight bullets rejected; speculative insights from a one-line capture rejected; contact fields never written by AI.
- **Search:** Arabic normalisation cases end to end (index + query).
- **Account deletion (D25):** scheduling revokes all sessions; a new login yields an export-only session; every non-allowed endpoint returns `403 account_deletion_pending`; `GET /me/export` contains exactly the user's vault and nothing of any other user; admins have no route to the export; cancel restores `active`; the purge job (fake clock past the grace period, and early confirm) removes the directory and every row of that user across all tables and nothing else; audit entry written.
- **Documents & places (§6.12):** nested-place queries return documents in sub-places; frontmatter always equals the newest custody event after any sequence of events (proptest); copies tracked separately; the example capture "Watanya's contract is at the Nasr City office in the safe, last with Shady" produces exactly: location = Safe — Nasr City office (part-of Nasr City office), holder empty, last-holder Shady, status stored, one cited custody event; confidence below threshold, ambiguous entities, unknown places, and conflicts with a newer event each produce a suggestion instead of an update; relative dates resolved per §6.7; AI never sets location/holder without a custody event; revert of the `ai:` commit restores the previous state.
- **Tasks (§6.11):** parse/serialise round-trip of every signifier, reminder markers, and block IDs (golden + proptest); recurrence grammar → RRULE table tests (monthly on the 1st, every Sunday, every 2 weeks, end of month, leap years, DST transitions in the user's timezone); completion of a recurring task writes exactly the expected two lines; unsupported recurrence preserved byte-for-byte and flagged; tasks survive note rename/move.
- **Reminders:** fake clock + fake push providers; each reminder sent exactly once per device even with job retries and restarts; timezone and DST correctness; token removal on sign-out/deletion; Linux event-stream delivery.
- **Duplicates (§9.7):** exact/near/semantic detection per kind, Arabic and English variants ("Watanya's ETA invoice" vs "ETA invoice for Watanya", "ووتانيا"/"وطنية" spelling variants as configured aliases); `409 duplicate_candidates` payload asserted exactly; `force` creates and records keep-both; keep-both pairs never re-flagged by create checks or the nightly sweep; captures never refused; offline local check and server `duplicate{}` push result; AI-off path still runs exact + near.
- **Accounts (D22):** signup creates `pending` with no vault or session; pending/rejected/disabled users cannot log in (exact problem types asserted); approval creates the user directory exactly once; only admins can approve; signup rate limits and pending cap enforced; confusable usernames rejected.
- **Isolation:** two users with overlapping note titles/entity names; verify zero leakage across every endpoint, job, event stream, sync feed, and export; admins cannot read member vaults; a disabled user's sessions die immediately.
- **Migrations:** each migration tested from the previous schema with fixture data.
- **Security checklist items (§15)** each have a dedicated test.

### 16.4 Client core (pure Rust, no Flutter needed)
- **Every frb-exposed function** has tests; view-model streams are tested headless against a real in-process backend (from `testkit`), asserting the exact sequence of emitted view-models.
- **Sync as a state machine (proptest):** randomized op sequences from two or more offline devices + server converge to the same state; replayed pushes are idempotent; epoch change triggers re-bootstrap; each D19 conflict path; outbox survives simulated crashes at every step; offline-created IDs stay stable.
- **Accounts:** separate DB per account; sign-out with pending ops warns and never loses data silently; disabled account wipes local data after the warning.
- Local search and graph layout tested against the same fixtures as the backend so results agree.

### 16.5 Flutter app
- **Widget tests for every screen at every size class:** compact (390×844), medium (1024×768), expanded (1440×900), and a large desktop size (1920×1080); plus live resize across breakpoints, asserting the right navigation and pane structure appear.
- **Golden tests** for every screen and key state (empty, loading, offline, conflict, error, AI paused, account disabled) × size class × light/dark × LTR (English UI) / RTL (Arabic UI) × text scale 1.0 and 2.0. Goldens are rendered with the bundled fonts on a pinned CI image.
- **Accessibility checks** in widget tests at every size: tap-target, labelled-tap-target, and text-contrast guidelines; semantics labels on every interactive element; keyboard focus order on desktop layouts.
- Widgets are fed view-model fixtures (the same types the core streams); no business logic to test in Dart (L15), so every Dart test is about rendering and interaction wiring.
- **Integration tests (`integration_test`)** run the real app with the real core against a spawned backend.

### 16.6 End-to-end (full stack)
- Spawned `stratad` on a temp data root + the real app on **Linux desktop (CI, headless display)** and an **Android emulator**; macOS/Windows/iOS runs on release branches.
- Scenarios (each asserts UI state and server state): first login; capture → inbox → accept filing; edit a note, go offline, edit again, reconnect, resolve a conflict; entity page shows cited insights from fixture AI; nickname link-or-create → alias added → next mention auto-links; reject an AI edge and verify it never returns; ask with citations opens the cited block (fake LLM); admin creates a user, the user signs in on another device, isolation verified; sign-out with pending outbox; account disabled mid-session; account scheduled for deletion → export downloaded in the app → deletion completes; "remind me to make Watanya's invoice" when that recurring task exists → flagged → create anyway → both exist and the pair is never flagged again; recurring task completed → next occurrence appears and its reminder is delivered to the device.

### 16.7 Coverage and performance gates
- Line coverage ≥ 90% and branch coverage tracked for backend, shared crates, and client core (`cargo-llvm-cov`); the threshold only goes up.
- Benchmarks (`criterion`) for parsing, indexing, search, sync apply, graph layout; CI fails on significant regressions against the stored baseline.
- A 10k-note synthetic vault is part of the performance suite from Phase 2 onward (sync bootstrap time, search latency, map frame time).

---

## 17. Phased roadmap (each phase ends with a stop gate)

Every phase's acceptance includes: all tests required by §16 for its scope written and passing in CI, coverage gates met, contract and docs up to date.

| Phase | Scope | Acceptance |
|-------|-------|------------|
| **0 Design** | §13 | Owner approves design; D2–D4 recorded |
| **1 Foundation** | Monorepo, CI with all gates, `testkit`, shared crates (`vault-format`, `text-normalize`, `domain`), backend skeleton, config, **multi-user foundations (L20, L22, D21, D22: PostgreSQL schema with RLS and the three roles, per-user vault directories, `UserScope` transactions, signup/approval and admin endpoints)**, auth + devices (D6, D7), MessagePack wire layer (L21) and streaming transport (D24), vault store (parse/serialise/atomic write/git), notes CRUD, manual entity CRUD + merge, tree, move with link rewrite, soft delete, history/revert, export/import, reconciliation, change log + idempotency + sync endpoints (D19), tasks (§6.11: parsing, recurrence, completion, API), documents & places with manual custody events (§6.12), duplicate detection exact + near (§9.7) with `force`, reminders job + push providers (D27), utoipa OpenAPI + generated Rust client (D15), deploy scripts | Golden, property, fuzz-smoke, contract-conformance, and isolation suites pass; deployed to VPS behind nginx |
| **2 Client core** | Rust client core (§12.1–12.5, 12.7): local DB per account, outbox, sync engine, local search/graph, auth + token storage (D14), view-models, frb facade — tested headless | Sync convergence and conflict tests pass; full offline edit → reconnect → sync works in tests; 10k-note bootstrap within budget |
| **3 App core** | Flutter app (§11, D11, D2): adaptive shell at all size classes, login, capture, inbox (manual), note view/editor, backlinks, People & Companies + entity pages (manual data), `@` mentions, search, history/revert, sync status & conflicts, settings, Admin → Users, RTL; CI Dart logic guard; builds for all five platforms | Owner uses the app on phone and desktop, fully offline and back; widget + golden tests at every size class pass; E2E suite green on Linux desktop and Android |
| **4 AI linking** | Semantic duplicate detection, task suggestions and document custody extraction (D30) from captures, provider layer (`claude -p` per L18 for the owner, D23 for other users, local granite embeddings per L19, runtime D9), jobs, embeddings, summaries, auto-linking, concepts, entity extraction/resolution (D13, nickname rules), entity insights (no-speculation and timeline-date rules), inbox filing, suggestions UI, ask with citations, budget/status | New notes get typed relations and people/company links automatically; entity pages show cited insights, open items, timeline; rejected edges stay rejected; ask cites real blocks |
| **5 Graph** | Graph APIs, similarity, clustering (D10) + names, global map (D3), local mind map (D4), edge edit interactions, entity lens, saved `.canvas` layouts, live animations | Map reflects vault live; saved maps open in Obsidian after export; map performance budget met at 10k notes |
| **6 Hardening** | Digests, dedupe suggestions, performance with 10k-note synthetic vault, security checklist, store/release pipelines | Checklist passes; signed releases for every platform |
| **7 Deferred** | §18, when the owner starts it | Per §18 |

---

## 18. Deferred requirements

### 18.1 Phone capture (voice, files, images, chat)
- Channels (owner picks later): in-app recording/upload (via core outbox, uploads when online), Telegram bot, WhatsApp. All call one `POST /capture/raw` (multipart).
- **Raw file saved first**, unchanged, to `attachments/YYYY/MM/<ulid>.<ext>`; sha256 recorded.
- Capture note created in `inbox/` with `source: "[[attachments/...]]"` in frontmatter and an embed `![[attachments/...]]` in the body.
- Jobs: speech-to-text (provider benchmarked on real Egyptian Arabic + mixed voice notes before choosing), OCR for images, text extraction for PDFs/docs. Output appended under a `## Transcript` / `## Extracted text` heading in the capture note.
- Normal filing and linking then run; the raw file stays linked wherever the note is moved.
- Reprocess endpoint: re-run transcription/extraction from the original raw file with a newer model.
- Bot channels authenticate by whitelisted chat/user ID mapped to a device row.

### 18.2 Others
- Share-sheet capture target (Android/iOS).
- Obsidian plugin as an API client (ask, suggestions, sync of a local copy **through the API only**).
- Multiple vaults per user.

---

## 19. Glossary
- **Vault** — a user's folder of markdown notes; the source of truth for that user.
- **Account** — an isolated user with their own vault, index, jobs, budget, and devices; `admin` or `member`.
- **Relation** — typed, directed edge stored as a frontmatter key on the source note, with provenance in `.meta/`.
- **Concept** — AI-created note representing a recurring entity or idea, linked via `concepts:`.
- **Entity** — a person or company note (`kind: person|company`) that aggregates mentions, AI insights, open items, and a timeline.
- **Suggestion** — an AI proposal awaiting user accept/reject (filing when auto-file is off, duplicates, low-confidence items, entity link-or-create).
- **Outbox** — local queue of pending mutations in the client core, each with an idempotency `op_id`, pushed to `/sync/push` when online.
- **Client core** — the Rust crate holding all client logic, exposed to Flutter via flutter_rust_bridge; the same core on every platform.
- **Size class** — compact / medium / expanded window-width band that selects an adaptive layout (§11).
- **Stop gate** — end-of-phase checkpoint requiring owner approval.

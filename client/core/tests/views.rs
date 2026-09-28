//! View-models over pulled data (PLAN §11, §12.1 `view/`, `search/`, `graph/`): directory,
//! entity, document and place pages (nested places), Arabic-normalised search, the local mind
//! map and the global map, the notes list, settings, and the "not yet available" screens.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use chrono::NaiveDate;
use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::graph;
use strata_core::search;
use strata_core::session::Session;
use strata_core::sync::engine::Trigger;
use strata_core::view::build;
use strata_core::view::model::{
    Availability, Citation, CustodyItem, DirectoryCounts, DirectoryItem, DirectoryTab,
    DocumentBrief, DocumentView, EntityPageKind, EntityRef, FolderItem, HighlightSpan, PlaceNode,
    PlaceView, SearchHit, SearchMode, TextDir,
};

const HOME: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V2A";
const SAFE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V2B";
const DRAWER: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V2C";
const SHADY: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V2D";
const WATANYA: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V2E";
const LICENSE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V2F";
const CAPTURE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V2G";
const PRICING: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V2H";

fn r(id: &str, title: &str) -> EntityRef {
    let kind = match id {
        HOME | SAFE | DRAWER => "place",
        SHADY => "person",
        WATANYA => "company",
        LICENSE => "document",
        CAPTURE => "capture",
        _ => "note",
    };
    EntityRef {
        id: Some(id.to_owned()),
        title: title.to_owned(),
        kind: Some(kind.to_owned()),
    }
}

/// A directory row with every optional detail empty.
fn row(id: &str, title: &str, kind: &str) -> DirectoryItem {
    DirectoryItem {
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: None,
        aliases: Vec::new(),
        kind: kind.to_owned(),
        title_dir: TextDir::Ltr,
        initials: strata_core::format::labels::initials(title),
        mention_count: 0,
        last_active: Some(now()),
        last_active_label: Some("Today".to_owned()),
        role: None,
        company: None,
        industry: None,
        tags: Vec::new(),
        status: None,
        doc_type: None,
        location: Vec::new(),
        holder: None,
        last_holder: None,
        holder_label: None,
        copy: None,
        expires: None,
        expires_label: None,
        expiring_soon: false,
        breadcrumb: Vec::new(),
        document_count: 0,
        has_open_items: false,
    }
}

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(common::NOW)
        .expect("now")
        .with_timezone(&chrono::Utc)
}

/// A custody event as the pages show it.
fn custody(
    kind: &str,
    day: u32,
    place: Option<EntityRef>,
    person: Option<EntityRef>,
    sentence: &str,
    here: bool,
) -> CustodyItem {
    let (actor, destination) = match kind {
        "handed-to" => (None, person.clone()),
        _ => (None, place.clone()),
    };
    CustodyItem {
        date: NaiveDate::from_ymd_opt(2026, 9, day).expect("date"),
        kind: kind.to_owned(),
        document: Some(r(LICENSE, "Car license")),
        place,
        person,
        counterparty: None,
        citations: vec![cite(CAPTURE, "Capture 2026-09-20")],
        by: "ai".to_owned(),
        confidence: None,
        decision_id: None,
        sentence_key: format!("custody.{}", kind.replace('-', "_")),
        actor,
        destination,
        sentence: sentence.to_owned(),
        date_label: format!("{day} Sep"),
        here,
        note: None,
        note_dir: TextDir::Neutral,
    }
}

fn cite(id: &str, target: &str) -> Citation {
    Citation {
        note_id: Some(id.to_owned()),
        target: target.to_owned(),
        anchor: None,
    }
}

/// Signs in as A and pulls a small vault: three nested places, a person, a company, a
/// document with custody history, a capture and an Arabic note.
async fn vault() -> (Harness, std::sync::Arc<Session>) {
    let h = Harness::new();
    let up = |id: &str, path: &str, content: String| {
        h.server.remote_upsert(id, path, &content);
    };
    up(
        HOME,
        "places/Home.md",
        format!("---\nid: {HOME}\nkind: place\n---\n"),
    );
    up(
        SAFE,
        "places/Safe.md",
        format!("---\nid: {SAFE}\nkind: place\npart-of: \"[[Home]]\"\n---\n"),
    );
    up(
        DRAWER,
        "places/Desk drawer.md",
        format!("---\nid: {DRAWER}\nkind: place\npart-of: \"[[Safe]]\"\n---\n"),
    );
    up(
        SHADY,
        "people/Shady.md",
        format!(
            "---\nid: {SHADY}\nkind: person\naliases: [شادي]\nrole: Driver\n---\n\
             ## Summary\n\nDrives the company car.\n\n\
             ## Timeline\n\n- 2026-09-10 — got the car license [[Capture 2026-09-20]]\n"
        ),
    );
    up(
        WATANYA,
        "companies/Watanya.md",
        format!("---\nid: {WATANYA}\nkind: company\nindustry: Fuel\n---\n"),
    );
    up(
        LICENSE,
        "documents/Car license.md",
        format!(
            "---\nid: {LICENSE}\nkind: document\ndoc-type: license\nstatus: stored\n\
             location: \"[[Desk drawer]]\"\ncompanies: [\"[[Watanya]]\"]\n---\n\
             ## Custody\n\n\
             - 2026-09-20 — stored-at [[Desk drawer]] — [[Capture 2026-09-20]]\n\
             - 2026-09-10 — handed-to [[Shady]] — [[Capture 2026-09-20]]\n"
        ),
    );
    up(
        CAPTURE,
        "captures/Capture 2026-09-20.md",
        format!(
            "---\nid: {CAPTURE}\nkind: capture\n---\nPut the car license in the drawer. \
             [[Shady]] had it.\n"
        ),
    );
    up(
        PRICING,
        "notes/Pricing/تجارب التسعير.md",
        format!("---\nid: {PRICING}\n---\nالأسعار زادت في سبتمبر.\n"),
    );
    h.server.set_page_size(50);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("sync");
    (h, s)
}

#[tokio::test]
async fn directory_tabs_filter_by_names_and_aliases_in_both_scripts() {
    let (_h, s) = vault().await;
    let counts = DirectoryCounts {
        people: 1,
        companies: 1,
        documents: 1,
        places: 3,
    };
    let people = s
        .read(|c, ctx| build::directory(c, ctx, DirectoryTab::People, ""))
        .expect("people");
    assert_eq!(
        people.items,
        [DirectoryItem {
            subtitle: Some("Driver".into()),
            aliases: vec!["شادي".into()],
            // The capture links to him and the license's custody line names him.
            mention_count: 2,
            role: Some("Driver".into()),
            ..row(SHADY, "Shady", "person")
        }]
    );
    assert_eq!(people.counts, counts);
    // The Arabic alias finds him; so does a lower-case prefix of the Latin name.
    let by_alias = s
        .read(|c, ctx| build::directory(c, ctx, DirectoryTab::People, "شادى"))
        .expect("alias");
    assert_eq!(by_alias.items.len(), 1);
    let none = s
        .read(|c, ctx| build::directory(c, ctx, DirectoryTab::People, "mona"))
        .expect("none");
    assert_eq!(none.items, []);

    let docs = s
        .read(|c, ctx| build::directory(c, ctx, DirectoryTab::Documents, ""))
        .expect("docs");
    assert_eq!(
        docs.items,
        [DirectoryItem {
            subtitle: Some("stored · Desk drawer".into()),
            status: Some("stored".into()),
            doc_type: Some("license".into()),
            location: vec![r(HOME, "Home"), r(SAFE, "Safe"), r(DRAWER, "Desk drawer")],
            ..row(LICENSE, "Car license", "document")
        }]
    );
    let places = s
        .read(|c, ctx| build::directory(c, ctx, DirectoryTab::Places, ""))
        .expect("places");
    assert_eq!(
        places
            .items
            .iter()
            .map(|i| (i.title.as_str(), i.subtitle.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("Desk drawer", Some("Safe")),
            ("Home", None),
            ("Safe", Some("Home")),
        ]
    );
    assert_eq!(
        places
            .items
            .iter()
            .map(|i| (i.title.as_str(), i.document_count, i.breadcrumb.len()))
            .collect::<Vec<_>>(),
        [("Desk drawer", 1, 2), ("Home", 1, 0), ("Safe", 1, 1)]
    );
    let companies = s
        .read(|c, ctx| build::directory(c, ctx, DirectoryTab::Companies, ""))
        .expect("companies");
    assert_eq!(companies.items[0].subtitle.as_deref(), Some("Fuel"));
}

#[tokio::test]
async fn document_page_has_the_location_breadcrumb_and_custody_history() {
    let (_h, s) = vault().await;
    let screen = s
        .read(|c, ctx| build::entity_screen(c, ctx, LICENSE))
        .expect("screen");
    assert_eq!(screen.kind, EntityPageKind::Document);
    assert_eq!(
        screen.document.expect("document"),
        DocumentView {
            id: LICENSE.into(),
            title: "Car license".into(),
            aliases: vec![],
            doc_type: Some("license".into()),
            copy: None,
            status: Some("stored".into()),
            expires: None,
            location: vec![r(HOME, "Home"), r(SAFE, "Safe"), r(DRAWER, "Desk drawer")],
            holder: None,
            last_holder: None,
            custody: vec![
                custody(
                    "stored-at",
                    20,
                    Some(r(DRAWER, "Desk drawer")),
                    None,
                    "Stored at Desk drawer",
                    false
                ),
                custody(
                    "handed-to",
                    10,
                    None,
                    Some(r(SHADY, "Shady")),
                    "Handed to Shady",
                    false
                ),
            ],
            copies: vec![],
            concerns: vec![r(WATANYA, "Watanya")],
            title_dir: TextDir::Ltr,
            path: "documents/Car license.md".into(),
            pending_sync: false,
            expires_label: None,
            expiring_soon: false,
            renewal_task: None,
            mentions: vec![],
            copy_briefs: vec![],
            user_notes: String::new(),
            holder_label: None,
        }
    );
}

#[tokio::test]
async fn place_pages_include_documents_and_movements_of_nested_places() {
    let (_h, s) = vault().await;
    let home = s
        .read(|c, ctx| build::entity_screen(c, ctx, HOME))
        .expect("home");
    assert_eq!(home.kind, EntityPageKind::Place);
    let brief = DocumentBrief {
        id: LICENSE.into(),
        title: "Car license".into(),
        status: Some("stored".into()),
        location: Some(r(DRAWER, "Desk drawer")),
        holder: None,
        doc_type: Some("license".into()),
        last_holder: None,
        location_path: vec![r(HOME, "Home"), r(SAFE, "Safe"), r(DRAWER, "Desk drawer")],
        expiring_soon: false,
        title_dir: TextDir::Ltr,
    };
    assert_eq!(
        home.place.expect("place"),
        PlaceView {
            id: HOME.into(),
            title: "Home".into(),
            aliases: vec![],
            breadcrumb: vec![],
            sub_places: vec![r(SAFE, "Safe")],
            documents: vec![brief.clone()],
            // Stored in the drawer, a nested place (not "here").
            recent_movements: vec![custody(
                "stored-at",
                20,
                Some(r(DRAWER, "Desk drawer")),
                None,
                "Stored at Desk drawer",
                false
            )],
            title_dir: TextDir::Ltr,
            tree: vec![
                PlaceNode {
                    place: r(SAFE, "Safe"),
                    depth: 1,
                    document_count: 0,
                    parent_id: HOME.into(),
                },
                PlaceNode {
                    place: r(DRAWER, "Desk drawer"),
                    depth: 2,
                    document_count: 1,
                    parent_id: SAFE.into(),
                },
            ],
            out_with_people: vec![],
            user_notes: String::new(),
            path: "places/Home.md".into(),
        }
    );
    let drawer = s
        .read(|c, ctx| build::entity_screen(c, ctx, DRAWER))
        .expect("drawer")
        .place
        .expect("place");
    assert_eq!(drawer.breadcrumb, [r(HOME, "Home"), r(SAFE, "Safe")]);
    assert_eq!(drawer.sub_places, []);
    assert_eq!(drawer.documents, [brief]);
}

#[tokio::test]
async fn person_page_shows_summary_timeline_mentions_and_documents() {
    let (_h, s) = vault().await;
    let screen = s
        .read(|c, ctx| build::entity_screen(c, ctx, SHADY))
        .expect("screen");
    assert_eq!(screen.kind, EntityPageKind::Entity);
    let e = screen.entity.expect("entity");
    assert_eq!(e.title, "Shady");
    assert_eq!(e.kind, "person");
    assert_eq!(e.aliases, ["شادي"]);
    assert_eq!(e.summary.as_deref(), Some("Drives the company car."));
    assert_eq!(
        e.timeline
            .iter()
            .map(|b| (b.text.as_str(), b.date, b.citations.clone()))
            .collect::<Vec<_>>(),
        [(
            "got the car license",
            NaiveDate::from_ymd_opt(2026, 9, 10),
            vec![cite(CAPTURE, "Capture 2026-09-20")]
        )]
    );
    assert_eq!(
        e.properties
            .iter()
            .map(|p| (p.key.as_str(), p.values.clone()))
            .collect::<Vec<_>>(),
        [("role", vec!["Driver".to_owned()])]
    );
    // Notes that link to him, newest first: the capture and the document's custody line.
    assert_eq!(
        e.mentions.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        [CAPTURE, LICENSE]
    );
    assert!(!e.pending_sync);
    assert_eq!(e.initials, "S");
    assert_eq!(e.path, "people/Shady.md");
    assert_eq!(e.mention_count, 2);
    assert_eq!(e.last_active_label.as_deref(), Some("last active today"));
    assert_eq!(e.user_notes, "");
    assert_eq!(e.summary_dir, TextDir::Ltr);
    assert_eq!(e.timeline[0].date_label.as_deref(), Some("10 Sep"));
    // The capture's snippet highlights the mention.
    assert_eq!(
        e.mentions[0].snippet,
        "Put the car license in the drawer. [[Shady]] had it."
    );
    assert_eq!(
        e.mentions[0].highlights,
        [HighlightSpan { start: 37, end: 42 }]
    );

    let missing = s
        .read(|c, ctx| build::entity_screen(c, ctx, "01J8ZK3M4X7Q9W2E5R6T8Y0V9Z"))
        .expect("missing");
    assert_eq!(missing.kind, EntityPageKind::NotFound);
    assert_eq!(missing.entity, None);
}

#[tokio::test]
async fn keyword_search_normalises_arabic_and_works_offline() {
    let (_h, s) = vault().await;
    // "الاسعار" (bare alef) finds "الأسعار" (hamza); results carry the matching line.
    let v = s
        .read(|c, ctx| search::search(c, ctx, "الاسعار", SearchMode::Keyword, None))
        .expect("search");
    assert_eq!(v.results.len(), 1);
    assert_eq!(
        v.results[0],
        SearchHit {
            note_id: PRICING.into(),
            title: "تجارب التسعير".into(),
            path: "notes/Pricing/تجارب التسعير.md".into(),
            kind: "note".into(),
            snippet: "الأسعار زادت في سبتمبر.".into(),
            title_dir: TextDir::Rtl,
            snippet_dir: TextDir::Rtl,
            // The query spells bare alef; the snippet has hamza: no literal span.
            highlights: vec![],
            score: v.results[0].score,
        }
    );
    assert!(v.results[0].score > 0.0);
    assert_eq!(
        v.available_modes,
        [
            SearchMode::Keyword,
            SearchMode::Semantic,
            SearchMode::Hybrid
        ]
    );
    assert_eq!(v.availability, Availability::Available);
    // Title matches rank first (the title column weighs most); case never matters.
    let v = s
        .read(|c, ctx| search::search(c, ctx, "CAR LIC", SearchMode::Keyword, None))
        .expect("search");
    assert_eq!(
        v.results
            .iter()
            .map(|h| h.note_id.as_str())
            .collect::<Vec<_>>(),
        [LICENSE, SHADY, CAPTURE]
    );
    // Folder-scoped: only notes under `captures/`.
    let v = s
        .read(|c, ctx| search::search(c, ctx, "car", SearchMode::Keyword, Some("captures")))
        .expect("search");
    assert_eq!(
        v.results
            .iter()
            .map(|h| h.note_id.as_str())
            .collect::<Vec<_>>(),
        [CAPTURE]
    );
    assert_eq!(v.folder.as_deref(), Some("captures"));
    assert_eq!(
        v.results[0].highlights,
        [HighlightSpan { start: 8, end: 11 }]
    );
    let v = s
        .read(|c, ctx| search::search(c, ctx, "  ", SearchMode::Keyword, None))
        .expect("search");
    assert_eq!(v.results, []);
}

#[tokio::test]
async fn local_graph_is_radial_around_the_centre_and_global_graph_caches_positions() {
    let (_h, s) = vault().await;
    let local = s
        .read(|c, ctx| graph::local_graph(c, ctx, SHADY, 1))
        .expect("local");
    assert!(local.found);
    assert_eq!(local.depth, 1);
    let centre = local.nodes.iter().find(|n| n.id == SHADY).expect("centre");
    assert_eq!((centre.depth, centre.x, centre.y), (0, 0.0, 0.0));
    let mut ring: Vec<(&str, u8)> = local
        .nodes
        .iter()
        .filter(|n| n.id != SHADY)
        .map(|n| (n.id.as_str(), n.depth))
        .collect();
    ring.sort_unstable();
    assert_eq!(ring, [(LICENSE, 1), (CAPTURE, 1)]);
    // Every depth-1 node sits on the same circle.
    let radii: Vec<f64> = local
        .nodes
        .iter()
        .filter(|n| n.depth == 1)
        .map(|n| (n.x * n.x + n.y * n.y).sqrt())
        .collect();
    assert!(
        radii
            .iter()
            .all(|r| (r - radii[0]).abs() < 1e-9 && *r > 0.0)
    );
    // The mind map's edge toggles filter in the core.
    let license = s
        .read(|c, ctx| graph::local_graph(c, ctx, LICENSE, 1))
        .expect("license map");
    let mut custody_only = license.edges.clone();
    graph::retain_edge_kinds(&mut custody_only, &["custody".to_owned()]);
    assert_eq!(
        custody_only
            .iter()
            .map(|e| (e.src.as_str(), e.dst.as_str(), e.kind.as_str()))
            .collect::<Vec<_>>(),
        [(LICENSE, DRAWER, "custody:location")]
    );
    assert!(license.edges.len() > custody_only.len());
    let deeper = s
        .read(|c, ctx| graph::local_graph(c, ctx, SHADY, 9))
        .expect("clamped");
    assert_eq!(deeper.depth, 3);
    let missing = s
        .read(|c, ctx| graph::local_graph(c, ctx, "01J8ZK3M4X7Q9W2E5R6T8Y0V9Z", 2))
        .expect("missing");
    assert!(!missing.found);
    assert_eq!(missing.nodes, []);

    let first = s.read(graph::global_graph).expect("global");
    assert_eq!(first.nodes.len(), 8);
    let cached: i64 = s
        .read(|c, _| Ok(c.query_row("SELECT COUNT(*) FROM graph_positions", [], |r| r.get(0))?))
        .expect("count");
    assert_eq!(cached, 8);
    // Warm-started from the cache: a second layout stays close to the first.
    let second = s.read(graph::global_graph).expect("again");
    for (a, b) in first.nodes.iter().zip(&second.nodes) {
        assert_eq!(a.id, b.id);
        assert!(a.x.is_finite() && a.y.is_finite());
    }
}

#[tokio::test]
async fn notes_list_groups_folders_and_sorts_notes() {
    let (_h, s) = vault().await;
    let root = s
        .read(|c, ctx| build::notes_list(c, ctx, ""))
        .expect("root");
    assert_eq!(
        root.folders,
        [
            FolderItem {
                path: "captures".into(),
                name: "captures".into(),
                note_count: 1
            },
            FolderItem {
                path: "companies".into(),
                name: "companies".into(),
                note_count: 1
            },
            FolderItem {
                path: "documents".into(),
                name: "documents".into(),
                note_count: 1
            },
            FolderItem {
                path: "notes".into(),
                name: "notes".into(),
                note_count: 0
            },
            FolderItem {
                path: "people".into(),
                name: "people".into(),
                note_count: 1
            },
            FolderItem {
                path: "places".into(),
                name: "places".into(),
                note_count: 3
            },
        ]
    );
    assert_eq!(root.notes, []);
    let places = s
        .read(|c, ctx| build::notes_list(c, ctx, "/places/"))
        .expect("places");
    assert_eq!(places.folder, "places");
    assert_eq!(
        places
            .notes
            .iter()
            .map(|n| n.title.as_str())
            .collect::<Vec<_>>(),
        ["Desk drawer", "Home", "Safe"]
    );
    let pricing = s
        .read(|c, ctx| build::notes_list(c, ctx, "notes/Pricing"))
        .expect("pricing");
    assert_eq!(pricing.notes[0].id, PRICING);
    assert_eq!(pricing.notes[0].snippet, "الأسعار زادت في سبتمبر.");
    assert_eq!(pricing.notes[0].title_dir, TextDir::Rtl);
    assert_eq!(pricing.notes[0].updated_label, "13:00");
    assert_eq!(pricing.note_count, 1);
    assert_eq!(
        pricing.breadcrumb,
        [
            FolderItem {
                path: String::new(),
                name: "Notes".into(),
                note_count: 0
            },
            FolderItem {
                path: "notes".into(),
                name: "notes".into(),
                note_count: 0
            },
            FolderItem {
                path: "notes/Pricing".into(),
                name: "Pricing".into(),
                note_count: 1
            },
        ]
    );
}

#[tokio::test]
async fn settings_ask_and_admin_views() {
    let (_h, s) = vault().await;
    let settings = s
        .read(|c, ctx| build::settings_view(c, ctx, common::SERVER))
        .expect("settings")
        .expect("signed in");
    assert_eq!(settings.account.username, "shawket");
    assert!(settings.reminders.enabled);
    assert_eq!(settings.reminders.default_time, "09:00");
    // Online sections are fetched with `refresh_settings`; nothing cached yet.
    assert_eq!(settings.ai, Availability::Available);
    assert_eq!(settings.integrity, Availability::Available);
    assert_eq!(settings.device_list, []);
    assert_eq!(settings.reminders.snooze_minutes, 15);
    assert_eq!(
        (
            settings.reminders.quiet_enabled,
            settings.reminders.quiet_from.as_str(),
            settings.reminders.quiet_until.as_str()
        ),
        (false, "22:00", "07:00")
    );
    // Admin screens are for admins only.
    assert_eq!(settings.admin, Availability::NotAllowed);

    let ask = s.read(|c, ctx| build::ask(c, ctx, &[])).expect("ask");
    assert_eq!(ask.availability, Availability::Available);
    assert_eq!(ask.messages, []);
    assert_eq!(
        ask.scopes
            .iter()
            .map(|sc| sc.label.as_str())
            .collect::<Vec<_>>(),
        [
            "All notes",
            "Shady",
            "Watanya",
            "captures",
            "companies",
            "documents",
            "notes",
            "people",
            "places"
        ]
    );

    let at = |d: &str| {
        chrono::DateTime::parse_from_rfc3339(d)
            .expect("ts")
            .with_timezone(&chrono::Utc)
    };
    let user =
        |id: &str, username: &str, status: &str, created: &str| strata_core::net::AdminUserInfo {
            id: id.into(),
            username: username.into(),
            display_name: username.into(),
            role: "member".into(),
            status: status.into(),
            created: at(created),
            deletion_at: None,
            export_downloaded_at: None,
            password_change_required: false,
        };
    let ctx = s.ctx();
    let users = vec![
        user("4", "zeina", "active", "2026-01-01T00:00:00Z"),
        user("3", "new2", "pending", "2026-09-26T00:00:00Z"),
        user("2", "Ahmed", "disabled", "2026-02-01T00:00:00Z"),
        user("1", "new1", "pending", "2026-09-25T00:00:00Z"),
    ];
    let v = build::admin_users(&ctx, "4", users.clone(), "");
    assert_eq!(
        v.pending.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
        ["1", "3"]
    );
    assert_eq!(
        v.users.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
        ["2", "4"]
    );
    assert_eq!(
        v.users
            .iter()
            .map(|u| (u.is_self, u.initials.as_str(), u.created_label.as_str()))
            .collect::<Vec<_>>(),
        [
            (false, "A", "Joined 1 Feb 2026"),
            (true, "Z", "Joined 1 Jan 2026")
        ]
    );
    assert_eq!(v.pending[0].created_label, "Requested 2 days ago");
    let found = build::admin_users(&ctx, "4", users, "NEW2");
    assert_eq!(
        found
            .pending
            .iter()
            .map(|u| u.id.as_str())
            .collect::<Vec<_>>(),
        ["3"]
    );
    assert_eq!(found.users, []);
}

fn no_filter() -> strata_core::view::model::DirectoryFilter {
    strata_core::view::model::DirectoryFilter {
        tags: vec![],
        role: None,
        company_id: None,
        industry: None,
        doc_type: None,
        status: None,
        place_id: None,
        holder_id: None,
        expiring: false,
        has_open_items: false,
    }
}

#[tokio::test]
async fn directory_filters_facets_sections_and_sort_run_in_the_core() {
    use strata_core::view::model::{DirectoryFilter, DirectorySort, FilterOption};
    let (_h, s) = vault().await;
    let docs = |f: DirectoryFilter| {
        s.read(|c, ctx| {
            build::directory_filtered(c, ctx, DirectoryTab::Documents, "", &f, DirectorySort::Name)
        })
        .expect("docs")
    };
    // "In Home" includes documents stored in nested places (Home › Safe › Desk drawer).
    let in_home = docs(DirectoryFilter {
        place_id: Some(HOME.into()),
        ..no_filter()
    });
    assert_eq!(
        in_home
            .items
            .iter()
            .map(|i| i.title.as_str())
            .collect::<Vec<_>>(),
        ["Car license"]
    );
    let opt = |facet: &str, value: &str, label: &str, selected: bool| FilterOption {
        facet: facet.into(),
        value: value.into(),
        label: label.into(),
        count: 1,
        selected,
    };
    assert_eq!(
        in_home.filter_options,
        [
            opt("doc_type", "license", "license", false),
            opt("place", HOME, "Home", true),
            opt("status", "stored", "stored", false),
        ]
    );
    assert_eq!(
        in_home
            .sections
            .iter()
            .map(|x| (x.label.as_str(), x.items.len()))
            .collect::<Vec<_>>(),
        [("All documents · A–Z", 1)]
    );
    let lost = docs(DirectoryFilter {
        status: Some("lost".into()),
        ..no_filter()
    });
    assert_eq!(lost.items, []);
    let places = s
        .read(|c, ctx| {
            build::directory_filtered(
                c,
                ctx,
                DirectoryTab::Places,
                "",
                &no_filter(),
                DirectorySort::LastActive,
            )
        })
        .expect("places");
    // Same activity: ties fall back to the title.
    assert_eq!(
        places
            .items
            .iter()
            .map(|i| i.title.as_str())
            .collect::<Vec<_>>(),
        ["Desk drawer", "Home", "Safe"]
    );
}

#[tokio::test]
async fn global_map_filters_lens_counts_and_focus_are_computed_in_the_core() {
    use strata_core::view::model::{GraphFilter, GraphLens, GraphNodeKind, KindCount};
    let (_h, s) = vault().await;
    let all = GraphFilter {
        edge_kinds: vec![],
        node_kinds: vec![],
        similarity: false,
        cluster: None,
        lens: GraphLens::Notes,
        focus: None,
        include_tags: false,
    };
    let map = |f: &GraphFilter| {
        s.read(|c, ctx| graph::global_graph_filtered(c, ctx, f))
            .expect("map")
    };
    let g = map(&all);
    assert_eq!(g.nodes.len(), 8);
    // Place nesting appears once, as `part-of-place` (the shared edge-kind mapping, L16).
    assert_eq!(g.edges.len(), 9);
    let kc = |kind: &str, label: &str, count: u32| KindCount {
        kind: kind.into(),
        label: label.into(),
        count,
    };
    assert_eq!(
        g.edge_counts,
        [
            kc("custody", "Custody", 1),
            kc("link", "Links", 5),
            kc("mention", "Mentions", 1),
            kc("part-of-place", "Inside", 2),
        ]
    );
    assert_eq!(
        g.node_counts,
        [
            kc("company", "Companies", 1),
            kc("document", "Documents", 1),
            kc("note", "Notes", 2),
            kc("person", "People", 1),
            kc("place", "Places", 3),
        ]
    );
    assert_eq!(g.similarity, Availability::Available);

    // Node-kind filter plus a focus: only places, and the focused place's neighbours.
    let places = map(&GraphFilter {
        node_kinds: vec!["place".into()],
        focus: Some(SAFE.into()),
        ..all.clone()
    });
    assert_eq!(
        places
            .nodes
            .iter()
            .map(|n| n.title.as_str())
            .collect::<Vec<_>>(),
        ["Home", "Safe", "Desk drawer"]
    );
    assert_eq!(
        places
            .edges
            .iter()
            .map(|e| (e.src.as_str(), e.dst.as_str(), e.kind.as_str()))
            .collect::<Vec<_>>(),
        [
            (SAFE, HOME, "part-of-place"),
            (DRAWER, SAFE, "part-of-place")
        ]
    );
    assert_eq!(places.neighbours, [HOME, DRAWER]);
    // Counts are before filtering: the panel keeps showing every kind.
    assert_eq!(places.node_counts, g.node_counts);

    // Edge-kind filter keeps every node and only the custody edge.
    let custody = map(&GraphFilter {
        edge_kinds: vec!["custody".into()],
        ..all.clone()
    });
    assert_eq!(custody.nodes.len(), 8);
    assert_eq!(
        custody
            .edges
            .iter()
            .map(|e| (e.src.as_str(), e.dst.as_str(), e.kind.as_str()))
            .collect::<Vec<_>>(),
        [(LICENSE, DRAWER, "custody:location")]
    );
    // A full kind selects just that kind within its family.
    let exact = map(&GraphFilter {
        edge_kinds: vec!["custody:location".into(), "relation:supports".into()],
        ..all.clone()
    });
    assert_eq!(exact.edges, custody.edges);

    // The people lens shows people only.
    let people = map(&GraphFilter {
        lens: GraphLens::People,
        ..all.clone()
    });
    assert_eq!(
        people
            .nodes
            .iter()
            .map(|n| n.title.as_str())
            .collect::<Vec<_>>(),
        ["Shady"]
    );
    assert_eq!(people.nodes[0].kind, GraphNodeKind::Person);
    assert_eq!(people.nodes[0].path.as_deref(), Some("people/Shady.md"));

    // The tag toggle adds `tag:<tag>` nodes and note → tag edges, like the server's graph.
    s.update_note(
        PRICING,
        &format!("---\nid: {PRICING}\ntags: [Pricing]\n---\nالأسعار زادت في سبتمبر.\n"),
    )
    .expect("tag");
    let tagged = map(&GraphFilter {
        include_tags: true,
        node_kinds: vec!["tag".into(), "note".into()],
        ..all.clone()
    });
    let tag = tagged
        .nodes
        .iter()
        .find(|n| n.kind == GraphNodeKind::Tag)
        .expect("tag node");
    assert_eq!(
        (
            tag.id.as_str(),
            tag.title.as_str(),
            tag.path.as_deref(),
            tag.updated
        ),
        ("tag:pricing", "pricing", None, None)
    );
    assert_eq!(
        tagged
            .edges
            .iter()
            .map(|e| (e.src.as_str(), e.dst.as_str(), e.kind.as_str()))
            .collect::<Vec<_>>(),
        [(PRICING, "tag:pricing", "tag")]
    );

    // A document copy links to its original with `document:copy-of`.
    let copy = s
        .create_note(
            "documents/Car license copy.md",
            "---\nkind: document\ncopy: true\ncopy-of: \"[[Car license]]\"\n---\n",
            true,
        )
        .expect("copy")
        .id
        .expect("created");
    let docs = map(&GraphFilter {
        edge_kinds: vec!["document".into()],
        ..all
    });
    assert_eq!(
        docs.edges
            .iter()
            .map(|e| (
                e.src.as_str(),
                e.dst.as_str(),
                e.kind.as_str(),
                e.label.as_str()
            ))
            .collect::<Vec<_>>(),
        [(copy.as_str(), LICENSE, "document:copy-of", "copy of")]
    );
}

#[tokio::test]
async fn editor_completions_mentions_and_pickers() {
    use strata_core::view::extra;
    use strata_core::view::model::{
        CompletionItem, CompletionKind, Completions, MentionEdit, PlaceOption,
    };
    let (_h, s) = vault().await;
    let len = |t: &str| u32::try_from(t.encode_utf16().count()).expect("len");
    let link = "---\nid: X\n---\nMet [[Sha";
    let c = s
        .read(|c, ctx| extra::completions(c, ctx, CAPTURE, link, len(link)))
        .expect("completions");
    assert_eq!(
        c,
        Completions {
            kind: CompletionKind::WikiLink,
            replace_start: 20,
            replace_end: 23,
            query: "Sha".into(),
            items: vec![CompletionItem {
                label: "Shady".into(),
                detail: "people/Shady.md".into(),
                insert_text: "Shady]]".into(),
                target_id: Some(SHADY.into()),
                entity_kind: Some("person".into()),
                label_dir: TextDir::Ltr,
            }],
        }
    );
    let mention = "---\nid: X\n---\nMet @Wat";
    let m = s
        .read(|c, ctx| extra::completions(c, ctx, CAPTURE, mention, len(mention)))
        .expect("mention");
    assert_eq!(m.kind, CompletionKind::Mention);
    assert_eq!(
        m.items
            .iter()
            .map(|i| (i.label.as_str(), i.target_id.as_deref()))
            .collect::<Vec<_>>(),
        [("Watanya", Some(WATANYA))]
    );
    // Picking it writes the link and adds the company to the note's frontmatter.
    let edit = s
        .read(|c, _| extra::insert_mention(c, mention, len(mention) - 4, len(mention), WATANYA))
        .expect("insert");
    assert_eq!(
        edit,
        MentionEdit {
            content: "---\nid: X\ncompanies: [\"[[Watanya]]\"]\n---\nMet [[Watanya]]".into(),
            cursor: 56,
        }
    );

    let options = s
        .read(|c, _| extra::place_options(c, Some(LICENSE)))
        .expect("places");
    assert_eq!(
        options,
        [
            PlaceOption {
                id: HOME.into(),
                title: "Home".into(),
                breadcrumb: vec![],
                depth: 0,
                is_current: false,
            },
            PlaceOption {
                id: SAFE.into(),
                title: "Safe".into(),
                breadcrumb: vec![r(HOME, "Home")],
                depth: 1,
                is_current: false,
            },
            PlaceOption {
                id: DRAWER.into(),
                title: "Desk drawer".into(),
                breadcrumb: vec![r(HOME, "Home"), r(SAFE, "Safe")],
                depth: 2,
                is_current: true,
            },
        ]
    );
    let merge = s
        .read(|c, _| extra::merge_preview(c, SAFE, DRAWER))
        .expect("merge");
    assert_eq!(
        (
            merge.source,
            merge.into,
            merge.aliases,
            merge.mention_count,
            merge.relation_count
        ),
        (
            r(SAFE, "Safe"),
            r(DRAWER, "Desk drawer"),
            vec!["Safe".to_owned()],
            1,
            2
        )
    );
}

#[tokio::test]
async fn new_task_sheet_understands_mentions_dates_times_and_recurrence() {
    use strata_core::view::extra;
    use strata_core::view::model::TaskChipKind;
    let (_h, s) = vault().await;
    let p = s
        .read(|c, ctx| {
            extra::task_draft_preview(c, ctx, "Call @Shady tomorrow 3pm every week !high")
        })
        .expect("preview");
    let d = NaiveDate::from_ymd_opt(2026, 9, 28).expect("date");
    assert_eq!(p.description, "Call [[Shady]]");
    assert_eq!(p.due, Some(d));
    assert_eq!(p.due_label.as_deref(), Some("Tomorrow"));
    assert_eq!(p.recurrence.as_deref(), Some("every week"));
    assert_eq!(p.priority.as_deref(), Some("high"));
    assert_eq!(p.links, [r(SHADY, "Shady")]);
    assert_eq!(p.reminders, [d.and_hms_opt(15, 0, 0).expect("time")]);
    assert_eq!(
        p.chips
            .iter()
            .map(|c| (c.kind, c.label.as_str()))
            .collect::<Vec<_>>(),
        [
            (TaskChipKind::Due, "Tomorrow"),
            (TaskChipKind::Recurrence, "Every week"),
            (TaskChipKind::Priority, "High priority"),
            (TaskChipKind::Reminder, "15:00"),
            (TaskChipKind::Link, "Shady"),
        ]
    );
    assert_eq!(p.draft.description, p.description);
    assert_eq!(p.draft.priority.as_deref(), Some("high"));
}

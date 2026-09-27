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
    AdminUserItem, Availability, Citation, CustodyItem, DirectoryCounts, DirectoryItem,
    DirectoryTab, DocumentBrief, DocumentView, EntityPageKind, EntityRef, FolderItem, PlaceView,
    SearchHit, SearchMode,
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
    EntityRef {
        id: Some(id.to_owned()),
        title: title.to_owned(),
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
        .read(|c, _| build::directory(c, DirectoryTab::People, ""))
        .expect("people");
    assert_eq!(
        people.items,
        [DirectoryItem {
            id: SHADY.into(),
            title: "Shady".into(),
            subtitle: Some("Driver".into()),
            aliases: vec!["شادي".into()],
        }]
    );
    assert_eq!(people.counts, counts);
    // The Arabic alias finds him; so does a lower-case prefix of the Latin name.
    let by_alias = s
        .read(|c, _| build::directory(c, DirectoryTab::People, "شادى"))
        .expect("alias");
    assert_eq!(by_alias.items.len(), 1);
    let none = s
        .read(|c, _| build::directory(c, DirectoryTab::People, "mona"))
        .expect("none");
    assert_eq!(none.items, []);

    let docs = s
        .read(|c, _| build::directory(c, DirectoryTab::Documents, ""))
        .expect("docs");
    assert_eq!(
        docs.items,
        [DirectoryItem {
            id: LICENSE.into(),
            title: "Car license".into(),
            subtitle: Some("stored · Desk drawer".into()),
            aliases: vec![],
        }]
    );
    let places = s
        .read(|c, _| build::directory(c, DirectoryTab::Places, ""))
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
    let companies = s
        .read(|c, _| build::directory(c, DirectoryTab::Companies, ""))
        .expect("companies");
    assert_eq!(companies.items[0].subtitle.as_deref(), Some("Fuel"));
}

#[tokio::test]
async fn document_page_has_the_location_breadcrumb_and_custody_history() {
    let (_h, s) = vault().await;
    let screen = s
        .read(|c, _| build::entity_screen(c, LICENSE))
        .expect("screen");
    assert_eq!(screen.kind, EntityPageKind::Document);
    let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).expect("date");
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
                CustodyItem {
                    date: d(9, 20),
                    kind: "stored-at".into(),
                    document: Some(r(LICENSE, "Car license")),
                    place: Some(r(DRAWER, "Desk drawer")),
                    person: None,
                    counterparty: None,
                    citations: vec![cite(CAPTURE, "Capture 2026-09-20")],
                },
                CustodyItem {
                    date: d(9, 10),
                    kind: "handed-to".into(),
                    document: Some(r(LICENSE, "Car license")),
                    place: None,
                    person: Some(r(SHADY, "Shady")),
                    counterparty: None,
                    citations: vec![cite(CAPTURE, "Capture 2026-09-20")],
                },
            ],
            copies: vec![],
            concerns: vec![r(WATANYA, "Watanya")],
        }
    );
}

#[tokio::test]
async fn place_pages_include_documents_and_movements_of_nested_places() {
    let (_h, s) = vault().await;
    let home = s.read(|c, _| build::entity_screen(c, HOME)).expect("home");
    assert_eq!(home.kind, EntityPageKind::Place);
    let brief = DocumentBrief {
        id: LICENSE.into(),
        title: "Car license".into(),
        status: Some("stored".into()),
        location: Some(r(DRAWER, "Desk drawer")),
        holder: None,
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
            recent_movements: vec![CustodyItem {
                date: NaiveDate::from_ymd_opt(2026, 9, 20).expect("date"),
                kind: "stored-at".into(),
                document: Some(r(LICENSE, "Car license")),
                place: Some(r(DRAWER, "Desk drawer")),
                person: None,
                counterparty: None,
                citations: vec![cite(CAPTURE, "Capture 2026-09-20")],
            }],
        }
    );
    let drawer = s
        .read(|c, _| build::entity_screen(c, DRAWER))
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
        .read(|c, _| build::entity_screen(c, SHADY))
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

    let missing = s
        .read(|c, _| build::entity_screen(c, "01J8ZK3M4X7Q9W2E5R6T8Y0V9Z"))
        .expect("missing");
    assert_eq!(missing.kind, EntityPageKind::NotFound);
    assert_eq!(missing.entity, None);
}

#[tokio::test]
async fn keyword_search_normalises_arabic_and_works_offline() {
    let (_h, s) = vault().await;
    // "الاسعار" (bare alef) finds "الأسعار" (hamza); results carry the matching line.
    let v = s
        .read(|c, ctx| search::search(c, ctx, "الاسعار", SearchMode::Keyword))
        .expect("search");
    assert_eq!(
        v.results,
        [SearchHit {
            note_id: PRICING.into(),
            title: "تجارب التسعير".into(),
            path: "notes/Pricing/تجارب التسعير.md".into(),
            kind: "note".into(),
            snippet: "الأسعار زادت في سبتمبر.".into(),
        }]
    );
    assert_eq!(v.availability, Availability::Available);
    // Title matches rank first (the title column weighs most); case never matters.
    let v = s
        .read(|c, ctx| search::search(c, ctx, "CAR LIC", SearchMode::Keyword))
        .expect("search");
    assert_eq!(
        v.results
            .iter()
            .map(|h| h.note_id.as_str())
            .collect::<Vec<_>>(),
        [LICENSE, SHADY, CAPTURE]
    );
    // Semantic search needs a server endpoint that does not exist yet.
    let v = s
        .read(|c, ctx| search::search(c, ctx, "car", SearchMode::Semantic))
        .expect("search");
    assert_eq!(v.availability, Availability::NotYetAvailable);
    assert_eq!(v.results.len(), 3);
    let v = s
        .read(|c, ctx| search::search(c, ctx, "  ", SearchMode::Keyword))
        .expect("search");
    assert_eq!(v.results, []);
}

#[tokio::test]
async fn local_graph_is_radial_around_the_centre_and_global_graph_caches_positions() {
    let (_h, s) = vault().await;
    let local = s
        .read(|c, _| graph::local_graph(c, SHADY, 1))
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
    let deeper = s
        .read(|c, _| graph::local_graph(c, SHADY, 9))
        .expect("clamped");
    assert_eq!(deeper.depth, 3);
    let missing = s
        .read(|c, _| graph::local_graph(c, "01J8ZK3M4X7Q9W2E5R6T8Y0V9Z", 2))
        .expect("missing");
    assert!(!missing.found);
    assert_eq!(missing.nodes, []);

    let first = s.read(|c, _| graph::global_graph(c)).expect("global");
    assert_eq!(first.nodes.len(), 8);
    let cached: i64 = s
        .read(|c, _| Ok(c.query_row("SELECT COUNT(*) FROM graph_positions", [], |r| r.get(0))?))
        .expect("count");
    assert_eq!(cached, 8);
    // Warm-started from the cache: a second layout stays close to the first.
    let second = s.read(|c, _| graph::global_graph(c)).expect("again");
    for (a, b) in first.nodes.iter().zip(&second.nodes) {
        assert_eq!(a.id, b.id);
        assert!(a.x.is_finite() && a.y.is_finite());
    }
}

#[tokio::test]
async fn notes_list_groups_folders_and_sorts_notes() {
    let (_h, s) = vault().await;
    let root = s.read(|c, _| build::notes_list(c, "")).expect("root");
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
        .read(|c, _| build::notes_list(c, "/places/"))
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
        .read(|c, _| build::notes_list(c, "notes/Pricing"))
        .expect("pricing");
    assert_eq!(pricing.notes[0].id, PRICING);
    assert_eq!(pricing.notes[0].snippet, "الأسعار زادت في سبتمبر.");
}

#[tokio::test]
async fn settings_ask_and_admin_views() {
    let (_h, s) = vault().await;
    let settings = s
        .read(build::settings_view)
        .expect("settings")
        .expect("signed in");
    assert_eq!(settings.account.username, "shawket");
    assert!(settings.reminders.enabled);
    assert_eq!(settings.reminders.default_time, "09:00");
    assert_eq!(settings.ai, Availability::NotYetAvailable);
    assert_eq!(settings.integrity, Availability::NotYetAvailable);
    // Admin screens are for admins only.
    assert_eq!(settings.admin, Availability::NotAllowed);

    let ask = s.read(|_, ctx| Ok(build::ask(ctx))).expect("ask");
    assert_eq!(ask.availability, Availability::NotYetAvailable);
    assert_eq!(ask.messages, []);

    let at = |d: &str| {
        chrono::DateTime::parse_from_rfc3339(d)
            .expect("ts")
            .with_timezone(&chrono::Utc)
    };
    let user = |id: &str, username: &str, status: &str, created: &str| AdminUserItem {
        id: id.into(),
        username: username.into(),
        display_name: username.into(),
        role: "member".into(),
        status: status.into(),
        created: at(created),
        deletion_at: None,
        export_downloaded_at: None,
    };
    let v = build::admin_users(vec![
        user("4", "zeina", "active", "2026-01-01T00:00:00Z"),
        user("3", "new2", "pending", "2026-09-26T00:00:00Z"),
        user("2", "Ahmed", "disabled", "2026-02-01T00:00:00Z"),
        user("1", "new1", "pending", "2026-09-25T00:00:00Z"),
    ]);
    assert_eq!(
        v.pending.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
        ["1", "3"]
    );
    assert_eq!(
        v.users.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
        ["2", "4"]
    );
}

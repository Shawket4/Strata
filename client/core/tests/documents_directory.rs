//! The Documents tab of the directory (PLAN §11 screen 5, §6.12): filter chips with counts,
//! filters by holder, place (nested places included), type, status and expiry, the
//! "recently moved" order, and the expiry and holder labels of each row. Against the fake
//! server, with documents whose custody fields the server has written.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::session::Session;
use strata_core::sync::engine::Trigger;
use strata_core::view::build;
use strata_core::view::model::{
    DirectoryFilter, DirectorySort, DirectoryTab, DirectoryView, DocumentBrief, EntityRef,
    FilterOption, TextDir,
};

const MONA: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VP1";
const SAMIR: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VP2";
const SAFE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VS1";
const DRAWER: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VS2";
const LICENSE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VD1";
const LEASE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VD2";
const PASSPORT: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VD3";

async fn world() -> (Harness, std::sync::Arc<Session>) {
    let h = Harness::new();
    let up = |id: &str, path: &str, fm: &str, body: &str| {
        h.server
            .remote_upsert(id, path, &format!("---\nid: {id}\n{fm}---\n{body}"));
    };
    up(MONA, "people/Mona Adel.md", "kind: person\n", "");
    up(SAMIR, "people/Samir.md", "kind: person\n", "");
    up(SAFE, "places/Safe.md", "kind: place\n", "");
    up(
        DRAWER,
        "places/Desk drawer.md",
        "kind: place\npart-of: \"[[Safe]]\"\n",
        "",
    );
    up(
        LICENSE,
        "documents/Car license.md",
        "kind: document\ndoc-type: license\nstatus: stored\nlocation: \"[[Desk drawer]]\"\n\
         last-holder: \"[[Mona Adel]]\"\nexpires: 2026-10-10\n",
        "## Custody\n\n- 2026-09-20 — stored-at [[Desk drawer]]\n\
         - 2026-09-10 — handed-to [[Mona Adel]]\n",
    );
    up(
        LEASE,
        "documents/Lease.md",
        "kind: document\ndoc-type: contract\nstatus: checked-out\nholder: \"[[Samir]]\"\n\
         last-holder: \"[[Samir]]\"\nexpires: 2026-01-01\n",
        "## Custody\n\n- 2026-09-25 — handed-to [[Samir]]\n",
    );
    up(
        PASSPORT,
        "documents/Passport.md",
        "kind: document\ndoc-type: id\nstatus: stored\nlast-holder: \"[[Mona Adel]]\"\n\
         expires: 2031-05-01\n",
        "## Custody\n\n- 2026-09-01 — returned-by [[Mona Adel]]\n",
    );
    h.server.set_page_size(50);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    (h, s)
}

fn docs(s: &Session, filter: &DirectoryFilter, sort: DirectorySort) -> DirectoryView {
    s.read(|c, ctx| build::directory_filtered(c, ctx, DirectoryTab::Documents, "", filter, sort))
        .expect("directory")
}

fn titles(v: &DirectoryView) -> Vec<&str> {
    v.items.iter().map(|i| i.title.as_str()).collect()
}

fn opt(facet: &str, value: &str, label: &str, count: u32, selected: bool) -> FilterOption {
    FilterOption {
        facet: facet.into(),
        value: value.into(),
        label: label.into(),
        count,
        selected,
    }
}

#[tokio::test]
async fn rows_carry_holder_and_expiry_labels() {
    let (_h, s) = world().await;
    let all = docs(&s, &DirectoryFilter::default(), DirectorySort::Name);
    assert_eq!(titles(&all), ["Car license", "Lease", "Passport"]);
    assert_eq!(
        all.items
            .iter()
            .map(|i| (
                i.holder_label.as_deref(),
                i.expires_label.as_deref(),
                i.expiring_soon
            ))
            .collect::<Vec<_>>(),
        [
            (
                Some("Last with Mona Adel · 20 Sep"),
                Some("Expires 10 Oct 2026"),
                true
            ),
            (Some("With Samir"), Some("Expired 1 Jan 2026"), true),
            (
                Some("Last with Mona Adel · 1 Sep"),
                Some("Expires 1 May 2031"),
                false
            ),
        ]
    );
    assert_eq!(all.expiring_count, 2);
    assert_eq!(
        all.sections
            .iter()
            .map(|s| (s.label.as_str(), s.items.len()))
            .collect::<Vec<_>>(),
        [("All documents · A–Z", 3)]
    );
}

#[tokio::test]
async fn filters_combine_with_facet_counts_and_nested_places() {
    let (_h, s) = world().await;
    let all = docs(&s, &DirectoryFilter::default(), DirectorySort::Name);
    assert_eq!(
        all.filter_options,
        [
            opt("doc_type", "contract", "contract", 1, false),
            opt("doc_type", "id", "id", 1, false),
            opt("doc_type", "license", "license", 1, false),
            opt("expiring", "", "Expiring", 2, false),
            opt("holder", SAMIR, "Samir", 1, false),
            opt("place", SAFE, "Safe", 1, false),
            opt("status", "checked-out", "checked-out", 1, false),
            opt("status", "stored", "stored", 2, false),
        ]
    );

    // Everything inside the safe (the drawer is part of it).
    let in_safe = DirectoryFilter {
        place_id: Some(SAFE.into()),
        ..DirectoryFilter::default()
    };
    let v = docs(&s, &in_safe, DirectorySort::Name);
    assert_eq!(titles(&v), ["Car license"]);
    assert!(
        v.filter_options
            .iter()
            .any(|o| o.facet == "place" && o.selected)
    );
    let in_drawer = DirectoryFilter {
        place_id: Some(DRAWER.into()),
        ..DirectoryFilter::default()
    };
    assert_eq!(
        titles(&docs(&s, &in_drawer, DirectorySort::Name)),
        ["Car license"]
    );

    let with_samir = DirectoryFilter {
        holder_id: Some(SAMIR.into()),
        ..DirectoryFilter::default()
    };
    assert_eq!(
        titles(&docs(&s, &with_samir, DirectorySort::Name)),
        ["Lease"]
    );

    let expiring_stored = DirectoryFilter {
        expiring: true,
        status: Some("STORED".into()),
        ..DirectoryFilter::default()
    };
    let v = docs(&s, &expiring_stored, DirectorySort::Name);
    assert_eq!(titles(&v), ["Car license"]);
    assert_eq!(
        v.filter_options
            .iter()
            .filter(|o| o.selected)
            .map(|o| o.facet.as_str())
            .collect::<Vec<_>>(),
        ["expiring"],
        "status matching ignores case but the chip is selected only on an exact value"
    );
    let contracts = DirectoryFilter {
        doc_type: Some("contract".into()),
        ..DirectoryFilter::default()
    };
    assert_eq!(
        titles(&docs(&s, &contracts, DirectorySort::Name)),
        ["Lease"]
    );
}

#[tokio::test]
async fn recently_moved_orders_by_the_newest_custody_event() {
    let (_h, s) = world().await;
    let v = docs(
        &s,
        &DirectoryFilter::default(),
        DirectorySort::RecentlyMoved,
    );
    assert_eq!(titles(&v), ["Lease", "Car license", "Passport"]);
    assert_eq!(
        v.sections
            .iter()
            .map(|s| (s.label.as_str(), s.items.len()))
            .collect::<Vec<_>>(),
        [("Results", 3)]
    );
}

const COPY: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VD4";
const ERRANDS: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VN1";

fn place(id: &str, title: &str) -> EntityRef {
    EntityRef {
        id: Some(id.into()),
        title: title.into(),
        kind: Some("place".into()),
    }
}

#[tokio::test]
async fn a_document_page_lists_its_copies_and_the_renewal_task() {
    let (h, s) = world().await;
    h.server.remote_upsert(
        COPY,
        "documents/Car license copy.md",
        &format!(
            "---\nid: {COPY}\nkind: document\ndoc-type: license\ncopy: copy\n\
             copy-of: [\"[[Car license]]\"]\nstatus: stored\nlocation: \"[[Safe]]\"\n---\n"
        ),
    );
    h.server.remote_upsert(
        ERRANDS,
        "notes/Errands.md",
        &format!("---\nid: {ERRANDS}\n---\n- [ ] Renew the [[Car license]] 📅 2026-10-05 ^r1\n"),
    );
    s.pull().await.expect("pull");

    let page = |id: &str| {
        s.read(|c, ctx| build::entity_screen(c, ctx, id))
            .expect("screen")
            .document
            .expect("document")
    };
    let license = page(LICENSE);
    assert_eq!(
        license.copies,
        [EntityRef {
            id: Some(COPY.into()),
            title: "Car license copy".into(),
            kind: Some("document".into()),
        }]
    );
    assert_eq!(
        license.copy_briefs,
        [DocumentBrief {
            id: COPY.into(),
            title: "Car license copy".into(),
            status: Some("stored".into()),
            location: Some(place(SAFE, "Safe")),
            holder: None,
            doc_type: Some("license".into()),
            last_holder: None,
            location_path: vec![place(SAFE, "Safe")],
            expiring_soon: false,
            title_dir: TextDir::Ltr,
        }]
    );
    let renewal = license.renewal_task.expect("renewal task");
    assert_eq!(
        (
            renewal.id.as_str(),
            renewal.description.as_str(),
            renewal.origin_label.as_deref()
        ),
        (
            "r1",
            "Renew the [[Car license]]",
            Some("From document expiry")
        )
    );
    assert_eq!(
        (
            license.holder_label.as_deref(),
            license.expires_label.as_deref(),
            license.expiring_soon
        ),
        (
            Some("Last with Mona Adel · 20 Sep"),
            Some("Expires 10 Oct 2026"),
            true
        )
    );
    // The copy points back at its original.
    let copy = page(COPY);
    assert_eq!(
        (
            copy.copy.as_deref(),
            copy.copies
                .iter()
                .map(|c| c.id.as_deref())
                .collect::<Vec<_>>(),
            copy.renewal_task
        ),
        (Some("copy"), vec![Some(LICENSE)], None)
    );
}

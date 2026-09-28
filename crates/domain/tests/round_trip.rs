//! Exhaustive string round-trip tests for every domain enum: the exact spelling of every
//! variant, `Display`/`FromStr`, JSON and `MessagePack` (serde) round trips, and exact parse
//! errors.

use std::fmt::{Debug, Display};
use std::str::FromStr;

use domain::{
    AccountStatus, CopyKind, CustodyEdge, CustodyEventType, DedupeKind, DocType,
    DocumentRelationType, DocumentStatus, EntityRelationType, GraphEdgeKind, GraphNodeKind, Lang,
    MatchLevel, MentionType, NoteKind, ParseError, Priority, RelationOrigin, RelationType, Role,
    TaskStatus,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Checks that `expected` lists every variant (in `all` order) with its exact string, and that
/// every representation round-trips.
fn check<T>(what: &'static str, all: &[T], expected: &[(T, &str)]) -> TestResult
where
    T: Copy + Eq + Debug + Display + FromStr<Err = ParseError> + Serialize + DeserializeOwned,
{
    let listed: Vec<T> = expected.iter().map(|(v, _)| *v).collect();
    assert_eq!(
        listed, all,
        "{what}: expected table must list every variant in order"
    );

    for (value, s) in expected {
        assert_eq!(value.to_string(), *s);
        assert_eq!(s.parse::<T>(), Ok(*value));

        let json = serde_json::to_string(value)?;
        assert_eq!(json, format!("\"{s}\""));
        assert_eq!(serde_json::from_str::<T>(&json)?, *value);

        let bytes = rmp_serde::to_vec_named(value)?;
        assert_eq!(
            bytes,
            rmp_serde::to_vec_named(s)?,
            "{what}: msgpack is a plain str"
        );
        assert_eq!(rmp_serde::from_slice::<T>(&bytes)?, *value);
    }

    for bad in ["", "unknown", " x"] {
        assert_eq!(
            bad.parse::<T>(),
            Err(ParseError {
                what,
                value: bad.to_owned()
            })
        );
    }
    let (_, first) = expected[0];
    let upper = first.to_uppercase();
    assert!(
        upper.parse::<T>().is_err(),
        "{what}: parsing is case-sensitive"
    );
    let padded = format!("{first} ");
    assert!(
        padded.parse::<T>().is_err(),
        "{what}: parsing does not trim"
    );

    let err = serde_json::from_str::<T>("\"nope\"")
        .err()
        .map(|e| e.to_string());
    assert!(
        err.as_deref()
            .is_some_and(|e| e.starts_with("unknown variant `nope`")),
        "{what}: {err:?}"
    );
    Ok(())
}

#[test]
fn relation_type() -> TestResult {
    use RelationType::*;
    check(
        "relation type",
        RelationType::ALL,
        &[
            (Related, "related"),
            (PartOf, "part-of"),
            (Supports, "supports"),
            (Contradicts, "contradicts"),
            (FollowsUp, "follows-up"),
            (Duplicates, "duplicates"),
        ],
    )
}

#[test]
fn entity_relation_type() -> TestResult {
    use EntityRelationType::*;
    check(
        "entity relation type",
        EntityRelationType::ALL,
        &[
            (WorksAt, "works-at"),
            (WorkedAt, "worked-at"),
            (ReportsTo, "reports-to"),
            (Knows, "knows"),
            (IntroducedBy, "introduced-by"),
            (ClientOf, "client-of"),
            (SupplierOf, "supplier-of"),
            (PartnerOf, "partner-of"),
            (CompetitorOf, "competitor-of"),
            (SubsidiaryOf, "subsidiary-of"),
        ],
    )
}

#[test]
fn relation_origin() -> TestResult {
    check(
        "relation origin",
        RelationOrigin::ALL,
        &[(RelationOrigin::User, "user"), (RelationOrigin::Ai, "ai")],
    )
}

#[test]
fn note_kind() -> TestResult {
    use NoteKind::*;
    check(
        "note kind",
        NoteKind::ALL,
        &[
            (Note, "note"),
            (Concept, "concept"),
            (Person, "person"),
            (Company, "company"),
            (Document, "document"),
            (Place, "place"),
        ],
    )
}

#[test]
fn lang() -> TestResult {
    check(
        "language",
        Lang::ALL,
        &[(Lang::Ar, "ar"), (Lang::En, "en"), (Lang::Mixed, "mixed")],
    )
}

#[test]
fn custody_event_type() -> TestResult {
    use CustodyEventType::*;
    check(
        "custody event type",
        CustodyEventType::ALL,
        &[
            (StoredAt, "stored-at"),
            (MovedTo, "moved-to"),
            (HandedTo, "handed-to"),
            (ReturnedBy, "returned-by"),
            (SentTo, "sent-to"),
            (ReceivedFrom, "received-from"),
            (Lost, "lost"),
            (Found, "found"),
            (Destroyed, "destroyed"),
        ],
    )
}

#[test]
fn document_status() -> TestResult {
    use DocumentStatus::*;
    check(
        "document status",
        DocumentStatus::ALL,
        &[
            (Stored, "stored"),
            (CheckedOut, "checked-out"),
            (WithThirdParty, "with-third-party"),
            (Lost, "lost"),
            (Destroyed, "destroyed"),
        ],
    )
}

#[test]
fn copy_kind() -> TestResult {
    use CopyKind::*;
    check(
        "copy kind",
        CopyKind::ALL,
        &[
            (Original, "original"),
            (CertifiedCopy, "certified copy"),
            (Copy, "copy"),
            (Digital, "digital"),
        ],
    )?;
    assert_eq!("certified-copy".parse::<CopyKind>(), Ok(CertifiedCopy));
    assert_eq!(
        serde_json::from_str::<CopyKind>("\"certified-copy\"")?,
        CertifiedCopy
    );
    assert_eq!(serde_json::to_string(&CertifiedCopy)?, "\"certified copy\"");
    Ok(())
}

#[test]
fn doc_type() -> TestResult {
    use DocType::*;
    check(
        "document type",
        DocType::ALL,
        &[
            (Contract, "contract"),
            (Id, "id"),
            (Licence, "licence"),
            (Deed, "deed"),
            (Invoice, "invoice"),
            (Certificate, "certificate"),
            (Other, "other"),
        ],
    )
}

#[test]
fn document_relation_type() -> TestResult {
    check(
        "document relation type",
        DocumentRelationType::ALL,
        &[(DocumentRelationType::CopyOf, "copy-of")],
    )
}

#[test]
fn mention_type() -> TestResult {
    use MentionType::*;
    check(
        "mention type",
        MentionType::ALL,
        &[
            (Concepts, "concepts"),
            (People, "people"),
            (Companies, "companies"),
        ],
    )
}

#[test]
fn task_status() -> TestResult {
    use TaskStatus::*;
    check(
        "task status",
        TaskStatus::ALL,
        &[(Open, "open"), (Done, "done"), (Cancelled, "cancelled")],
    )
}

#[test]
fn priority() -> TestResult {
    use Priority::*;
    check(
        "priority",
        Priority::ALL,
        &[
            (Highest, "highest"),
            (High, "high"),
            (Medium, "medium"),
            (Normal, "normal"),
            (Low, "low"),
            (Lowest, "lowest"),
        ],
    )
}

#[test]
fn role() -> TestResult {
    check(
        "role",
        Role::ALL,
        &[(Role::Admin, "admin"), (Role::Member, "member")],
    )
}

#[test]
fn account_status() -> TestResult {
    use AccountStatus::*;
    check(
        "account status",
        AccountStatus::ALL,
        &[
            (Pending, "pending"),
            (Active, "active"),
            (Disabled, "disabled"),
            (Rejected, "rejected"),
            (DeletionPending, "deletion_pending"),
        ],
    )
}

#[test]
fn dedupe_kind() -> TestResult {
    use DedupeKind::*;
    check(
        "dedupe kind",
        DedupeKind::ALL,
        &[
            (Note, "note"),
            (Capture, "capture"),
            (Task, "task"),
            (Person, "person"),
            (Company, "company"),
            (Concept, "concept"),
            (Alias, "alias"),
            (Document, "document"),
            (Place, "place"),
        ],
    )
}

#[test]
fn match_level() -> TestResult {
    use MatchLevel::*;
    check(
        "match level",
        MatchLevel::ALL,
        &[(Exact, "exact"), (Near, "near"), (Semantic, "semantic")],
    )
}

#[test]
fn graph_node_kind() -> TestResult {
    use GraphNodeKind::*;
    check(
        "graph node kind",
        GraphNodeKind::ALL,
        &[
            (Note, "note"),
            (Concept, "concept"),
            (Person, "person"),
            (Company, "company"),
            (Document, "document"),
            (Place, "place"),
            (Attachment, "attachment"),
            (Tag, "tag"),
            (Cluster, "cluster"),
        ],
    )
}

#[test]
fn custody_edge() -> TestResult {
    use CustodyEdge::*;
    check(
        "custody edge",
        CustodyEdge::ALL,
        &[
            (Location, "location"),
            (Holder, "holder"),
            (LastHolder, "last-holder"),
        ],
    )
}

#[test]
fn graph_edge_kind() -> TestResult {
    use GraphEdgeKind::*;
    let expected: Vec<(GraphEdgeKind, &str)> = vec![
        (Link, "link"),
        (Embed, "embed"),
        (Relation(RelationType::Related), "relation:related"),
        (Relation(RelationType::PartOf), "relation:part-of"),
        (Relation(RelationType::Supports), "relation:supports"),
        (Relation(RelationType::Contradicts), "relation:contradicts"),
        (Relation(RelationType::FollowsUp), "relation:follows-up"),
        (Relation(RelationType::Duplicates), "relation:duplicates"),
        (Similarity, "similarity"),
        (Concept, "concept"),
        (Mention, "mention"),
        (Entity(EntityRelationType::WorksAt), "entity:works-at"),
        (Entity(EntityRelationType::WorkedAt), "entity:worked-at"),
        (Entity(EntityRelationType::ReportsTo), "entity:reports-to"),
        (Entity(EntityRelationType::Knows), "entity:knows"),
        (
            Entity(EntityRelationType::IntroducedBy),
            "entity:introduced-by",
        ),
        (Entity(EntityRelationType::ClientOf), "entity:client-of"),
        (Entity(EntityRelationType::SupplierOf), "entity:supplier-of"),
        (Entity(EntityRelationType::PartnerOf), "entity:partner-of"),
        (
            Entity(EntityRelationType::CompetitorOf),
            "entity:competitor-of",
        ),
        (
            Entity(EntityRelationType::SubsidiaryOf),
            "entity:subsidiary-of",
        ),
        (Custody(CustodyEdge::Location), "custody:location"),
        (Custody(CustodyEdge::Holder), "custody:holder"),
        (Custody(CustodyEdge::LastHolder), "custody:last-holder"),
        (PartOfPlace, "part-of-place"),
        (Document(DocumentRelationType::CopyOf), "document:copy-of"),
        (Tag, "tag"),
    ];
    let all = GraphEdgeKind::all();
    assert_eq!(all.len(), 27);
    for (value, s) in &expected {
        assert_eq!(value.to_string(), *s);
        assert_eq!(s.parse::<GraphEdgeKind>(), Ok(*value));
        let json = serde_json::to_string(value)?;
        assert_eq!(json, format!("\"{s}\""));
        assert_eq!(serde_json::from_str::<GraphEdgeKind>(&json)?, *value);
        let bytes = rmp_serde::to_vec_named(value)?;
        assert_eq!(bytes, rmp_serde::to_vec_named(s)?);
        assert_eq!(rmp_serde::from_slice::<GraphEdgeKind>(&bytes)?, *value);
    }
    let listed: Vec<GraphEdgeKind> = expected.iter().map(|(v, _)| *v).collect();
    assert_eq!(listed, all);

    for bad in [
        "",
        "relation",
        "relation:",
        "relation:works-at",
        "entity:related",
        "custody:place",
        "similar:related",
        "Link",
        "part-of",
        "copy-of",
        "document:related",
        "tag:x",
    ] {
        assert_eq!(
            bad.parse::<GraphEdgeKind>(),
            Err(ParseError {
                what: "graph edge kind",
                value: bad.to_owned()
            })
        );
    }
    let err = serde_json::from_str::<GraphEdgeKind>("\"relation:nope\"")
        .err()
        .map(|e| e.to_string());
    assert_eq!(
        err.as_deref(),
        Some("unknown graph edge kind: \"relation:nope\" at line 1 column 15")
    );
    Ok(())
}

#[test]
fn parse_error_message() {
    assert_eq!(
        "Related"
            .parse::<RelationType>()
            .err()
            .map(|e| e.to_string())
            .as_deref(),
        Some("unknown relation type: \"Related\"")
    );
}

//! Shared harness for strata-graph integration tests: a fresh database, a temp data root with
//! provisioned vaults, the fake LLM provider behind a real `AiService`, and the fixture vault
//! of the graph tests.
#![allow(dead_code, clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::BTreeMap;
use std::sync::Arc;

use strata_ai::{AiService, BudgetGuard, BudgetLimits, MemoryUsageStore, ProviderRouter};
use strata_common::config::AiProviderKind;
use strata_common::{NoteId, UserId};
use strata_graph::GraphService;
use strata_graph::assemble::{EdgeView, GraphView, NodeId};
use strata_graph::cluster::{ClusterConfig, ClusterHandler, RecordedClusterEvents};
use strata_graph::similarity::SimilaritySource;
use strata_index::UserScope;
use strata_testkit::{FakeLlmProvider, TempDataRoot, TestDb, TestUser};
use strata_vault::ops::notes::CreateNote;
use strata_vault::ops::relations::AiEdge;
use strata_vault::{VaultConfig, VaultService};
use vault_format::RelationKey;

/// One test's world.
pub struct World {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub vault: VaultService,
    pub llm: FakeLlmProvider,
    pub ai: Arc<AiService>,
    pub events: Arc<RecordedClusterEvents>,
}

impl World {
    pub async fn new() -> Self {
        let db = TestDb::new().await.expect("db");
        let data = TempDataRoot::new().expect("data root");
        let vault = VaultService::new(
            VaultConfig::new(data.path()),
            db.app_db.clone(),
            Arc::new(db.clock.clone()),
            db.ids.clone(),
        );
        let llm = FakeLlmProvider::new().named("claude_cli");
        let per_user: BTreeMap<String, AiProviderKind> =
            [("noai".to_owned(), AiProviderKind::Disabled)].into();
        let router = ProviderRouter::new(AiProviderKind::ClaudeCli, per_user)
            .with_provider(AiProviderKind::ClaudeCli, Arc::new(llm.clone()));
        let budget = BudgetGuard::new(
            BudgetLimits::default(),
            chrono_tz::UTC,
            Arc::new(db.clock.clone()),
            Arc::new(MemoryUsageStore::default()),
        );
        let ai = Arc::new(AiService::new(router, budget));
        Self {
            db,
            data,
            vault,
            llm,
            ai,
            events: Arc::new(RecordedClusterEvents::default()),
        }
    }

    /// A user with a provisioned vault.
    pub async fn user(&self, name: &str) -> (UserId, UserScope) {
        let user = TestUser::new(name).create(&self.db).await.expect("user").id;
        assert!(self.vault.provision(user).await.expect("provision"));
        (user, self.db.scope(user))
    }

    pub fn service(&self, similarity: Option<Arc<dyn SimilaritySource>>) -> GraphService {
        GraphService::new(self.db.app_db.clone(), self.vault.clone(), similarity)
    }

    pub fn clusterer(&self) -> ClusterHandler {
        self.clusterer_with(ClusterConfig::default())
    }

    pub fn clusterer_with(&self, config: ClusterConfig) -> ClusterHandler {
        ClusterHandler::new(
            self.db.app_db.clone(),
            self.vault.clone(),
            self.ai.clone(),
            self.events.clone(),
            config,
        )
    }

    pub async fn create(&self, scope: &UserScope, path: &str, content: &str) -> NoteId {
        self.vault
            .create_note(
                scope,
                CreateNote {
                    created: strata_common::clock::default_test_epoch(),
                    path: path.to_owned(),
                    content: content.to_owned(),
                    id: None,
                    force: true,
                },
            )
            .await
            .expect("create note")
            .id
    }

    pub fn dir(&self, user: UserId) -> std::path::PathBuf {
        self.vault.vault_dir(user)
    }

    pub fn read(&self, user: UserId, rel: &str) -> String {
        std::fs::read_to_string(self.dir(user).join(rel)).expect("read vault file")
    }

    pub fn write_raw(&self, user: UserId, rel: &str, text: &str) {
        let p = self.dir(user).join(rel);
        std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        std::fs::write(p, text).expect("write");
    }

    /// Commit messages, newest first.
    pub fn log(&self, user: UserId) -> Vec<String> {
        strata_vault::git::log(&self.dir(user))
            .expect("log")
            .into_iter()
            .map(|c| c.message)
            .collect()
    }

    /// Paths changed by the newest commit.
    pub fn last_commit_paths(&self, user: UserId) -> Vec<String> {
        let head = strata_vault::git::head(&self.dir(user))
            .expect("head")
            .expect("a commit");
        strata_vault::git::changed_paths(&self.dir(user), &head.id).expect("paths")
    }

    pub async fn finish(self) {
        drop(self.vault);
        self.db.cleanup().await.expect("cleanup");
    }
}

/// The fixture vault's notes.
#[derive(Debug, Clone, Copy)]
pub struct Fixture {
    pub shady: NoteId,
    pub mona: NoteId,
    pub omar: NoteId,
    pub watanya: NoteId,
    pub acme: NoteId,
    pub office: NoteId,
    pub safe: NoteId,
    pub contract: NoteId,
    pub pricing: NoteId,
    pub budget: NoteId,
    pub plan: NoteId,
    pub call: NoteId,
    pub meeting: NoteId,
    pub lonely: NoteId,
}

/// Builds the fixture vault (PLAN §6.7, §6.12, §10 node and edge kinds):
///
/// - people Shady, Mona (works-at Watanya, knows Shady), Omar; companies Watanya, Acme;
/// - places Nasr City office and Safe (part-of the office);
/// - document "Watanya contract" (location Safe, last holder Shady, companies Watanya);
/// - concept Pricing;
/// - notes Budget; Plan (AI `contradicts` Budget, 0.72); Call (mentions Shady, Mona,
///   Watanya; concept Pricing; related Plan; links and embeds Plan); Meeting (mentions
///   Shady, Mona, Omar); Lonely (no edges).
pub async fn fixture(w: &World, s: &UserScope) -> Fixture {
    let shady = w
        .create(s, "people/Shady.md", "---\nkind: person\n---\n")
        .await;
    let omar = w
        .create(s, "people/Omar.md", "---\nkind: person\n---\n")
        .await;
    let watanya = w
        .create(s, "companies/Watanya.md", "---\nkind: company\n---\n")
        .await;
    let acme = w
        .create(s, "companies/Acme.md", "---\nkind: company\n---\n")
        .await;
    let mona = w
        .create(
            s,
            "people/Mona.md",
            "---\nkind: person\nworks-at: [\"[[Watanya]]\"]\nknows: [\"[[Shady]]\"]\n---\n",
        )
        .await;
    let office = w
        .create(s, "places/Nasr City office.md", "---\nkind: place\n---\n")
        .await;
    let safe = w
        .create(
            s,
            "places/Safe.md",
            "---\nkind: place\npart-of: [\"[[Nasr City office]]\"]\n---\n",
        )
        .await;
    let contract = w
        .create(
            s,
            "documents/Watanya contract.md",
            "---\nkind: document\ndoc-type: contract\ncompanies: [\"[[Watanya]]\"]\nlocation: \"[[Safe]]\"\nlast-holder: \"[[Shady]]\"\n---\n",
        )
        .await;
    let pricing = w
        .create(
            s,
            "concepts/Pricing.md",
            "---\nkind: concept\n---\n## Summary\n",
        )
        .await;
    let budget = w
        .create(s, "notes/Budget.md", "Budget caps discounts at 5%.\n")
        .await;
    let plan = w.create(s, "notes/Plan.md", "A flat 10% discount.\n").await;
    w.vault
        .ai_add_relations(
            s,
            "link".into(),
            plan,
            vec![AiEdge {
                rel: RelationKey::Note(domain::RelationType::Contradicts),
                dst: budget,
                confidence: 0.72,
                reason: "10% flat vs a 5% cap".into(),
                model: "fake/fake-model".into(),
            }],
        )
        .await
        .expect("ai edge");
    let call = w
        .create(
            s,
            "notes/Call.md",
            "---\npeople: [\"[[Shady]]\", \"[[Mona]]\"]\ncompanies: [\"[[Watanya]]\"]\nconcepts: [\"[[Pricing]]\"]\nrelated: [\"[[Plan]]\"]\n---\nSee [[Plan]] and [[Plan]] again.\n\n![[Plan]]\n",
        )
        .await;
    let meeting = w
        .create(
            s,
            "notes/Meeting.md",
            "---\npeople: [\"[[Shady]]\", \"[[Mona]]\", \"[[Omar]]\"]\n---\nWeekly.\n",
        )
        .await;
    let lonely = w
        .create(s, "notes/Lonely.md", "Nothing links here.\n")
        .await;
    Fixture {
        shady,
        mona,
        omar,
        watanya,
        acme,
        office,
        safe,
        contract,
        pricing,
        budget,
        plan,
        call,
        meeting,
        lonely,
    }
}

/// Titles by ID, for readable assertions (tag nodes as `#tag`).
pub fn titles(view: &GraphView) -> BTreeMap<NodeId, String> {
    view.nodes
        .iter()
        .map(|n| {
            let title = match n.id {
                NodeId::Note(_) => n.title.clone(),
                NodeId::Tag(_) => format!("#{}", n.title),
            };
            (n.id.clone(), title)
        })
        .collect()
}

/// `(title, degree)` of every node, in response order (tag nodes as `#tag`).
pub fn node_degrees(view: &GraphView) -> Vec<(String, u32)> {
    let t = titles(view);
    view.nodes
        .iter()
        .map(|n| (t[&n.id].clone(), n.degree))
        .collect()
}

/// `(source title, target title, kind)` of every edge, in response order.
pub fn edge_triples(view: &GraphView) -> Vec<(String, String, String)> {
    let t = titles(view);
    view.edges
        .iter()
        .map(|e| {
            (
                t[&e.source].clone(),
                t[&e.target].clone(),
                e.kind.to_string(),
            )
        })
        .collect()
}

/// The edge `source → target` of `kind`.
pub fn edge<'a>(view: &'a GraphView, source: NoteId, target: NoteId, kind: &str) -> &'a EdgeView {
    view.edges
        .iter()
        .find(|e| e.source == source && e.target == target && e.kind.to_string() == kind)
        .unwrap_or_else(|| panic!("no {kind} edge {source} -> {target}"))
}

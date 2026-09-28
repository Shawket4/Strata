//! The graph component behind the API (PLAN §7.5 Graph, §9.2 `cluster`, §10): the
//! [`GraphApi`] app data registered by the composition root, the mapping of graph errors to
//! problem details, and [`BusClusterEvents`], which publishes the `cluster` job's
//! `cluster.updated` on the caller's event stream.

use std::sync::Arc;

use strata_common::config::RateLimit;
use strata_common::{Clock, IdGenerator, UserId};
use strata_graph::cluster::ClusterEvents;
use strata_graph::{GraphError, GraphService};

use crate::auth::rate_limit::RateLimiter;
use crate::events::{Event, EventBus};
use crate::wire::{Problem, ProblemFieldError, ProblemType};

/// Manual re-clusterings allowed per user and window (`POST /graph/recluster`).
pub const RECLUSTER_LIMIT: RateLimit = RateLimit {
    max: 3,
    window_secs: 3600,
};

/// What the graph and map endpoints need (`web::Data<GraphApi>`).
#[derive(Debug)]
pub struct GraphApi {
    /// Graph reads.
    pub graph: GraphService,
    /// Clock (job timestamps).
    pub clock: Arc<dyn Clock>,
    /// Job IDs.
    pub ids: Arc<dyn IdGenerator>,
    /// `POST /graph/recluster` limiter, per user.
    pub recluster: RateLimiter,
}

impl GraphApi {
    /// The API's graph features with the default recluster limit.
    pub fn new(graph: GraphService, clock: Arc<dyn Clock>, ids: Arc<dyn IdGenerator>) -> Self {
        Self::with_limit(graph, clock, ids, RECLUSTER_LIMIT)
    }

    /// With an explicit recluster limit.
    pub fn with_limit(
        graph: GraphService,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
        limit: RateLimit,
    ) -> Self {
        Self {
            graph,
            recluster: RateLimiter::new(limit, clock.clone()),
            clock,
            ids,
        }
    }
}

/// Publishes `cluster.updated` on the user's `/events` stream.
#[derive(Debug, Clone)]
pub struct BusClusterEvents(pub Arc<EventBus>);

impl ClusterEvents for BusClusterEvents {
    fn clusters_updated(&self, user: UserId, cluster_ids: &[String]) {
        self.0.publish(
            user,
            [Event::ClusterUpdated {
                cluster_ids: cluster_ids.to_vec(),
            }],
        );
    }
}

/// The problem for a graph error.
pub fn problem(err: &GraphError) -> Problem {
    match err {
        GraphError::NotFound => Problem::new(ProblemType::NotFound),
        GraphError::InvalidParameter {
            name,
            code,
            message,
        } => crate::vault::invalid_parameter(name, code, message),
        GraphError::InvalidField {
            pointer,
            code,
            message,
        } => Problem::new(ProblemType::InvalidBody)
            .with_detail(message.clone())
            .with_error(ProblemFieldError {
                code: (*code).to_owned(),
                pointer: Some((*pointer).to_owned()),
                message: message.clone(),
            }),
        GraphError::InvalidMap(issues) => issues.iter().fold(
            Problem::new(ProblemType::InvalidBody)
                .with_detail("the map is not a valid canvas for this vault"),
            |p, i| {
                p.with_error(ProblemFieldError {
                    code: i.code.to_owned(),
                    pointer: Some("/content".to_owned()),
                    message: i.message.clone(),
                })
            },
        ),
        GraphError::Vault(e) => crate::vault::problem(e),
        GraphError::Index(e) => Problem::internal(e),
        GraphError::Similarity(e) => Problem::internal(e),
    }
}

/// Converts graph results into handler results.
pub trait OrGraphProblem<T> {
    /// Maps the error to problem details.
    fn or_graph_problem(self) -> Result<T, Problem>;
}

impl<T> OrGraphProblem<T> for Result<T, GraphError> {
    fn or_graph_problem(self) -> Result<T, Problem> {
        self.map_err(|e| problem(&e))
    }
}

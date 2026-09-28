//! Job notices for the per-user event stream (`job.started` for interactive jobs,
//! `job.completed`, `job.failed`, PLAN §7.5 Events). The runner only knows this trait; the composition root connects it to the API's
//! event bus.

use std::sync::{Mutex, PoisonError};

use strata_common::{JobId, NoteId, UserId};

/// An interactive job started (`job.started`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobStart {
    /// Job ID.
    pub id: JobId,
    /// Kind.
    pub kind: String,
    /// The note it works on.
    pub note_id: Option<NoteId>,
}

/// How a job ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    /// It completed.
    Completed,
    /// It failed for good (attempts exhausted or a permanent error).
    Failed,
}

/// One finished job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobNotice {
    /// Job ID.
    pub id: JobId,
    /// Kind.
    pub kind: String,
    /// The note it worked on.
    pub note_id: Option<NoteId>,
    /// Outcome.
    pub outcome: JobOutcome,
}

/// Receives job notices. Called on the job's task after its row was updated; must not block.
pub trait JobEvents: Send + Sync {
    /// A job of `user` finished.
    fn job_finished(&self, user: UserId, notice: &JobNotice);

    /// An interactive job of `user` started.
    fn job_started(&self, _user: UserId, _start: &JobStart) {}
}

/// Discards notices.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoEvents;

impl JobEvents for NoEvents {
    fn job_finished(&self, _user: UserId, _notice: &JobNotice) {}
}

/// Records notices (tests and diagnostics).
#[derive(Debug, Default)]
pub struct RecordedEvents {
    notices: Mutex<Vec<(UserId, JobNotice)>>,
    starts: Mutex<Vec<(UserId, JobStart)>>,
}

impl RecordedEvents {
    /// Every notice so far, in order.
    pub fn notices(&self) -> Vec<(UserId, JobNotice)> {
        self.notices
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Every interactive start so far, in order.
    pub fn starts(&self) -> Vec<(UserId, JobStart)> {
        self.starts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl JobEvents for RecordedEvents {
    fn job_started(&self, user: UserId, start: &JobStart) {
        self.starts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((user, start.clone()));
    }

    fn job_finished(&self, user: UserId, notice: &JobNotice) {
        self.notices
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((user, notice.clone()));
    }
}

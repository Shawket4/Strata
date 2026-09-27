//! The in-memory revocation set (PLAN §8: "every request checks the session id against an
//! in-memory revocation set kept current from the database").
//!
//! Access tokens are verified without a database round trip, so revocation has to be known
//! in memory. The set holds:
//!
//! - **revoked sessions** (logout, device removal, refresh-token reuse, disable, deletion
//!   scheduling, admin actions). An entry is kept at least one access-token lifetime after it
//!   was last seen, which is as long as any token of that session can still verify;
//! - **user flags** for users whose every token must be refused or restricted:
//!   `disabled`, `deletion_pending` (export-only), `must_change_password` (admin reset);
//! - **purge tombstones**: a purged user has no rows left to reload from, so the tombstone
//!   outlives every token issued before the purge.
//!
//! **Consistency (single `stratad` process).** The process that changes an account updates
//! the set synchronously, in the same request, so the change holds from the next request.
//! Blocking changes are applied to the set *before* the database write (fail closed); lifting
//! a block happens after the write commits. A periodic [`RevocationSet::reload`] re-reads
//! the database. A local change made while a reload is in flight wins over that reload's
//! snapshot (each change bumps a version; the reload only overwrites users whose last local
//! change is older than the reload's start), so a reload can never resurrect access that a
//! concurrent request just revoked. With several processes, a change made by one would reach
//! the others only at their next reload; Strata runs one `stratad` (see
//! `docs/ARCHITECTURE.md`, "Auth").

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use strata_common::{Clock, SessionId, UserId};
use strata_index::AccountsDb;

use crate::auth::password::TEMPORARY_PREFIX;

/// Account-level restrictions of one user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UserFlags {
    /// Disabled by an admin: every token is refused.
    pub disabled: bool,
    /// Scheduled for deletion: tokens are export-only.
    pub deletion_pending: bool,
    /// Temporary password from an admin reset: only `GET/PATCH /me` and logout.
    pub must_change_password: bool,
}

impl UserFlags {
    fn is_default(self) -> bool {
        self == Self::default()
    }
}

/// Why a token is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Revoked {
    /// The session was revoked (or its device removed).
    Session,
    /// The user is disabled.
    UserDisabled,
    /// The user was purged.
    UserPurged,
}

#[derive(Debug, Default)]
struct State {
    /// Revoked sessions → keep-until.
    sessions: HashMap<SessionId, DateTime<Utc>>,
    /// Users with non-default flags.
    users: HashMap<UserId, UserFlags>,
    /// Version of each user's last local change.
    local: HashMap<UserId, u64>,
    /// Purged users → keep-until.
    purged: HashMap<UserId, DateTime<Utc>>,
    version: u64,
}

/// The revocation set. Cheap to share (`Arc`).
#[derive(Debug)]
pub struct RevocationSet {
    state: RwLock<State>,
    /// How long a revocation must be remembered: the access-token lifetime plus skew.
    retention: Duration,
    clock: Arc<dyn Clock>,
}

/// Token skew tolerated on top of the access-token lifetime.
const RETENTION_SKEW: Duration = Duration::seconds(120);

impl RevocationSet {
    /// An empty set for access tokens living `access_ttl`.
    pub fn new(access_ttl: Duration, clock: Arc<dyn Clock>) -> Self {
        Self {
            state: RwLock::new(State::default()),
            retention: access_ttl + RETENTION_SKEW,
            clock,
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.state.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.state.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Checks a token's session and user; returns the user's restrictions if allowed.
    pub fn check(&self, session: SessionId, user: UserId) -> Result<UserFlags, Revoked> {
        let state = self.read();
        if state.purged.contains_key(&user) {
            return Err(Revoked::UserPurged);
        }
        let flags = state.users.get(&user).copied().unwrap_or_default();
        if flags.disabled {
            return Err(Revoked::UserDisabled);
        }
        if state.sessions.contains_key(&session) {
            return Err(Revoked::Session);
        }
        Ok(flags)
    }

    /// The flags currently known for `user`.
    pub fn flags(&self, user: UserId) -> UserFlags {
        self.read().users.get(&user).copied().unwrap_or_default()
    }

    /// Records revoked sessions (kept for at least one token lifetime from now).
    pub fn revoke_sessions(&self, sessions: impl IntoIterator<Item = SessionId>) {
        let until = self.clock.now() + self.retention;
        let mut state = self.write();
        for id in sessions {
            let entry = state.sessions.entry(id).or_insert(until);
            *entry = (*entry).max(until);
        }
    }

    /// Replaces `user`'s flags (a local change: it survives an in-flight reload).
    pub fn set_user(&self, user: UserId, flags: UserFlags) {
        let mut state = self.write();
        state.version += 1;
        let version = state.version;
        state.local.insert(user, version);
        if flags.is_default() {
            state.users.remove(&user);
        } else {
            state.users.insert(user, flags);
        }
    }

    /// Updates `user`'s flags with `change`.
    pub fn update_user(&self, user: UserId, change: impl FnOnce(&mut UserFlags)) {
        let mut flags = self.flags(user);
        change(&mut flags);
        self.set_user(user, flags);
    }

    /// Records that `user` was purged: every token of theirs is refused from now on.
    pub fn mark_purged(&self, user: UserId) {
        let until = self.clock.now() + self.retention;
        let mut state = self.write();
        state.purged.insert(user, until);
        state.version += 1;
        let version = state.version;
        state.local.insert(user, version);
        state.users.remove(&user);
    }

    /// The version a reload must pass to [`Self::apply_reload`] (read before querying).
    pub fn begin_reload(&self) -> u64 {
        self.read().version
    }

    /// Applies a database snapshot taken after [`Self::begin_reload`] returned `since`:
    /// revoked sessions are merged in; user flags are replaced except for users changed
    /// locally after `since`; expired entries are dropped.
    pub fn apply_reload(&self, since: u64, sessions: &[SessionId], users: &[(UserId, UserFlags)]) {
        let now = self.clock.now();
        let until = now + self.retention;
        let mut state = self.write();
        for id in sessions {
            let entry = state.sessions.entry(*id).or_insert(until);
            *entry = (*entry).max(until);
        }
        state.sessions.retain(|_, keep| *keep > now);
        state.purged.retain(|_, keep| *keep > now);

        let fresh: HashMap<UserId, UserFlags> = users
            .iter()
            .copied()
            .filter(|(_, f)| !f.is_default())
            .collect();
        let State { users, local, .. } = &mut *state;
        let changed_locally = |u: &UserId| local.get(u).is_some_and(|v| *v > since);
        users.retain(|u, _| changed_locally(u) || fresh.contains_key(u));
        for (user, flags) in fresh {
            if !changed_locally(&user) {
                users.insert(user, flags);
            }
        }
        local.retain(|_, v| *v > since);
    }

    /// Reloads from the database (`strata_accounts` pool): revoked unexpired sessions and
    /// users that are disabled, pending deletion or on a temporary password.
    pub async fn reload(&self, accounts: &AccountsDb, pool: &PgPool) -> strata_index::Result<()> {
        let since = self.begin_reload();
        let sessions = accounts.live_revocations(self.clock.now()).await?;
        let rows: Vec<(UserId, String, bool)> = sqlx::query_as(
            "SELECT id, status, starts_with(password_hash, $1) FROM users \
             WHERE status IN ('disabled', 'deletion_pending') OR starts_with(password_hash, $1)",
        )
        .bind(TEMPORARY_PREFIX)
        .fetch_all(pool)
        .await?;
        let users: Vec<(UserId, UserFlags)> = rows
            .into_iter()
            .map(|(id, status, temporary)| {
                (
                    id,
                    UserFlags {
                        disabled: status == "disabled",
                        deletion_pending: status == "deletion_pending",
                        must_change_password: temporary,
                    },
                )
            })
            .collect();
        self.apply_reload(since, &sessions, &users);
        Ok(())
    }

    /// Number of remembered revoked sessions (diagnostics and tests).
    pub fn revoked_session_count(&self) -> usize {
        self.read().sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_common::{FakeClock, IdGenerator, SequentialIdGenerator};

    fn setup() -> (RevocationSet, FakeClock, SequentialIdGenerator) {
        let clock = FakeClock::at_default_epoch();
        (
            RevocationSet::new(Duration::minutes(15), Arc::new(clock.clone())),
            clock,
            SequentialIdGenerator::default(),
        )
    }

    const DISABLED: UserFlags = UserFlags {
        disabled: true,
        deletion_pending: false,
        must_change_password: false,
    };

    #[test]
    fn revoked_sessions_are_refused_for_one_token_lifetime() {
        let (set, clock, ids) = setup();
        let (s, u) = (
            SessionId::from(ids.next_ulid()),
            UserId::from(ids.next_ulid()),
        );
        assert_eq!(set.check(s, u), Ok(UserFlags::default()));
        set.revoke_sessions([s]);
        assert_eq!(set.check(s, u), Err(Revoked::Session));
        clock.advance(Duration::minutes(17));
        set.apply_reload(set.begin_reload(), &[], &[]);
        assert_eq!(set.check(s, u), Ok(UserFlags::default()));
        assert_eq!(set.revoked_session_count(), 0);
    }

    #[test]
    fn database_sessions_are_merged_and_refreshed() {
        let (set, clock, ids) = setup();
        let (s, u) = (
            SessionId::from(ids.next_ulid()),
            UserId::from(ids.next_ulid()),
        );
        set.apply_reload(set.begin_reload(), &[s], &[]);
        clock.advance(Duration::minutes(16));
        set.apply_reload(set.begin_reload(), &[s], &[]);
        clock.advance(Duration::minutes(16));
        assert_eq!(set.check(s, u), Err(Revoked::Session));
    }

    #[test]
    fn user_flags_follow_reloads_and_local_changes() {
        let (set, _clock, ids) = setup();
        let (s, u) = (
            SessionId::from(ids.next_ulid()),
            UserId::from(ids.next_ulid()),
        );
        set.apply_reload(set.begin_reload(), &[], &[(u, DISABLED)]);
        assert_eq!(set.check(s, u), Err(Revoked::UserDisabled));
        // Re-enabled in the database; the next reload lifts the block.
        set.apply_reload(set.begin_reload(), &[], &[]);
        assert_eq!(set.check(s, u), Ok(UserFlags::default()));
        set.update_user(u, |f| f.deletion_pending = true);
        assert_eq!(
            set.check(s, u),
            Ok(UserFlags {
                deletion_pending: true,
                ..UserFlags::default()
            })
        );
    }

    #[test]
    fn a_stale_reload_cannot_undo_a_concurrent_local_revocation() {
        let (set, _clock, ids) = setup();
        let (s, u) = (
            SessionId::from(ids.next_ulid()),
            UserId::from(ids.next_ulid()),
        );
        let since = set.begin_reload();
        // Meanwhile a request disables the user…
        set.set_user(u, DISABLED);
        // …and the reload's snapshot, taken before that commit, says "not disabled".
        set.apply_reload(since, &[], &[]);
        assert_eq!(set.check(s, u), Err(Revoked::UserDisabled));
        // The next reload sees the committed state.
        set.apply_reload(set.begin_reload(), &[], &[(u, DISABLED)]);
        assert_eq!(set.check(s, u), Err(Revoked::UserDisabled));
        // A local enable during a reload wins over that reload's stale "disabled".
        let since = set.begin_reload();
        set.set_user(u, UserFlags::default());
        set.apply_reload(since, &[], &[(u, DISABLED)]);
        assert_eq!(set.check(s, u), Ok(UserFlags::default()));
    }

    #[test]
    fn purge_tombstones_outlive_reloads_until_tokens_expire() {
        let (set, clock, ids) = setup();
        let (s, u) = (
            SessionId::from(ids.next_ulid()),
            UserId::from(ids.next_ulid()),
        );
        set.mark_purged(u);
        set.apply_reload(set.begin_reload(), &[], &[]);
        assert_eq!(set.check(s, u), Err(Revoked::UserPurged));
        clock.advance(Duration::minutes(18));
        set.apply_reload(set.begin_reload(), &[], &[]);
        assert_eq!(set.check(s, u), Ok(UserFlags::default()));
    }
}

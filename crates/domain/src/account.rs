//! Accounts (§5.2, §7.4 `users`, §8).

use crate::macros::string_enum;

string_enum! {
    /// Account role.
    pub enum Role("role") {
        /// Manages accounts; cannot read other users' vaults.
        Admin => "admin",
        /// Regular user.
        Member => "member",
    }
}

string_enum! {
    /// Account lifecycle status (`snake_case`, as in `users.status` and the
    /// `account_deletion_pending` problem type).
    pub enum AccountStatus("account status") {
        /// Signed up, awaiting admin approval (D22).
        Pending => "pending",
        /// Approved and usable.
        Active => "active",
        /// Disabled by an admin; sessions revoked.
        Disabled => "disabled",
        /// Sign-up rejected by an admin.
        Rejected => "rejected",
        /// Scheduled for deletion; export-only sessions (D25).
        DeletionPending => "deletion_pending",
    }
}

impl AccountStatus {
    /// Whether a user with this status may sign in at all (`deletion_pending` gets an
    /// export-only session, §5.2).
    pub const fn can_sign_in(self) -> bool {
        matches!(self, Self::Active | Self::DeletionPending)
    }
}

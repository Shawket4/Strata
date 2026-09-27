//! Closed sets of values stored as `text` columns with `CHECK` constraints.

use std::fmt;

/// A stored text value that is not one of the enum's variants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {type_name} value `{value}`")]
pub struct UnknownVariant {
    /// Rust type name.
    pub type_name: &'static str,
    /// Rejected value.
    pub value: String,
}

macro_rules! text_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $variant:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name { $($(#[$vm])* $variant),+ }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The stored text value.
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
        }

        impl std::str::FromStr for $name {
            type Err = UnknownVariant;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($s => Ok(Self::$variant),)+
                    _ => Err(UnknownVariant { type_name: stringify!($name), value: s.to_owned() }),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <str as sqlx::Type<sqlx::Postgres>>::type_info()
            }
            fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
                <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
            }
        }

        impl sqlx::postgres::PgHasArrayType for $name {
            fn array_type_info() -> sqlx::postgres::PgTypeInfo {
                <&str as sqlx::postgres::PgHasArrayType>::array_type_info()
            }
        }

        impl sqlx::Encode<'_, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <&str as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_str(), buf)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <&str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
                Ok(s.parse::<Self>()?)
            }
        }
    };
}

text_enum!(
    /// `users.role`.
    UserRole { Admin = "admin", Member = "member" }
);
text_enum!(
    /// `users.status` (D22, D25).
    UserStatus {
        Pending = "pending",
        Active = "active",
        Disabled = "disabled",
        Rejected = "rejected",
        DeletionPending = "deletion_pending",
    }
);
text_enum!(
    /// `devices.platform`.
    Platform { Android = "android", Ios = "ios", Macos = "macos", Windows = "windows", Linux = "linux" }
);
text_enum!(
    /// `devices.push_provider` (D27).
    PushProvider { Fcm = "fcm", Apns = "apns", Wns = "wns", None = "none" }
);
text_enum!(
    /// `sessions.revoked_reason`.
    RevokeReason {
        Logout = "logout",
        DeviceRemoved = "device_removed",
        UserDisabled = "user_disabled",
        DeletionScheduled = "deletion_scheduled",
        RefreshReuse = "refresh_reuse",
        Expired = "expired",
        Admin = "admin",
    }
);
text_enum!(
    /// `notes.kind`.
    NoteKind {
        Note = "note",
        Concept = "concept",
        Person = "person",
        Company = "company",
        Document = "document",
        Place = "place",
    }
);
text_enum!(
    /// `notes.lang`.
    Lang { Ar = "ar", En = "en", Mixed = "mixed" }
);
text_enum!(
    /// `links.kind`.
    LinkKind { Link = "link", Embed = "embed" }
);
text_enum!(
    /// Provenance of a relation or custody event.
    By { User = "user", Ai = "ai" }
);
text_enum!(
    /// `entities.kind`.
    EntityKind { Person = "person", Company = "company", Document = "document", Place = "place" }
);
text_enum!(
    /// `documents.copy`.
    DocCopy { Original = "original", CertifiedCopy = "certified copy", Copy = "copy", Digital = "digital" }
);
text_enum!(
    /// `documents.status`.
    DocStatus {
        Stored = "stored",
        CheckedOut = "checked-out",
        WithThirdParty = "with-third-party",
        Lost = "lost",
        Destroyed = "destroyed",
    }
);
text_enum!(
    /// `custody_events.type` (§6.12).
    CustodyType {
        StoredAt = "stored-at",
        MovedTo = "moved-to",
        HandedTo = "handed-to",
        ReturnedBy = "returned-by",
        SentTo = "sent-to",
        ReceivedFrom = "received-from",
        Lost = "lost",
        Found = "found",
        Destroyed = "destroyed",
    }
);
text_enum!(
    /// `tasks.status`.
    TaskStatus { Open = "open", Done = "done", Cancelled = "cancelled" }
);
text_enum!(
    /// `tasks.priority` (Tasks plugin signifiers 🔺⏫🔼🔽⏬).
    Priority { Highest = "highest", High = "high", Medium = "medium", Low = "low", Lowest = "lowest" }
);
text_enum!(
    /// `notification_log.provider`.
    NotifyProvider { Fcm = "fcm", Apns = "apns", Wns = "wns", EventStream = "event_stream" }
);
text_enum!(
    /// `notification_log.result`.
    NotifyResult { Sent = "sent", Failed = "failed", InvalidToken = "invalid_token" }
);
text_enum!(
    /// `jobs.status`.
    JobStatus { Queued = "queued", Running = "running", Done = "done", Failed = "failed" }
);
text_enum!(
    /// `suggestions.status`.
    SuggestionStatus {
        Pending = "pending",
        Accepted = "accepted",
        Rejected = "rejected",
        Superseded = "superseded",
    }
);
text_enum!(
    /// `suggestion_replies.author`.
    ReplyAuthor { User = "user", Ai = "ai" }
);
text_enum!(
    /// `ai_decisions.kind` (§9.8).
    DecisionKind {
        Link = "link",
        EntityMention = "entity_mention",
        Relation = "relation",
        CustodyEvent = "custody_event",
        TaskSuggestion = "task_suggestion",
        Filing = "filing",
        Concept = "concept",
        Correction = "correction",
    }
);
text_enum!(
    /// `change_log.op`.
    ChangeOp { Upsert = "upsert", Delete = "delete" }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_values_round_trip() {
        assert_eq!(DocStatus::WithThirdParty.as_str(), "with-third-party");
        assert_eq!(
            "certified copy".parse::<DocCopy>(),
            Ok(DocCopy::CertifiedCopy)
        );
        assert_eq!(UserStatus::DeletionPending.to_string(), "deletion_pending");
        for s in UserStatus::ALL {
            assert_eq!(s.as_str().parse::<UserStatus>(), Ok(*s));
        }
        assert_eq!(
            "archived".parse::<JobStatus>(),
            Err(UnknownVariant {
                type_name: "JobStatus",
                value: "archived".into()
            })
        );
    }
}

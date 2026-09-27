//! `PgUsageStore` on a real per-test `PostgreSQL` database: per-user rows stay isolated by RLS,
//! the global day total sums every user, and the budget guard pauses on the global cap.
#![allow(clippy::expect_used, clippy::too_many_lines)] // tests: expect with messages, one scenario

use std::sync::Arc;

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use strata_ai::budget::{BudgetLimits, UsageTotals};
use strata_ai::{AiCaller, AiError, BudgetGuard, PauseReason, PgUsageStore, Usage, UsageStore};
use strata_index::repo::settings::{self, AiUsage};
use strata_testkit::{TestDb, TestUser};

fn usage(input: u64, output: u64, cost: u64) -> Usage {
    Usage {
        input_tokens: input,
        cache_read_input_tokens: 10,
        output_tokens: output,
        cost_micros: Some(cost),
        ..Usage::default()
    }
}

#[tokio::test]
async fn usage_is_recorded_per_user_under_rls_and_summed_globally() {
    let db = TestDb::new().await.expect("db");
    let alice = TestUser::new("alice").create(&db).await.expect("alice");
    let bob = TestUser::new("bob").create(&db).await.expect("bob");
    let (sa, sb) = (db.scope(alice.id), db.scope(bob.id));
    let store = PgUsageStore::new(db.app_db.clone());
    let day = NaiveDate::from_ymd_opt(2026, 9, 27).expect("date");

    store
        .add(
            &sa,
            day,
            "claude_cli",
            "claude-sonnet-5",
            &usage(100, 20, 5),
        )
        .await
        .expect("a1");
    store
        .add(
            &sa,
            day,
            "claude_cli",
            "claude-sonnet-5",
            &usage(100, 20, 5),
        )
        .await
        .expect("a2");
    store
        .add(
            &sb,
            day,
            "anthropic_api",
            "claude-opus-5",
            &usage(50, 5, 900),
        )
        .await
        .expect("b1");

    assert_eq!(
        store.user_day(&sa, day).await.expect("alice"),
        UsageTotals {
            calls: 2,
            input_tokens: 220,
            output_tokens: 40,
            cost_micros: 10
        }
    );
    assert_eq!(
        store.user_day(&sb, day).await.expect("bob"),
        UsageTotals {
            calls: 1,
            input_tokens: 60,
            output_tokens: 5,
            cost_micros: 900
        }
    );
    let global = UsageTotals {
        calls: 3,
        input_tokens: 280,
        output_tokens: 45,
        cost_micros: 910,
    };
    assert_eq!(store.global_day(&sa, day).await.expect("global"), global);
    assert_eq!(store.global_day(&sb, day).await.expect("global"), global);
    let next = day.succ_opt().expect("date");
    assert_eq!(
        store.global_day(&sa, next).await.expect("empty"),
        UsageTotals::default()
    );

    // Alice's scope sees only her ai_usage rows.
    let mut tx = db.begin(alice.id).await.expect("tx");
    assert_eq!(
        settings::ai_usage_for_day(&mut tx, day)
            .await
            .expect("rows"),
        vec![AiUsage {
            day,
            provider: "claude_cli".into(),
            model: "claude-sonnet-5".into(),
            calls: 2,
            input_tokens: 220,
            output_tokens: 40,
            est_cost_micros: 10,
        }]
    );
    tx.commit().await.expect("commit");

    // The global cap applies across users: 325 of 330 tokens used, then carol adds 10
    // (default test clock: 2026-09-27T12:00Z).
    let guard = BudgetGuard::new(
        BudgetLimits {
            global_daily_tokens: 330,
            ..BudgetLimits::default()
        },
        chrono_tz::UTC,
        Arc::new(db.clock.clone()),
        Arc::new(store),
    );
    let carol = TestUser::new("carol").create(&db).await.expect("carol");
    let caller = AiCaller {
        scope: db.scope(carol.id),
        username: "carol".into(),
    };
    assert_eq!(guard.check(&caller).await, Ok(()));
    guard
        .record(&caller, "claude_cli", "m", &usage(0, 0, 0))
        .await
        .expect("record");
    assert_eq!(
        guard.check(&caller).await,
        Err(AiError::Paused {
            reason: PauseReason::GlobalBudget,
            until: Some("2026-09-28T00:00:00Z".parse().expect("rfc3339")),
        })
    );
    db.cleanup().await.expect("cleanup");
}

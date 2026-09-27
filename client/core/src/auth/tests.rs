use std::sync::Mutex;

use strata_client::TokenProvider as _;

use super::*;
use crate::clock::FakeClock;
use crate::testing::FakeAccountApi;

#[derive(Debug, Default)]
struct MemStore {
    tokens: Mutex<Option<StoredTokens>>,
    ended: Mutex<u32>,
}

impl TokenStore for MemStore {
    fn load(&self) -> Option<StoredTokens> {
        self.tokens.lock().expect("lock").clone()
    }
    fn save(&self, t: &StoredTokens) {
        *self.tokens.lock().expect("lock") = Some(t.clone());
    }
    fn session_ended(&self) {
        *self.tokens.lock().expect("lock") = None;
        *self.ended.lock().expect("lock") += 1;
    }
}

async fn provider() -> (Arc<MemStore>, Arc<FakeAccountApi>, CoreTokenProvider) {
    let api = Arc::new(FakeAccountApi::default());
    api.add_user("shawket", "pw", "01K5DSSE000000000000000USR", "Africa/Cairo");
    let t = api
        .login(
            "https://s".into(),
            "shawket".into(),
            "pw".into(),
            "d".into(),
            crate::view::model::Platform::Linux,
        )
        .await
        .expect("login");
    let store = Arc::new(MemStore::default());
    store.save(&stored(&t, "2026-09-27T10:00:00+00:00"));
    let clock = Arc::new(FakeClock::at("2026-09-27T10:05:00Z"));
    let p = CoreTokenProvider::new(store.clone(), api.clone(), "https://s".into(), clock);
    (store, api, p)
}

#[tokio::test]
async fn access_token_comes_from_the_store() {
    let (_, _, p) = provider().await;
    assert_eq!(p.access_token().await.expect("ok"), Some("access-1".into()));
}

#[tokio::test]
async fn refresh_rotates_and_stores_tokens() {
    let (store, api, p) = provider().await;
    assert_eq!(p.refresh().await.expect("ok"), Some("access-2".into()));
    let t = store.load().expect("stored");
    assert_eq!(t.access_token, "access-2");
    assert_eq!(t.refresh_token, "refresh-2");
    assert_eq!(t.updated_at, "2026-09-27T10:05:00+00:00");
    assert_eq!(
        *api.calls.lock().expect("lock"),
        vec!["login:shawket".to_owned(), "refresh:refresh-1".to_owned()]
    );
}

#[tokio::test]
async fn a_caller_that_waited_reuses_the_new_token() {
    let (_, api, p) = provider().await;
    // First caller refreshes; the second one failed with the old token and must not spend the
    // rotated refresh token again.
    assert_eq!(p.refresh_now(Some("access-1".into())).await, Ok(Some("access-2".into())));
    assert_eq!(p.refresh_now(Some("access-1".into())).await, Ok(Some("access-2".into())));
    assert_eq!(api.calls.lock().expect("lock").len(), 2);
}

#[tokio::test]
async fn refused_refresh_ends_the_session() {
    let (store, api, p) = provider().await;
    *api.refresh_error.lock().expect("lock") = Some(NetError::Unauthorized);
    assert_eq!(p.refresh().await.expect("ok"), None);
    assert_eq!(store.load(), None);
    assert_eq!(*store.ended.lock().expect("lock"), 1);
}

#[tokio::test]
async fn offline_refresh_is_an_error_not_a_sign_out() {
    let (store, api, p) = provider().await;
    *api.refresh_error.lock().expect("lock") = Some(NetError::Offline("x".into()));
    assert_eq!(
        p.refresh_now(None).await,
        Err(NetError::Offline("x".into()))
    );
    assert_eq!(store.load().map(|t| t.access_token), Some("access-1".into()));
}

#[test]
fn account_modes() {
    assert_eq!(account_mode("active", false), AccountMode::Active);
    assert_eq!(account_mode("active", true), AccountMode::PasswordChangeRequired);
    assert_eq!(account_mode("disabled", false), AccountMode::Disabled);
    assert_eq!(account_mode("deletion_pending", true), AccountMode::DeletionPending);
}

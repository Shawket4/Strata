//! The remaining test builders: a user with a fixed ID and password hash, and the synthetic
//! vault written to disk (the performance suite's input, PLAN §16.7) with its default shape.
#![allow(clippy::expect_used)]

use pretty_assertions::assert_eq;
use strata_testkit::{SyntheticConfig, SyntheticVault, TestDb, TestUser};

#[tokio::test]
async fn a_user_can_have_a_fixed_id_and_password_hash() {
    let db = TestDb::new().await.expect("db");
    let id: strata_common::UserId = "01M3HBS0G000000000000000ZZ".parse().expect("id");
    let hash = "$argon2id$v=19$m=64,t=1,p=1$c2FsdHNhbHQ$aGFzaGhhc2g";
    let carol = TestUser::new("Carol")
        .id(id)
        .password_hash(hash)
        .create(&db)
        .await
        .expect("carol");
    assert_eq!(
        (
            carol.id,
            carol.username.as_str(),
            carol.display_name.as_str(),
            carol.password_hash.as_str()
        ),
        (id, "Carol", "Carol", hash)
    );
    // The fixed ID did not use up the deterministic sequence.
    let dave = TestUser::new("dave").create(&db).await.expect("dave");
    assert_eq!(dave.id.to_string(), "01M3HBS0G00000000000000001");
    db.cleanup().await.expect("cleanup");
}

#[test]
fn the_synthetic_vault_writes_every_file_and_defaults_to_ten_thousand() {
    assert_eq!(
        SyntheticConfig::default(),
        SyntheticConfig {
            files: 10_000,
            seed: 2026
        }
    );
    let vault = SyntheticVault::generate(&SyntheticConfig { files: 60, seed: 3 });
    let dir = tempfile::tempdir().expect("dir");
    vault.write_to(dir.path()).expect("write");
    let mut written = 0;
    for f in &vault.files {
        let on_disk = std::fs::read_to_string(dir.path().join(&f.path)).expect(&f.path);
        assert_eq!(on_disk, f.content, "{}", f.path);
        written += on_disk.len();
    }
    assert_eq!(vault.total_bytes(), written);
    assert_eq!(vault.files.len(), 60);
    // Writing into a path that is a file fails instead of half-writing silently.
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, "x").expect("file");
    assert_eq!(
        vault.write_to(&blocked).map_err(|e| e.kind()),
        Err(std::io::ErrorKind::NotADirectory)
    );
}

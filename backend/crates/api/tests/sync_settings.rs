//! Settings reach every device through the sync feed (PLAN §7.5 Sync, §12.5b, D27): `PATCH
//! /me`, `PATCH /devices/{id}`, `DELETE /devices/{id}` and the `device.settings` op append
//! change-log rows, so `/sync/changes` carries the new records (map settings such as
//! `preferences` as one record per entry, removed entries as tombstones, non-string values as
//! their `sync-model` text), and an unchanged value appends nothing.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod sync_harness;

use std::collections::HashMap;

use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use strata_common::Clock;
use sync_harness::{H, User};
use sync_model::changes::{DeviceSettingRecord, SettingRecord};
use sync_model::ops::{self as o, Op};
use sync_model::{Change, EntityType, Record, SyncOp};
use ulid::Ulid;

fn setting(key: &str, value: &str) -> (EntityType, String, Option<Record>) {
    (
        EntityType::Setting,
        key.to_owned(),
        Some(Record::Setting(SettingRecord {
            key: key.to_owned(),
            value: value.to_owned(),
        })),
    )
}

fn device_setting(device: Ulid, value: bool) -> (EntityType, String, Option<Record>) {
    (
        EntityType::DeviceSetting,
        format!("{device}:reminders_enabled"),
        Some(Record::DeviceSetting(DeviceSettingRecord {
            device_id: device,
            key: "reminders_enabled".into(),
            value: value.to_string(),
        })),
    )
}

fn tombstone(t: EntityType, id: &str) -> (EntityType, String, Option<Record>) {
    (t, id.to_owned(), None)
}

/// The changes after `since` as (type, ID, record or tombstone), in seq order; returns the
/// next seq too.
async fn changes_after(
    h: &H,
    user: &User,
    since: u64,
) -> (Vec<(EntityType, String, Option<Record>)>, u64) {
    let page = h.changes_page(user, since, 1, None).await.expect("changes");
    assert!(!page.has_more);
    let rows = page
        .changes
        .iter()
        .map(|c| {
            (
                c.entity_type,
                c.entity_id.clone(),
                match &c.change {
                    Change::Upsert { record } => Some(record.clone()),
                    Change::Delete => None,
                },
            )
        })
        .collect();
    (rows, page.next_seq)
}

fn prefs(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[tokio::test]
async fn patch_me_logs_each_changed_setting_record() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let start = h.bootstrap(&alice, None).await.cursor.expect("cursor").seq;

    ops::update_me(
        &alice.client,
        &types::UpdateMe {
            ui_language: Some(types::UiLanguage::Ar),
            timezone: Some("Africa/Cairo".into()),
            preferences: Some(prefs(&[("theme", "dark"), ("font_scale", "1.25")])),
            ..Default::default()
        },
    )
    .await
    .expect("patch");
    let (rows, seq) = changes_after(&h, &alice, start).await;
    assert_eq!(
        rows,
        vec![
            setting("ui_language", "ar"),
            setting("timezone", "Africa/Cairo"),
            setting("preferences.font_scale", "1.25"),
            setting("preferences.theme", "dark"),
        ]
    );

    // Same timezone, one preference removed, one changed, one added.
    ops::update_me(
        &alice.client,
        &types::UpdateMe {
            timezone: Some("Africa/Cairo".into()),
            preferences: Some(prefs(&[("font_scale", "1.5"), ("density", "compact")])),
            ..Default::default()
        },
    )
    .await
    .expect("patch");
    let (rows, seq2) = changes_after(&h, &alice, seq).await;
    assert_eq!(
        rows,
        vec![
            setting("preferences.density", "compact"),
            setting("preferences.font_scale", "1.5"),
            tombstone(EntityType::Setting, "preferences.theme"),
        ]
    );

    // Nothing changed: nothing logged.
    ops::update_me(
        &alice.client,
        &types::UpdateMe {
            ui_language: Some(types::UiLanguage::Ar),
            preferences: Some(prefs(&[("font_scale", "1.5"), ("density", "compact")])),
            ..Default::default()
        },
    )
    .await
    .expect("patch");
    assert_eq!(changes_after(&h, &alice, seq2).await, (vec![], seq2));

    // A fresh device bootstraps the same records.
    let device = h.bootstrap(&alice, None).await;
    let settings: Vec<&Record> = device
        .records
        .iter()
        .filter(|((t, _), _)| *t == EntityType::Setting)
        .map(|(_, r)| r)
        .collect();
    assert_eq!(
        settings,
        vec![
            &setting("preferences.density", "compact").2.expect("record"),
            &setting("preferences.font_scale", "1.5").2.expect("record"),
            &setting("timezone", "Africa/Cairo").2.expect("record"),
            &setting("ui_language", "ar").2.expect("record"),
        ]
    );
    h.finish().await;
}

#[tokio::test]
async fn non_string_setting_values_sync_as_their_text() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let start = h.bootstrap(&alice, None).await.cursor.expect("cursor").seq;
    let now = h.clock.now();
    let map = rmpv::Value::Map(vec![
        (rmpv::Value::from("auto_file"), rmpv::Value::from(true)),
        (rmpv::Value::from("digest_hour"), rmpv::Value::from(7)),
        (rmpv::Value::from("threshold"), rmpv::Value::F64(0.7)),
        (
            rmpv::Value::from("folders"),
            rmpv::Value::Array(vec![rmpv::Value::from("inbox")]),
        ),
    ]);
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, &map).expect("encode");
    let mut tx = h.db.begin(alice.id).await.expect("tx");
    strata_api::sync::records::put_setting_logged(&mut tx, "ai", &bytes, now)
        .await
        .expect("put");
    strata_api::sync::records::put_setting_logged(
        &mut tx,
        "relation_threshold",
        &[0xcb, 0x3f, 0xe6, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66],
        now,
    )
    .await
    .expect("put");
    tx.commit().await.expect("commit");
    let (rows, _) = changes_after(&h, &alice, start).await;
    // The array entry is not synced.
    assert_eq!(
        rows,
        vec![
            setting("ai.auto_file", "true"),
            setting("ai.digest_hour", "7"),
            setting("ai.threshold", "0.7"),
            setting("relation_threshold", "0.7"),
        ]
    );
    let device = h.bootstrap(&alice, None).await;
    assert_eq!(
        device
            .records
            .keys()
            .filter(|(t, _)| *t == EntityType::Setting)
            .map(|(_, k)| k.as_str())
            .collect::<Vec<_>>(),
        vec![
            "ai.auto_file",
            "ai.digest_hour",
            "ai.threshold",
            "relation_threshold"
        ]
    );
    h.finish().await;
}

#[tokio::test]
async fn device_reminder_switches_reach_every_device() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let phone = h.login(&alice, "alice", "alice-phone").await;
    let start = h.bootstrap(&alice, None).await.cursor.expect("cursor").seq;

    // PATCH /devices/{id} from the laptop switches the phone's reminders off.
    let d = ops::update_device(
        &alice.client,
        phone.device,
        &types::UpdateDevice {
            reminders_enabled: Some(false),
            name: None,
        },
    )
    .await
    .expect("patch");
    assert!(!d.reminders_enabled);
    let (rows, seq) = changes_after(&h, &alice, start).await;
    assert_eq!(rows, vec![device_setting(phone.device, false)]);
    // Unchanged (and a rename only): nothing logged.
    ops::update_device(
        &alice.client,
        phone.device,
        &types::UpdateDevice {
            reminders_enabled: Some(false),
            name: Some("Alice's phone".into()),
        },
    )
    .await
    .expect("patch");
    assert_eq!(changes_after(&h, &alice, seq).await, (vec![], seq));

    // The phone turns them back on through the op (same record, once).
    let op = |n: u128, on: bool| {
        SyncOp::new(
            Ulid(0x0199_4444_0000_0000_0000_0000_0000_0000 + n),
            None,
            Op::DeviceSettings(o::DeviceSettings {
                device_id: phone.device,
                reminders_enabled: Some(on),
            }),
        )
    };
    let (res, _) = h.push(&phone, vec![op(1, true), op(2, true)]).await;
    assert_eq!(
        res.results
            .iter()
            .map(|r| r.result.clone())
            .collect::<Vec<_>>(),
        vec![
            sync_model::OpResult::Applied {
                new_version: None,
                merged: false
            };
            2
        ]
    );
    let (rows, seq) = changes_after(&h, &alice, seq).await;
    assert_eq!(rows, vec![device_setting(phone.device, true)]);

    // Removing the phone leaves a tombstone for its setting.
    ops::delete_device(&alice.client, phone.device)
        .await
        .expect("delete");
    let (rows, _) = changes_after(&h, &alice, seq).await;
    assert_eq!(
        rows,
        vec![tombstone(
            EntityType::DeviceSetting,
            &format!("{}:reminders_enabled", phone.device)
        )]
    );
    h.finish().await;
}

// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable numeric review through real SQLite repositories.

#[path = "support/numeric_migrations.rs"]
mod numeric_migrations;

use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, EndpointData, PricePrior, PriorProvenance,
    PriorValues, TerminalPriors, UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_reporting::api::model::ReportKey;
use tallyline_reporting::api::{ManualEdit, ReportRepository, ReportingApplication};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(1000)
    }
}
struct Cache(Vec<CachedReference>);
impl ReferenceDataCache for Cache {
    type Error = std::convert::Infallible;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(self.0.clone())
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}
fn reference(
    price: &str,
) -> Result<tallyline_reference_data::api::model::ReferenceSnapshot, Box<dyn std::error::Error>> {
    let prior = PricePrior::new(
        CommodityId(42),
        TradeSide::Buy,
        PriorValues {
            price: price.parse()?,
            stock: "0".parse()?,
            status: std::num::NonZeroU8::new(3),
            container_sizes: vec![],
        },
        PriorProvenance {
            game_version: "4.10.2".to_owned(),
            modified_at: Timestamp::from_seconds(900),
        },
    )?;
    Ok(ReferenceApplication::load(
        Cache(vec![
            CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::Commodities(vec![Commodity::new(
                    CommodityId(43),
                    "Silver",
                    "SILV",
                    true,
                    CommodityAverages::new(None, None),
                )]),
            ),
            CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11), vec![prior])?),
            ),
        ]),
        Fixed,
    )?
    .snapshot(UexEnvironment::Live))
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_ui_12_q_manual_r_val_7_q_manual__price_approval_pending_revocation_and_focus_survive_reopening()
 {
    let path = test_path("tallyline-price-review").unwrap();
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&path).unwrap(), Fixed);
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap();
    let report = app.create(key, Some("4.10.2")).unwrap();
    let report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::AddCommodity(CommodityId(42)),
            &reference("100").unwrap(),
        )
        .unwrap();
    let row = report.rows()[0].id();
    let report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::Price(row, Some("100".parse().unwrap())),
            &reference("100").unwrap(),
        )
        .unwrap();
    let approved = app
        .confirm_price(
            report.id(),
            report.version(),
            row,
            &reference("100").unwrap(),
        )
        .unwrap();
    drop(app);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        store.load_draft(approved.id()).unwrap(),
        Some(approved.clone())
    );
    let mut app = ReportingApplication::new(store, Fixed);
    let revoked = app
        .reassess_prices(approved.id(), approved.version(), &reference("90").unwrap())
        .unwrap();
    assert!(revoked.rows()[0].price_confirmation_revoked());
    assert!(revoked.rows()[0].price_unreviewed());
    drop(app);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        store.load_draft(revoked.id()).unwrap(),
        Some(revoked.clone())
    );
    let mut app = ReportingApplication::new(store, Fixed);
    let focused = app
        .focus_price(revoked.id(), revoked.version(), row)
        .unwrap();
    assert!(!focused.rows()[0].price_unreviewed());
    assert!(focused.rows()[0].price_needs_confirmation());
    let confirmed = app
        .confirm_price(
            focused.id(),
            focused.version(),
            row,
            &reference("90").unwrap(),
        )
        .unwrap();
    assert!(!confirmed.rows()[0].price_confirmation_revoked());
    assert!(!confirmed.rows()[0].price_needs_confirmation());
    drop(app);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(store.load_draft(confirmed.id()).unwrap(), Some(confirmed));
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_ui_12_q_manual__contradictory_persisted_numeric_decisions_are_refused() {
    use tallyline_reporting::api::RepositoryError;
    let path = test_path("tallyline-price-corrupt").unwrap();
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    let mut report = store
        .create_manual(
            ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            Timestamp::from_seconds(900),
            None,
        )
        .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("100".parse().unwrap())).unwrap();
    store.save_draft(&report, 0).unwrap();
    drop(store);
    let connection = rusqlite::Connection::open(&path).unwrap();
    let payload: String = connection
        .query_row("SELECT payload FROM reports", [], |row| row.get(0))
        .unwrap();
    let original: serde_json::Value = serde_json::from_str(&payload).unwrap();
    for (price, approval, deviation, unreviewed, revoked) in [
        (
            serde_json::Value::Null,
            serde_json::json!(["100", "equal", "live"]),
            serde_json::json!("equal"),
            false,
            false,
        ),
        (
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::json!("minor"),
            false,
            false,
        ),
        (
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::Value::Null,
            true,
            false,
        ),
        (
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::Value::Null,
            false,
            true,
        ),
        (
            serde_json::json!("100"),
            serde_json::json!(["101", "major", "live"]),
            serde_json::json!("major"),
            false,
            false,
        ),
        (
            serde_json::json!("100"),
            serde_json::json!(["100", "minor", "live"]),
            serde_json::json!("major"),
            false,
            false,
        ),
        (
            serde_json::json!("100"),
            serde_json::json!(["100", "major", "live"]),
            serde_json::Value::Null,
            false,
            false,
        ),
        (
            serde_json::json!("100"),
            serde_json::json!(["100", "major", "live"]),
            serde_json::json!("major"),
            true,
            false,
        ),
        (
            serde_json::json!("100"),
            serde_json::json!(["100", "major", "live"]),
            serde_json::json!("major"),
            false,
            true,
        ),
        (
            serde_json::json!("100"),
            serde_json::Value::Null,
            serde_json::Value::Null,
            true,
            false,
        ),
        (
            serde_json::json!("100"),
            serde_json::Value::Null,
            serde_json::Value::Null,
            false,
            true,
        ),
        (
            serde_json::json!("100"),
            serde_json::Value::Null,
            serde_json::json!("minor"),
            true,
            false,
        ),
    ] {
        for field in ["price", "stock"] {
            let mut document = original.clone();
            let row = &mut document["rows"][0];
            row[field] = price.clone();
            let decision = format!("{field}_decision");
            row[decision.as_str()]["confirmation"] = approval.clone();
            row[decision.as_str()]["deviation"] = deviation.clone();
            row[decision.as_str()]["unreviewed"] = serde_json::json!(unreviewed);
            row[decision.as_str()]["revoked"] = serde_json::json!(revoked);
            connection
                .execute("UPDATE reports SET payload=?1", [document.to_string()])
                .unwrap();
            let mut store = SqliteReferenceCache::open(&path).unwrap();
            assert_eq!(
                store.load_draft(report.id()),
                Err(RepositoryError::Invalid),
                "{document}"
            );
            assert_eq!(store.drafts(), Err(RepositoryError::Invalid));
        }
    }
    drop(connection);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_ui_12_q_manual__stock_approval_and_focus_survive_reopening() {
    let path = test_path("tallyline-stock-review").unwrap();
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&path).unwrap(), Fixed);
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap();
    let report = app.create(key, Some("4.10.2")).unwrap();
    let snapshot = reference("100").unwrap();
    let report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::AddCommodity(CommodityId(42)),
            &snapshot,
        )
        .unwrap();
    let row = report.rows()[0].id();
    let report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::Stock(row, Some("100".parse().unwrap())),
            &snapshot,
        )
        .unwrap();
    let review = app
        .review_manual(report.id(), report.version(), &snapshot)
        .unwrap();
    assert!(review.report().rows()[0].stock_needs_confirmation());
    let focused = app
        .focus_stock(report.id(), review.report().version(), row)
        .unwrap();
    assert!(!focused.rows()[0].stock_unreviewed());
    drop(app);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        store.load_draft(report.id()).unwrap(),
        Some(focused.clone())
    );
    let mut app = ReportingApplication::new(store, Fixed);
    let approved = app
        .confirm_stock(report.id(), focused.version(), row, &snapshot)
        .unwrap();
    drop(app);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(store.load_draft(report.id()).unwrap(), Some(approved));
    drop(store);
    std::fs::remove_file(path).unwrap();
}

fn test_path(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    Ok(std::env::temp_dir().join(format!(
        "{name}-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    )))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_3_r_nf_11__cross_row_batch_values_survive_sqlite_reopening() {
    let path = test_path("tallyline-manual-batch").unwrap();
    let snapshot = reference("100").unwrap();
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&path).unwrap(), Fixed);
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap();
    let report = app.create(key, Some("4.10.2")).unwrap();
    let report = app
        .edit_batch(
            report.id(),
            0,
            &[
                ManualEdit::AddCommodity(CommodityId(42)),
                ManualEdit::AddCommodity(CommodityId(43)),
            ],
            &snapshot,
        )
        .unwrap();
    let report = app
        .edit_batch(
            report.id(),
            report.version(),
            &[
                ManualEdit::Price(
                    report.rows()[1].id(),
                    Some("12345678901234567890.125".parse().unwrap()),
                ),
                ManualEdit::TakeOver(report.rows()[0].id()),
            ],
            &snapshot,
        )
        .unwrap();
    assert_eq!(report.rows()[0].price().unwrap().to_string(), "100");
    assert_eq!(
        report.rows()[1].price().unwrap().to_string(),
        "12345678901234567890.125"
    );
    assert!(report.rows()[1].stock().is_none());
    assert!(report.rows()[1].status().is_none());
    assert_eq!(report.version(), 6);
    drop(app);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(store.load_draft(report.id()).unwrap(), Some(report));
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_nf_11__later_invalid_edit_rolls_back_every_value_and_decision_across_reopening() {
    use tallyline_reporting::api::RepositoryError;
    let path = test_path("tallyline-manual-batch-rollback").unwrap();
    let snapshot = reference("100").unwrap();
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&path).unwrap(), Fixed);
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap();
    let report = app.create(key, Some("4.10.2")).unwrap();
    let report = app
        .edit(
            report.id(),
            0,
            ManualEdit::AddCommodity(CommodityId(42)),
            &snapshot,
        )
        .unwrap();
    let row = report.rows()[0].id();
    let report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::Price(row, Some("100".parse().unwrap())),
            &snapshot,
        )
        .unwrap();
    let report = app
        .confirm_price(report.id(), report.version(), row, &snapshot)
        .unwrap();
    assert!(report.rows()[0].price_confirmation().is_some());
    assert_eq!(
        app.edit_batch(
            report.id(),
            report.version(),
            &[
                ManualEdit::Price(row, Some("900".parse().unwrap())),
                ManualEdit::Status(row, std::num::NonZeroU8::new(7)),
            ],
            &snapshot
        ),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(app.drafts().unwrap(), vec![report.clone()]);
    drop(app);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(store.load_draft(report.id()).unwrap(), Some(report));
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_13_r_nf_11__entered_affinity_survives_durable_draft_reopening() {
    let path = test_path("tallyline-affinity-draft").unwrap();
    let mut app = ReportingApplication::new(SqliteReferenceCache::open(&path).unwrap(), Fixed);
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap();
    let mut report = app.create(key, Some("4.10.2")).unwrap();
    for value in [Some("-100"), Some("0"), Some("100"), None] {
        report = app
            .edit(
                report.id(),
                report.version(),
                ManualEdit::FactionAffinity(value.map(|v| v.parse().unwrap())),
                &reference("100").unwrap(),
            )
            .unwrap();
        drop(app);
        let mut store = SqliteReferenceCache::open(&path).unwrap();
        assert_eq!(store.load_draft(report.id()).unwrap(), Some(report.clone()));
        app = ReportingApplication::new(store, Fixed);
    }
    drop(app);
    std::fs::remove_file(path).unwrap();
}

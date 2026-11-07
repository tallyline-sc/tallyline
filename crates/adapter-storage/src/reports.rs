// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic manual Draft persistence in the coordinated application database.

use crate::api::SqliteReferenceCache;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tallyline_reporting::api::model::{
    Deviation, Report, ReportId, ReportKey, RowId, StoredReport,
};
use tallyline_reporting::api::{ReportRepository, RepositoryError};
use tallyline_shared_kernel::api::model::{
    CommodityId, TerminalId, Timestamp, TradeSide, UexEnvironment,
};

impl ReportRepository for SqliteReferenceCache {
    fn create_manual(
        &mut self,
        key: ReportKey,
        observed_at: Timestamp,
        game_version: Option<&str>,
    ) -> Result<Report, RepositoryError> {
        let transaction = self.connection.transaction().map_err(unavailable)?;
        transaction
            .execute("INSERT INTO reports(version,payload) VALUES(0,'')", [])
            .map_err(unavailable)?;
        let id =
            u64::try_from(transaction.last_insert_rowid()).map_err(|_| RepositoryError::Invalid)?;
        let report = Report::manual(ReportId(id), key, observed_at, game_version)
            .map_err(|_| RepositoryError::Invalid)?;
        let payload = encode(&report)?;
        transaction
            .execute(
                "UPDATE reports SET payload=?1 WHERE id=?2",
                (&payload, sql_integer(id)?),
            )
            .map_err(unavailable)?;
        transaction.execute("INSERT INTO report_recents(report_id,changed_order) VALUES(?1,(SELECT COALESCE(MAX(changed_order),0)+1 FROM report_recents))", [sql_integer(id)?]).map_err(unavailable)?;
        transaction.commit().map_err(unavailable)?;
        Ok(report)
    }
    fn load_draft(&mut self, id: ReportId) -> Result<Option<Report>, RepositoryError> {
        let record: Option<(i64, String)> = self
            .connection
            .query_row(
                "SELECT version,payload FROM reports WHERE id=?1 AND phase='draft'",
                [sql_integer(id.0)?],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(unavailable)?;
        record
            .map(|(version, payload)| decode(id, domain_integer(version)?, &payload))
            .transpose()
    }
    fn drafts(&mut self) -> Result<Vec<Report>, RepositoryError> {
        let mut query = self.connection.prepare("SELECT r.id,r.version,r.payload FROM reports r JOIN report_recents o ON o.report_id=r.id WHERE r.phase='draft' ORDER BY o.changed_order DESC").map_err(unavailable)?;
        let rows = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(unavailable)?;
        rows.map(|row| {
            let (id, version, payload) = row.map_err(unavailable)?;
            decode(
                ReportId(domain_integer(id)?),
                domain_integer(version)?,
                &payload,
            )
        })
        .collect()
    }
    fn save_draft(
        &mut self,
        report: &Report,
        expected_version: u64,
    ) -> Result<(), RepositoryError> {
        if report.state() != tallyline_reporting::api::model::ReportState::Draft
            || report.version() <= expected_version
        {
            return Err(RepositoryError::Invalid);
        }
        let payload = encode(report)?;
        let transaction = self.connection.transaction().map_err(unavailable)?;
        let changed = transaction
            .execute(
                "UPDATE reports SET version=?1,payload=?2 WHERE id=?3 AND version=?4 AND phase='draft'",
                (
                    sql_integer(report.version())?,
                    payload,
                    sql_integer(report.id().0)?,
                    sql_integer(expected_version)?,
                ),
            )
            .map_err(unavailable)?;
        if changed != 1 {
            return Err(RepositoryError::Conflict);
        }
        transaction.execute("UPDATE report_recents SET changed_order=(SELECT MAX(changed_order)+1 FROM report_recents) WHERE report_id=?1", [sql_integer(report.id().0)?]).map_err(unavailable)?;
        transaction.commit().map_err(unavailable)
    }
}
fn unavailable(_: rusqlite::Error) -> RepositoryError {
    RepositoryError::Unavailable
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    lifecycle: crate::report_lifecycle::LifecycleDocument,
    terminal: u32,
    side: String,
    environment: String,
    observed_at: String,
    game_version: Option<String>,
    faction_affinity: Option<i8>,
    rows: Vec<RowDocument>,
    attachment: Option<AttachmentDocument>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RowDocument {
    id: u64,
    commodity: u32,
    price: Option<String>,
    stock: Option<String>,
    status: Option<std::num::NonZeroU8>,
    price_decision: PriceDecisionDocument,
    stock_decision: PriceDecisionDocument,
    status_confirmation: Option<(std::num::NonZeroU8, StoredDeviation, StoredEnvironment)>,
    status_deviation: Option<StoredDeviation>,
    status_unreviewed: bool,
    status_confirmation_revoked: bool,
}
pub(crate) fn encode(report: &Report) -> Result<String, RepositoryError> {
    let document = Document {
        lifecycle: report.stored_lifecycle().into(),
        attachment: report.attachment().map(AttachmentDocument::from),
        terminal: report.key().terminal().0,
        side: match report.key().side() {
            TradeSide::Buy => "buy",
            TradeSide::Sell => "sell",
        }
        .to_owned(),
        environment: environment_name(report.key().environment()).to_owned(),
        observed_at: report.observed_at().millis().to_string(),
        game_version: report.game_version().map(str::to_owned),
        faction_affinity: report.faction_affinity().map(|value| value.value()),
        rows: report
            .rows()
            .iter()
            .map(|row| RowDocument {
                id: row.id().0,
                commodity: row.commodity().0,
                price: row.price().map(|v| v.to_string()),
                stock: row.stock().map(|v| v.to_string()),
                status: row.status(),
                price_decision: PriceDecisionDocument {
                    confirmation: row.price_confirmation().map(|approval| {
                        (
                            approval.value().to_string(),
                            approval.deviation().into(),
                            approval.reference_environment().into(),
                        )
                    }),
                    deviation: row.price_deviation().map(Into::into),
                    unreviewed: row.price_unreviewed(),
                    revoked: row.price_confirmation_revoked(),
                },
                stock_decision: PriceDecisionDocument {
                    confirmation: row.stock_confirmation().map(|approval| {
                        (
                            approval.value().to_string(),
                            approval.deviation().into(),
                            approval.reference_environment().into(),
                        )
                    }),
                    deviation: row.stock_deviation().map(Into::into),
                    unreviewed: row.stock_unreviewed(),
                    revoked: row.stock_confirmation_revoked(),
                },
                status_confirmation: row.status_confirmation().map(|approval| {
                    (
                        approval.value(),
                        approval.deviation().into(),
                        StoredEnvironment::from(approval.reference_environment()),
                    )
                }),
                status_deviation: row.status_deviation().map(Into::into),
                status_unreviewed: row.status_unreviewed(),
                status_confirmation_revoked: row.status_confirmation_revoked(),
            })
            .collect(),
    };
    serde_json::to_string(&document).map_err(|_| RepositoryError::Invalid)
}
pub(crate) fn decode(id: ReportId, version: u64, payload: &str) -> Result<Report, RepositoryError> {
    use tallyline_reporting::api::model::StoredRow;
    let document: Document = serde_json::from_str(payload).map_err(|_| RepositoryError::Invalid)?;
    let side = match document.side.as_str() {
        "buy" => TradeSide::Buy,
        "sell" => TradeSide::Sell,
        _ => return Err(RepositoryError::Invalid),
    };
    let environment = parse_environment(&document.environment)?;
    let key = ReportKey::new(TerminalId(document.terminal), side, environment)
        .map_err(|_| RepositoryError::Invalid)?;
    let observed_at = crate::reference_cache::timestamp(&document.observed_at)
        .map_err(|_| RepositoryError::Invalid)?;
    let rows = document
        .rows
        .into_iter()
        .map(|row| {
            Ok(StoredRow {
                id: RowId(row.id),
                commodity: CommodityId(row.commodity),
                price: row
                    .price
                    .map(|v| v.parse())
                    .transpose()
                    .map_err(|_| RepositoryError::Invalid)?,
                stock: row
                    .stock
                    .map(|v| v.parse())
                    .transpose()
                    .map_err(|_| RepositoryError::Invalid)?,
                status: row.status,
                price_decision: tallyline_reporting::api::model::StoredPriceDecision {
                    confirmation: row
                        .price_decision
                        .confirmation
                        .map(|(value, deviation, environment)| {
                            Ok((
                                value.parse().map_err(|_| RepositoryError::Invalid)?,
                                deviation.into(),
                                environment.into(),
                            ))
                        })
                        .transpose()?,
                    deviation: row.price_decision.deviation.map(Into::into),
                    unreviewed: row.price_decision.unreviewed,
                    revoked: row.price_decision.revoked,
                },
                stock_decision: tallyline_reporting::api::model::StoredStockDecision {
                    confirmation: row
                        .stock_decision
                        .confirmation
                        .map(|(value, deviation, environment)| {
                            Ok((
                                value.parse().map_err(|_| RepositoryError::Invalid)?,
                                deviation.into(),
                                environment.into(),
                            ))
                        })
                        .transpose()?,
                    deviation: row.stock_decision.deviation.map(Into::into),
                    unreviewed: row.stock_decision.unreviewed,
                    revoked: row.stock_decision.revoked,
                },
                status_confirmation: row.status_confirmation.map(
                    |(value, deviation, environment)| (value, deviation.into(), environment.into()),
                ),
                status_deviation: row.status_deviation.map(Into::into),
                status_unreviewed: row.status_unreviewed,
                status_confirmation_revoked: row.status_confirmation_revoked,
            })
        })
        .collect::<Result<Vec<_>, RepositoryError>>()?;
    Report::reconstitute(StoredReport {
        lifecycle: document.lifecycle.restore()?,
        id,
        key,
        observed_at,
        game_version: document.game_version,
        faction_affinity: document
            .faction_affinity
            .map(tallyline_shared_kernel::api::model::FactionAffinity::new)
            .transpose()
            .map_err(|_| RepositoryError::Invalid)?,
        version,
        rows,
        attachment: document
            .attachment
            .map(AttachmentDocument::restore)
            .transpose()?,
    })
    .map_err(|_| RepositoryError::Invalid)
}
pub(crate) fn environment_name(
    environment: tallyline_game::api::model::GameEnvironment,
) -> &'static str {
    use tallyline_game::api::model::GameEnvironment;
    match environment {
        GameEnvironment::Live => "live",
        GameEnvironment::Hotfix => "hotfix",
        GameEnvironment::Ptu => "ptu",
        GameEnvironment::Eptu => "eptu",
        GameEnvironment::TechPreview => "tech-preview",
    }
}
pub(crate) fn parse_environment(
    value: &str,
) -> Result<tallyline_game::api::model::GameEnvironment, RepositoryError> {
    use tallyline_game::api::model::GameEnvironment;
    match value {
        "live" => Ok(GameEnvironment::Live),
        "hotfix" => Ok(GameEnvironment::Hotfix),
        "ptu" => Ok(GameEnvironment::Ptu),
        "eptu" => Ok(GameEnvironment::Eptu),
        "tech-preview" => Ok(GameEnvironment::TechPreview),
        _ => Err(RepositoryError::Invalid),
    }
}

fn sql_integer(value: u64) -> Result<i64, RepositoryError> {
    i64::try_from(value).map_err(|_| RepositoryError::Invalid)
}
fn domain_integer(value: i64) -> Result<u64, RepositoryError> {
    u64::try_from(value).map_err(|_| RepositoryError::Invalid)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum StoredDeviation {
    Equal,
    Minor,
    Major,
    NoReference,
}
impl From<Deviation> for StoredDeviation {
    fn from(value: Deviation) -> Self {
        match value {
            Deviation::Equal => Self::Equal,
            Deviation::Minor => Self::Minor,
            Deviation::Major => Self::Major,
            Deviation::NoReference => Self::NoReference,
        }
    }
}
impl From<StoredDeviation> for Deviation {
    fn from(value: StoredDeviation) -> Self {
        match value {
            StoredDeviation::Equal => Self::Equal,
            StoredDeviation::Minor => Self::Minor,
            StoredDeviation::Major => Self::Major,
            StoredDeviation::NoReference => Self::NoReference,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum StoredEnvironment {
    Live,
    Ptu,
}
impl From<UexEnvironment> for StoredEnvironment {
    fn from(value: UexEnvironment) -> Self {
        match value {
            UexEnvironment::Live => Self::Live,
            UexEnvironment::Ptu => Self::Ptu,
        }
    }
}
impl From<StoredEnvironment> for UexEnvironment {
    fn from(value: StoredEnvironment) -> Self {
        match value {
            StoredEnvironment::Live => Self::Live,
            StoredEnvironment::Ptu => Self::Ptu,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentDocument {
    image: [u8; 32],
    rectangle: [u32; 4],
    observed_at: String,
    time_source: TimeSource,
    preview_confirmed: bool,
    working_copy: Option<[u8; 32]>,
    #[serde(default)]
    working_copy_owner: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TimeSource {
    ImportedFile,
    User,
}
impl From<tallyline_reporting::api::model::ManualAttachment> for AttachmentDocument {
    fn from(value: tallyline_reporting::api::model::ManualAttachment) -> Self {
        use tallyline_reporting::api::model::AttachmentTime;
        Self {
            image: value.binding().image().bytes(),
            rectangle: value.binding().region().rectangle(),
            observed_at: value.time().at().millis().to_string(),
            time_source: match value.time() {
                AttachmentTime::ImportedFile(_) => TimeSource::ImportedFile,
                AttachmentTime::User(_) => TimeSource::User,
            },
            preview_confirmed: value.preview_confirmed(),
            working_copy_owner: value.working_copy_owner().map(|owner| owner.0),
            working_copy: value
                .working_copy()
                .map(tallyline_reporting::api::model::ImageHash::bytes),
        }
    }
}
impl AttachmentDocument {
    fn restore(self) -> Result<tallyline_reporting::api::model::StoredAttachment, RepositoryError> {
        use tallyline_reporting::api::model::{
            AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash, StoredAttachment,
        };
        let [x, y, width, height] = self.rectangle;
        let region =
            AttachmentRegion::new(x, y, width, height).map_err(|_| RepositoryError::Invalid)?;
        let at = crate::reference_cache::timestamp(&self.observed_at)
            .map_err(|_| RepositoryError::Invalid)?;
        Ok(StoredAttachment {
            binding: AttachmentBinding::new(ImageHash::new(self.image), region),
            time: match self.time_source {
                TimeSource::ImportedFile => AttachmentTime::ImportedFile(at),
                TimeSource::User => AttachmentTime::User(at),
            },
            preview_confirmed: self.preview_confirmed,
            working_copy: self.working_copy.map(ImageHash::new),
            working_copy_owner: self.working_copy_owner.map(ReportId),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PriceDecisionDocument {
    confirmation: Option<(String, StoredDeviation, StoredEnvironment)>,
    deviation: Option<StoredDeviation>,
    unreviewed: bool,
    revoked: bool,
}

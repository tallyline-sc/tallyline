// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual review fixtures built through the public Reporting application.

use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::{
    CachedReference, EndpointData, PricePrior, PriorProvenance, PriorValues, ReferenceSnapshot,
    TerminalPriors, UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_reporting::api::model::{ManualReview, Report, ReportId, ReportKey};
use tallyline_reporting::api::{ReportRepository, ReportingApplication, RepositoryError};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(1000)
    }
}
struct Repository(Report);
impl ReportRepository for Repository {
    fn create_manual(
        &mut self,
        _: ReportKey,
        _: Timestamp,
        _: Option<&str>,
    ) -> Result<Report, RepositoryError> {
        Err(RepositoryError::Invalid)
    }
    fn load_draft(&mut self, id: ReportId) -> Result<Option<Report>, RepositoryError> {
        Ok((self.0.id() == id).then(|| self.0.clone()))
    }
    fn drafts(&mut self) -> Result<Vec<Report>, RepositoryError> {
        Ok(vec![self.0.clone()])
    }
    fn save_draft(&mut self, report: &Report, expected: u64) -> Result<(), RepositoryError> {
        if self.0.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        self.0 = report.clone();
        Ok(())
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
pub fn review() -> Result<(ManualReview, ReferenceSnapshot), Box<dyn std::error::Error>> {
    review_prices("200", "100")
}
pub fn review_prices(
    value: &str,
    reference: &str,
) -> Result<(ManualReview, ReferenceSnapshot), Box<dyn std::error::Error>> {
    let prior = PricePrior::new(
        CommodityId(42),
        TradeSide::Buy,
        PriorValues {
            price: reference.parse()?,
            stock: "0".parse()?,
            status: std::num::NonZeroU8::new(3),
            container_sizes: vec![],
        },
        PriorProvenance {
            game_version: "4.10.2".to_owned(),
            modified_at: Timestamp::from_seconds(900),
        },
    )?;
    let snapshot = ReferenceApplication::load(
        Cache(vec![CachedReference::new(
            Timestamp::from_seconds(950),
            EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11), vec![prior])?),
        )]),
        Fixed,
    )?
    .snapshot(UexEnvironment::Live);
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live)?;
    let mut report = Report::manual(
        ReportId(1),
        key,
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )?;
    let row = report.add_commodity(CommodityId(42))?;
    report.set_price(row, Some(value.parse()?))?;
    report.set_status(row, std::num::NonZeroU8::new(5))?;
    let version = report.version();
    let mut app = ReportingApplication::new(Repository(report), Fixed);
    Ok((
        app.review_manual(ReportId(1), version, &snapshot)?,
        snapshot,
    ))
}

pub fn fonts() -> Result<tallyline_ui::api::model::FontFiles, &'static str> {
    Ok(tallyline_ui::api::model::FontFiles {
        material_symbols: include_bytes!(
            "../../assets/fonts/material-symbols/MaterialSymbolsRounded-Tallyline.ttf"
        )
        .to_vec()
        .try_into()?,
        plex_regular: include_bytes!("../../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf")
            .to_vec(),
        plex_semibold: include_bytes!("../../assets/fonts/ibm-plex/IBMPlexSans-SemiBold.ttf")
            .to_vec(),
        plex_bold: include_bytes!("../../assets/fonts/ibm-plex/IBMPlexSans-Bold.ttf").to_vec(),
        mono_regular: include_bytes!(
            "../../assets/fonts/jetbrains-mono/JetBrainsMonoNL-Regular.ttf"
        )
        .to_vec(),
        mono_bold: include_bytes!("../../assets/fonts/jetbrains-mono/JetBrainsMonoNL-Bold.ttf")
            .to_vec(),
    })
}

pub fn without_prior() -> Result<(ManualReview, ReferenceSnapshot), Box<dyn std::error::Error>> {
    let (review, _) = review()?;
    let report = review.report().clone();
    let version = report.version();
    let snapshot =
        ReferenceApplication::load(Cache(Vec::new()), Fixed)?.snapshot(UexEnvironment::Live);
    let mut app = ReportingApplication::new(Repository(report), Fixed);
    Ok((
        app.review_manual(ReportId(1), version, &snapshot)?,
        snapshot,
    ))
}

pub fn empty_snapshot(
    environment: UexEnvironment,
) -> Result<ReferenceSnapshot, Box<dyn std::error::Error>> {
    Ok(ReferenceApplication::load(Cache(Vec::new()), Fixed)?.snapshot(environment))
}

pub fn execute(
    review: ManualReview,
    command: tallyline_ui::api::model::ManualReviewCommand,
) -> Result<ManualReview, Box<dyn std::error::Error>> {
    use tallyline_reporting::api::model::ReviewField;
    use tallyline_ui::api::model::ManualReviewAction;
    let mut app = ReportingApplication::new(Repository(review.report().clone()), Fixed);
    let report = match command.action {
        ManualReviewAction::Reassess => review.report().clone(),
        ManualReviewAction::Focus(ReviewField::Price(row)) => {
            app.focus_price(command.id, command.expected, row)?
        }
        ManualReviewAction::Focus(ReviewField::Stock(row)) => {
            app.focus_stock(command.id, command.expected, row)?
        }
        ManualReviewAction::Focus(ReviewField::Status(row)) => {
            app.focus_status(command.id, command.expected, row)?
        }
        ManualReviewAction::Confirm(ReviewField::Price(row)) => {
            app.confirm_price(command.id, command.expected, row, &command.snapshot)?
        }
        ManualReviewAction::Confirm(ReviewField::Stock(row)) => {
            app.confirm_stock(command.id, command.expected, row, &command.snapshot)?
        }
        ManualReviewAction::Confirm(ReviewField::Status(row)) => {
            app.confirm_status(command.id, command.expected, row, &command.snapshot)?
        }
    };
    Ok(app.review_manual(command.id, report.version(), &command.snapshot)?)
}

pub fn harness(
    shell: tallyline_ui::api::ShellState,
) -> Result<egui_kittest::Harness<'static, tallyline_ui::api::ShellState>, Box<dyn std::error::Error>>
{
    let catalogue = tallyline_ui::api::Catalogue::english()?;
    let mut fonts = Some(fonts()?);
    Ok(egui_kittest::Harness::builder()
        .with_size(egui::vec2(1200.0, 1900.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                tallyline_ui::api::show_manual_editor(ui, state, &catalogue);
            },
            shell,
        ))
}

pub fn review_report(
    report: Report,
    snapshot: &ReferenceSnapshot,
) -> Result<ManualReview, Box<dyn std::error::Error>> {
    let id = report.id();
    let version = report.version();
    Ok(
        ReportingApplication::new(Repository(report), Fixed)
            .review_manual(id, version, snapshot)?,
    )
}

pub fn stock_review() -> Result<(ManualReview, ReferenceSnapshot), Box<dyn std::error::Error>> {
    let (review, snapshot) = review()?;
    let mut report = review.report().clone();
    let row = report
        .rows()
        .first()
        .ok_or("missing stock fixture row")?
        .id();
    report.set_stock(row, Some("100".parse()?))?;
    let version = report.version();
    let mut app = ReportingApplication::new(Repository(report), Fixed);
    Ok((
        app.review_manual(ReportId(1), version, &snapshot)?,
        snapshot,
    ))
}

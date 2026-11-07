// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn atomic manual edits and request-owned receipts.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, EndpointData, PricePrior, PriorProvenance,
    PriorValues, Terminal, TerminalPriors, TerminalText, UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_reporting::api::RepositoryError;
use tallyline_reporting::api::model::{Report, ReportId, ReportKey};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
use tallyline_ui::api::model::{BackgroundEvent, ManualCommand};
use tallyline_ui::api::{Catalogue, ShellState, show_manual_editor};

pub(super) struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(120)
    }
}
pub(super) struct Cache(pub(super) &'static str);
impl ReferenceDataCache for Cache {
    type Error = Box<dyn std::error::Error>;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        let terminal = |id| {
            Terminal::new(
                TerminalId(id),
                TerminalText {
                    name: format!("Depot {id}"),
                    nickname: "DP".to_owned(),
                    display_name: "Shop".to_owned(),
                    location: "Pyro".to_owned(),
                },
                true,
            )
        };
        Ok(vec![
            EndpointData::Terminals(vec![terminal(11), terminal(12)]),
            EndpointData::Commodities(vec![Commodity::new(
                CommodityId(44),
                "Copper",
                "COP",
                true,
                CommodityAverages::new(None, None),
            )]),
            EndpointData::TerminalPriors(TerminalPriors::new(
                TerminalId(11),
                vec![PricePrior::new(
                    CommodityId(42),
                    TradeSide::Buy,
                    PriorValues {
                        price: self.0.parse()?,
                        stock: "0".parse()?,
                        status: std::num::NonZeroU8::new(3),
                        container_sizes: vec![],
                    },
                    PriorProvenance {
                        game_version: "4.10.2".to_owned(),
                        modified_at: Timestamp::from_seconds(100),
                    },
                )?],
            )?),
        ]
        .into_iter()
        .map(|data| CachedReference::new(Timestamp::from_seconds(100), data))
        .collect())
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(super) fn harness(
    order: &[u32],
) -> Result<Harness<'static, ShellState>, Box<dyn std::error::Error>> {
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live)?;
    let mut report = Report::manual(ReportId(1), key, Timestamp::from_seconds(100), None)?;
    for commodity in order {
        report.add_commodity(CommodityId(*commodity))?;
    }
    let mut other = Report::manual(ReportId(2), key, Timestamp::from_seconds(100), None)?;
    other.add_commodity(CommodityId(99))?;
    let mut state = ShellState::default();
    state.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report, other],
        selected: Some(ReportId(1)),
    });
    let reference = ReferenceApplication::load(Cache("100"), Fixed)?;
    state.receive(BackgroundEvent::ReferenceUpdated {
        live: reference.snapshot(UexEnvironment::Live),
        ptu: reference.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(120),
    });
    let catalogue = Catalogue::english()?;
    let mut fonts = Some(crate::font_files()?);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 1500.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_manual_editor(ui, state, &catalogue);
            },
            state,
        );
    harness.run();
    Ok(harness)
}

pub(super) fn price_label(commodity: u32) -> String {
    format!("Price per SCU for commodity {commodity} (aUEC/SCU)")
}
pub(super) fn enter(harness: &mut Harness<'_, ShellState>, commodity: u32, text: &str) {
    let label = price_label(commodity);
    harness
        .get_by_role_and_label(egui::Role::TextInput, &label)
        .focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::TextInput, &label)
        .type_text(text);
    harness.run();
}
pub(super) fn value(
    harness: &Harness<'_, ShellState>,
    commodity: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    harness
        .get_by_role_and_label(egui::Role::TextInput, &price_label(commodity))
        .accesskit_node()
        .value()
        .ok_or_else(|| "input has no accessible value".into())
}
pub(super) fn result(
    state: &mut ShellState,
    command: &ManualCommand,
    result: Result<Report, RepositoryError>,
) -> Result<(), &'static str> {
    let ManualCommand::Edit {
        request,
        id,
        expected,
        ..
    } = command
    else {
        return Err("expected an edit");
    };
    state.receive(BackgroundEvent::ReportEditFinished {
        request: *request,
        id: *id,
        expected: *expected,
        result: result.map(Box::new),
    });
    Ok(())
}
pub(super) fn takeover(report: &mut Report) -> Result<(), Box<dyn std::error::Error>> {
    let row = report
        .rows()
        .iter()
        .find(|row| row.commodity() == CommodityId(42))
        .ok_or("takeover row is absent")?
        .id();
    report.set_price(row, Some("100".parse()?))?;
    report.set_stock(row, Some("0".parse()?))?;
    report.set_status(row, std::num::NonZeroU8::new(3))?;
    Ok(())
}

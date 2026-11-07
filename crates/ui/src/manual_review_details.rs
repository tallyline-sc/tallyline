// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared exact reference details for review and confirmation.

use crate::api::Catalogue;
use crate::number_text;
use tallyline_egui::TypeStyle;
use tallyline_reporting::api::model::{
    Deviation, PriceAssessment, PriceReferenceSource, StatusAssessment,
};
pub(crate) fn price_details(
    ui: &mut egui::Ui,
    assessment: PriceAssessment,
    catalogue: &Catalogue,
    theme: tallyline_egui::Theme,
) {
    ui.label(TypeStyle::NUMERALS.text(format!(
        "{}: {} · {}: {}",
        catalogue.text("manual.reference"),
        assessment.reference().map_or_else(
            || catalogue.text("manual.no_reference").to_owned(),
            number_text::price
        ),
        catalogue.text("review.age"),
        age(assessment.reference_age_millis(), catalogue)
    )));
    if assessment.reference_source() == Some(PriceReferenceSource::CommodityAverage) {
        ui.label(catalogue.text("review.average"));
        ui.label(catalogue.text("review.average_band"));
    }
    if assessment.reference_outdated() {
        ui.label(catalogue.text("review.outdated"));
    }
    let percent = match assessment.percentage_difference() {
        Ok(Some(percent)) => number_text::percentage(&percent),
        Ok(None) => catalogue.text("review.percentage_unavailable").to_owned(),
        Err(_) => catalogue.text("review.precision_unavailable").to_owned(),
    };
    if let Some(difference) = assessment.difference() {
        ui.label(TypeStyle::NUMERALS.text(format!(
            "{}: {} {} · {}",
            catalogue.text("review.difference"),
            number_text::difference(difference),
            catalogue.text("review.per_scu"),
            percent
        )));
    } else {
        ui.label(catalogue.text("review.no_difference"));
    }
    ui.label(TypeStyle::NUMERALS.text(format!(
        "{}: {}%",
        catalogue.text("review.tolerance"),
        assessment.tolerance().percent().get()
    )));
    match assessment.tolerance().source() {
        tallyline_reference_data::api::model::PriceToleranceSource::Uex(_) => {
            ui.label(catalogue.text("review.policy_uex"));
        }
        tallyline_reference_data::api::model::PriceToleranceSource::SettingsFallback(source) => {
            ui.label(TypeStyle::NUMERALS.text(format!(
                "{} {}",
                catalogue.text("review.policy_fallback"),
                source.verified_on()
            )));
        }
    }
    marking(ui, assessment.display_deviation(), catalogue, theme);
}
pub(crate) fn stock_details(
    ui: &mut egui::Ui,
    assessment: tallyline_reporting::api::model::StockAssessment,
    catalogue: &Catalogue,
    theme: tallyline_egui::Theme,
) {
    ui.label(TypeStyle::NUMERALS.text(format!(
        "{}: {} · {}: {}",
        catalogue.text("manual.reference"),
        assessment.reference().map_or_else(
            || catalogue.text("manual.no_reference").to_owned(),
            number_text::quantity
        ),
        catalogue.text("review.age"),
        age(assessment.reference_age_millis(), catalogue)
    )));
    if assessment.reference_outdated() {
        ui.label(catalogue.text("review.outdated"));
    }
    let percent = match assessment.percentage_difference() {
        Ok(Some(percent)) => number_text::percentage(&percent),
        Ok(None) => catalogue.text("review.percentage_unavailable").to_owned(),
        Err(_) => catalogue.text("review.precision_unavailable").to_owned(),
    };
    if let Some(difference) = assessment.difference() {
        let signed = number_text::difference(difference);
        ui.label(TypeStyle::NUMERALS.text(format!(
            "{}: {} {} · {}",
            catalogue.text("review.difference"),
            signed,
            catalogue.text("review.scu"),
            percent
        )));
    } else {
        ui.label(catalogue.text("review.no_difference"));
    }
    ui.label(TypeStyle::NUMERALS.text(format!(
        "{}: {}% {} {} {}",
        catalogue.text("review.stock_tolerance"),
        assessment.tolerance().percent().get(),
        catalogue.text("review.and"),
        number_text::quantity(assessment.tolerance().minimum()),
        catalogue.text("review.scu")
    )));
    ui.label(catalogue.text("review.policy_scu"));
    marking(ui, assessment.display_deviation(), catalogue, theme);
}
pub(crate) fn status_details(
    ui: &mut egui::Ui,
    assessment: StatusAssessment,
    catalogue: &Catalogue,
    theme: tallyline_egui::Theme,
) {
    ui.label(format!(
        "{}: {} · {}: {}",
        catalogue.text("manual.reference"),
        assessment.reference().map_or_else(
            || catalogue.text("manual.no_reference").to_owned(),
            |status| status.to_string()
        ),
        catalogue.text("review.age"),
        age(assessment.reference_age_millis(), catalogue)
    ));
    if assessment.reference_outdated() {
        ui.label(catalogue.text("review.outdated"));
    }
    marking(ui, Some(assessment.display_deviation()), catalogue, theme);
}
fn age(millis: Option<u128>, catalogue: &Catalogue) -> String {
    millis.map_or_else(
        || catalogue.text("review.age_unknown").to_owned(),
        |millis| format!("{} {}", millis / 1000, catalogue.text("reference.seconds")),
    )
}
fn marking(
    ui: &mut egui::Ui,
    deviation: Option<Deviation>,
    catalogue: &Catalogue,
    theme: tallyline_egui::Theme,
) {
    ui.horizontal_wrapped(|ui| {
        let icon = match deviation {
            Some(Deviation::Major) => Some(tallyline_egui::Icon::Help),
            Some(Deviation::Minor) => Some(tallyline_egui::Icon::CompareArrows),
            Some(Deviation::NoReference) => Some(tallyline_egui::Icon::FiberNew),
            Some(Deviation::Equal) => None,
            None => Some(tallyline_egui::Icon::CrisisAlert),
        };
        if let Some(icon) = icon {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
            icon.paint(ui.painter(), rect, ui.visuals().text_color());
        }
        match deviation {
            Some(Deviation::Major) => {
                ui.add(tallyline_egui::DeltaBadge {
                    text: catalogue.text("review.major"),
                    major: true,
                    theme,
                });
            }
            Some(Deviation::Minor) => {
                ui.add(tallyline_egui::DeltaBadge {
                    text: catalogue.text("review.minor"),
                    major: false,
                    theme,
                });
            }
            Some(Deviation::NoReference) => {
                let palette = match theme {
                    tallyline_egui::Theme::Dark => tallyline_egui::tokens::DARK,
                    tallyline_egui::Theme::Light => tallyline_egui::tokens::LIGHT,
                    tallyline_egui::Theme::HighContrast => tallyline_egui::tokens::HIGH_CONTRAST,
                };
                ui.add(tallyline_egui::Chip {
                    text: catalogue.text("manual.no_reference"),
                    foreground: palette
                        .color("no-reference-fg")
                        .unwrap_or_else(|| ui.visuals().text_color()),
                    background: palette
                        .color("no-reference-bg")
                        .unwrap_or(egui::Color32::TRANSPARENT),
                });
            }
            Some(Deviation::Equal) => {
                ui.label(catalogue.text("review.equal"));
            }
            None => {
                ui.label(catalogue.text("review.precision_unavailable"));
            }
        }
    });
}

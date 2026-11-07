// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless wgpu pixels compared with the unchanged pinned legal logo.

#[path = "resources/common.rs"]
mod resources;

use egui_kittest::{Harness, kittest::Queryable};
use tallyline_ui::api::model::FAN_KIT_LOGO_ALT_TEXT;
use tallyline_ui::api::{Catalogue, FanKitUnit, configure_fonts, show_about_view};

const CHANNEL_TOLERANCE: u8 = 8;
const LOGO_FILE: &[u8] = include_bytes!("../../../docs/images/fankit/made-by-the-community.png");

#[derive(Clone, Copy)]
enum Orientation {
    Unchanged,
    Mirrored,
}

fn software_setup() -> egui_wgpu::WgpuSetup {
    use egui_wgpu::wgpu;
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = if cfg!(windows) {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::VULKAN
    };
    let required_backend = if cfg!(windows) {
        wgpu::Backend::Dx12
    } else {
        wgpu::Backend::Vulkan
    };
    let required_name = if cfg!(windows) {
        "microsoft basic render driver"
    } else {
        "llvmpipe"
    };
    setup.native_adapter_selector = Some(std::sync::Arc::new(move |adapters, _surface| {
        adapters
            .iter()
            .find(|adapter| {
                let info = adapter.get_info();
                info.device_type == wgpu::DeviceType::Cpu
                    && info.backend == required_backend
                    && info.name.to_ascii_lowercase().contains(required_name)
            })
            .cloned()
            .ok_or_else(|| format!("required CPU software renderer {required_name} is unavailable"))
    }));
    egui_wgpu::WgpuSetup::CreateNew(setup)
}
fn pixel_contrast(first: [u8; 4], second: [u8; 4]) -> f64 {
    let luminance = |pixel: [u8; 4]| {
        pixel
            .iter()
            .take(3)
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(&channel, weight)| {
                let value = f64::from(channel) / 255.0;
                let linear = if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                };
                linear * weight
            })
            .sum::<f64>()
    };
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

fn logo_difference(orientation: Orientation) -> Result<(u8, usize), String> {
    let expected = resources::decode_logo(LOGO_FILE).map_err(|error| error.to_string())?;
    let mut pixels = expected.clone();
    let pixels = match orientation {
        Orientation::Unchanged => pixels,
        Orientation::Mirrored => {
            for row in pixels
                .pixels
                .chunks_exact_mut(pixels.size.first().copied().ok_or("logo has no width")?)
            {
                row.reverse();
            }
            pixels
        }
    };
    let fonts = resources::font_files()?;
    let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
    let mut resources = Some((fonts, pixels));
    let mut unit = None;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 900.0))
        .with_pixels_per_point(256.0 / 96.0)
        .wgpu_setup(software_setup())
        .build_ui(move |ui| {
            if let Some((fonts, pixels)) = resources.take() {
                configure_fonts(ui.ctx(), fonts);
                unit = Some(FanKitUnit::new(ui.ctx(), &catalogue, pixels));
            }
            if let Some(unit) = &unit {
                let _actions = show_about_view(ui, &catalogue, unit);
            }
        });
    harness.run();
    let image = harness.render()?;
    let scale = harness.ctx.pixels_per_point();
    let rect = harness.get_by_label(FAN_KIT_LOGO_ALT_TEXT).rect();
    let x0 = (rect.min.x * scale).round() as u32;
    let y0 = (rect.min.y * scale).round() as u32;
    let background = image
        .get_pixel_checked(
            ((rect.max.x + 40.0) * scale) as u32,
            (rect.center().y * scale) as u32,
        )
        .ok_or("background sample is outside the frame")?
        .0;
    if background.get(3).copied() != Some(255) {
        return Err("background sample is not opaque".to_owned());
    }
    let width = expected.size.first().copied().ok_or("logo has no width")?;
    let mut worst = 0;
    let mut differing = 0;
    for (index, pixel) in expected.pixels.iter().enumerate() {
        let [r, g, b, a] = pixel.to_srgba_unmultiplied();
        let alpha = f32::from(a) / 255.0;
        let seen = image
            .get_pixel_checked(x0 + (index % width) as u32, y0 + (index / width) as u32)
            .ok_or("logo is outside the frame")?
            .0;
        let difference = [r, g, b]
            .iter()
            .zip(background.iter())
            .zip(seen.iter())
            .map(|((&colour, &background), &seen)| {
                let expected = (f32::from(colour) * alpha + f32::from(background) * (1.0 - alpha))
                    .round() as u8;
                expected.abs_diff(seen)
            })
            .max()
            .unwrap_or(0);
        worst = worst.max(difference);
        differing += usize::from(difference > CHANNEL_TOLERANCE);
    }
    Ok((worst, differing))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_19_q_app__wgpu_preserves_the_pinned_logo_pixels() -> Result<(), String> {
    let (worst, differing) = logo_difference(Orientation::Unchanged)?;
    assert_eq!(differing, 0, "worst channel difference {worst}");
    Ok(())
}

#[test]
fn a_mirrored_logo_fails_the_independent_pixel_oracle() -> Result<(), String> {
    let (worst, differing) = logo_difference(Orientation::Mirrored)?;
    assert!(
        differing > 1000,
        "only {differing} pixels differ; worst channel difference {worst}"
    );
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_7_q_theme__native_root_transitions_paint_readable_surfaces() -> Result<(), String> {
    use eframe::App;
    use tallyline_ui::api::DesktopShell;
    use tallyline_ui::api::model::BackgroundEvent;
    use tallyline_workflows::api::model::{AppSettings, Theme};
    let (events_tx, events_rx) = std::sync::mpsc::sync_channel(16);
    let (commands_tx, _commands_rx) = std::sync::mpsc::sync_channel(1);
    let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
    let fonts = resources::font_files()?;
    let pixels = resources::decode_logo(LOGO_FILE).map_err(|error| error.to_string())?;
    let mut resources = Some((fonts, pixels, commands_tx, events_rx, catalogue));
    let mut shell = None;
    let mut frame = eframe::Frame::_new_kittest();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 900.0))
        .wgpu_setup(software_setup())
        .build_ui(move |ui| {
            if let Some((fonts, pixels, commands, events, catalogue)) = resources.take() {
                configure_fonts(ui.ctx(), fonts);
                shell = Some(DesktopShell::new(
                    ui.ctx(),
                    catalogue,
                    pixels.clone(),
                    pixels,
                    commands,
                    events,
                ));
            }
            if let Some(shell) = &mut shell {
                shell.logic(ui.ctx(), &mut frame);
                let background = ui.painter().add(egui::Shape::Noop);
                shell.ui(ui, &mut frame);
                let clear = shell.clear_color(&ui.ctx().global_style().visuals);
                let colour = egui::Color32::from_rgb(
                    (clear[0] * 255.0).round() as u8,
                    (clear[1] * 255.0).round() as u8,
                    (clear[2] * 255.0).round() as u8,
                );
                ui.painter().set(
                    background,
                    egui::Shape::rect_filled(ui.max_rect(), 0.0, colour),
                );
            }
        });
    for theme in [Theme::Dark, Theme::Light, Theme::Dark] {
        events_tx
            .send(BackgroundEvent::SettingsLoaded(
                AppSettings::from_persisted(theme, true),
            ))
            .map_err(|error| error.to_string())?;
        harness.run();
        let image = harness.render()?;
        let background = image.get_pixel(600, 700).0;
        let foreground = harness.ctx.global_style().visuals.text_color();
        let rect = harness.get_by_label("Workspace").rect();
        let scale = harness.ctx.pixels_per_point();
        let mut painted_text_contrast = 1.0_f64;
        for y in (rect.min.y * scale) as u32..(rect.max.y * scale) as u32 {
            for x in (rect.min.x * scale) as u32..(rect.max.x * scale) as u32 {
                if let Some(pixel) = image.get_pixel_checked(x, y) {
                    painted_text_contrast =
                        painted_text_contrast.max(pixel_contrast(pixel.0, background));
                }
            }
        }
        assert!(
            painted_text_contrast >= 4.5,
            "{theme:?}: foreground pixels have insufficient contrast against {background:?}"
        );
        let delta = foreground.r().abs_diff(background[0]);
        assert!(
            delta > 100,
            "{theme:?}: rendered background {background:?} conflicts with text {foreground:?}"
        );
        if theme == Theme::Light {
            assert!(
                background[0] > 200,
                "Light native root stayed dark: {background:?}"
            );
        } else {
            assert!(
                background[0] < 70,
                "Dark native root stayed light: {background:?}"
            );
        }
    }
    Ok(())
}

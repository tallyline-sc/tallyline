// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Paint, accessibility and texture assertions for the coupled legal unit.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::resources::decode_logo;
use egui::accesskit::Role;
use egui::epaint::{ImageData, Shape, TextShape, TextureId, textures::TexturesDelta};
use egui::{Color32, ColorImage, Pos2, Rect};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, TestRenderer};
use sha2::{Digest, Sha256};
use tallyline_ui::api::model::{FAN_KIT_LOGO_ALT_TEXT, FAN_KIT_LOGO_TEXTURE_NAME};

const FAN_KIT_TRADEMARK_LINE: &str = "Star Citizen®, Roberts Space Industries® and Cloud Imperium ® are registered trademarks of Cloud Imperium Rights LLC";
const FAN_KIT_AGREEMENT_NOTICE: &str = "This site is not endorsed by or affiliated with the Cloud Imperium or Roberts Space Industries group of companies. All game content and materials are copyright Cloud Imperium Rights LLC and Cloud Imperium Rights Ltd.. Star Citizen®, Squadron 42®, Roberts Space Industries®, and Cloud Imperium® are registered trademarks of Cloud Imperium Rights LLC. All rights reserved.";

/// SHA-256 of `made-by-the-community.png` as pinned in `NOTICE` §4.
pub const LOGO_FILE_SHA256: &str =
    "b6869016da2220a61dc6306f8365dcce006ddb6d8b405545c6d4540166469b6b";

/// SHA-256 of the §2b trademark line.
pub const TRADEMARK_LINE_SHA256: &str =
    "154deedbb183aeec2ba7f05c3d16416e55ceee634a36ec1f9ee4785b26467fa1";

/// SHA-256 of the clause 2(g) notice.
pub const AGREEMENT_NOTICE_SHA256: &str =
    "703b53f047769fe1a5299b53454cc79a36e2b3a09129291baf2c6948ce740933";

/// 10 typographic points in egui points at a scale of 100 % (96 logical pixels per inch).
pub const MIN_NOTICE_FONT_SIZE: f32 = 10.0 * 96.0 / 72.0;

/// The largest gap between neighbouring parts of the unit in egui points.
pub const MAX_PART_GAP: f32 = 16.0;

/// The smallest contrast ratio of a notice against its background (WCAG 2 AA for body text).
pub const MIN_CONTRAST: f32 = 4.5;

const UV_TOLERANCE: f32 = 1e-4;

/// Lower-case hex SHA-256 of the bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Lower-case hex SHA-256 of the RGBA bytes of an egui image.
pub fn pixels_sha256(image: &ColorImage) -> String {
    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_array())
        .collect();
    format!(
        "{}x{}:{}",
        image.size.first().unwrap_or(&0),
        image.size.get(1).unwrap_or(&0),
        sha256_hex(&bytes)
    )
}

/// The logo file read from the repository at test time, independent of the embedded constant.
pub fn logo_file_on_disk() -> Result<Vec<u8>, String> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/images/fankit/made-by-the-community.png"
    ))
    .map_err(|error| error.to_string())
}

/// The full textures the harness uploaded, by texture id.
#[derive(Clone, Default)]
pub struct RecordedTextures(Rc<RefCell<HashMap<TextureId, ColorImage>>>);

impl RecordedTextures {
    /// The recorded image of a texture, if it was uploaded.
    pub fn get(&self, id: TextureId) -> Option<ColorImage> {
        self.0.borrow().get(&id).cloned()
    }
}

/// A headless renderer that keeps every texture upload instead of drawing.
pub struct RecordingRenderer(pub RecordedTextures);

impl TestRenderer for RecordingRenderer {
    fn handle_delta(&mut self, delta: &mut TexturesDelta) {
        let mut textures = self.0.0.borrow_mut();
        for (id, image_deltas) in delta.set.drain() {
            for image_delta in image_deltas {
                let ImageData::Color(image) = image_delta.image;
                match image_delta.pos {
                    None => {
                        textures.insert(id, (*image).clone());
                    }
                    Some(_) => {
                        textures.remove(&id);
                    }
                }
            }
        }
        for id in delta.free.drain() {
            textures.remove(&id);
        }
    }

    fn render(
        &mut self,
        _ctx: &egui::Context,
        _output: &egui::FullOutput,
    ) -> Result<image::RgbaImage, String> {
        Err("the recording renderer does not draw".to_owned())
    }
}

struct Painted<'a> {
    clip: Rect,
    shape: &'a Shape,
}

fn flatten<'a>(clip: Rect, shape: &'a Shape, out: &mut Vec<Painted<'a>>) {
    if let Shape::Vec(shapes) = shape {
        for inner in shapes {
            flatten(clip, inner, out);
        }
    } else {
        out.push(Painted { clip, shape });
    }
}

fn painted_shapes<'h, State>(harness: &'h Harness<'_, State>) -> Vec<Painted<'h>> {
    let mut out = Vec::new();
    for clipped in &harness.output().shapes {
        flatten(clipped.clip_rect, &clipped.shape, &mut out);
    }
    out
}

struct LogoPaint {
    index: usize,
    clip: Rect,
    texture: TextureId,
    rect: Rect,
}

fn relative_luminance(colour: Color32) -> f32 {
    let channel = |value: u8| {
        let c = f32::from(value) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
}

fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn over(foreground: Color32, opacity: f32, background: Color32) -> Color32 {
    let alpha = f32::from(foreground.to_srgba_unmultiplied()[3]) / 255.0 * opacity;
    let [fr, fg, fb, _] = foreground.to_srgba_unmultiplied();
    let mix = |f: u8, b: u8| (f32::from(f) * alpha + f32::from(b) * (1.0 - alpha)).round() as u8;
    Color32::from_rgb(
        mix(fr, background.r()),
        mix(fg, background.g()),
        mix(fb, background.b()),
    )
}

fn text_colours(text: &TextShape) -> Vec<Color32> {
    text.galley
        .job
        .sections
        .iter()
        .map(|section| {
            text.override_text_color
                .unwrap_or(if section.format.color == Color32::PLACEHOLDER {
                    text.fallback_color
                } else {
                    section.format.color
                })
        })
        .collect()
}

fn background_behind(
    shapes: &[Painted<'_>],
    index: usize,
    area: Rect,
    fallback: Color32,
) -> Color32 {
    shapes
        .iter()
        .take(index)
        .rev()
        .find_map(|painted| match painted.shape {
            Shape::Rect(rect) if rect.fill.a() == 255 && rect.rect.contains_rect(area) => {
                Some(rect.fill)
            }
            Shape::Noop
            | Shape::Vec(_)
            | Shape::Band(_)
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Rect(_)
            | Shape::Text(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => None,
        })
        .unwrap_or(fallback)
}

fn gap(a: Rect, b: Rect) -> f32 {
    let dx = (a.min.x - b.max.x).max(b.min.x - a.max.x).max(0.0);
    let dy = (a.min.y - b.max.y).max(b.min.y - a.max.y).max(0.0);
    dx.max(dy)
}

fn occluder(shapes: &[Painted<'_>], index: usize, area: Rect) -> Option<Rect> {
    shapes.iter().skip(index + 1).find_map(|painted| {
        let bounds = painted.shape.visual_bounding_rect().intersect(painted.clip);
        (bounds.is_positive() && bounds.intersects(area)).then_some(bounds)
    })
}

/// Checks that the Fan Kit unit is shown completely, byte-exact, legible and together in the current frame.
///
/// # Errors
/// Returns the first deviation found, as a message for the assertion.
pub fn check_fan_kit_unit<State>(
    harness: &Harness<'_, State>,
    textures: &RecordedTextures,
) -> Result<(), String> {
    let screen = harness.ctx.content_rect();
    let zoom = harness.ctx.zoom_factor();
    let panel_fill = harness.ctx.style_of(harness.ctx.theme()).visuals.panel_fill;

    let trademark = single_node(harness, FAN_KIT_TRADEMARK_LINE, "the trademark line")?;
    let agreement = single_node(harness, FAN_KIT_AGREEMENT_NOTICE, "the agreement notice")?;
    let logo = single_node(harness, FAN_KIT_LOGO_ALT_TEXT, "the logo")?;

    if logo.accesskit_node().role() != Role::Image {
        return Err(format!(
            "the logo node has role {:?}",
            logo.accesskit_node().role()
        ));
    }
    for (name, node) in [
        ("trademark line", &trademark),
        ("agreement notice", &agreement),
    ] {
        if node.accesskit_node().role() != Role::Label {
            return Err(format!(
                "the {name} node has role {:?}",
                node.accesskit_node().role()
            ));
        }
    }

    let parents: Vec<_> = [&logo, &trademark, &agreement]
        .iter()
        .map(|node| node.accesskit_node().parent_id())
        .collect();
    if parents
        .iter()
        .any(|parent| *parent != parents.first().copied().flatten())
    {
        return Err(format!(
            "the three parts have different parents: {parents:?}"
        ));
    }
    let siblings = logo
        .accesskit_node()
        .parent()
        .map_or(0, |parent| parent.children().count());
    if siblings != 3 {
        return Err(format!(
            "the group of the three parts holds {siblings} nodes, not only the unit"
        ));
    }

    let parts = [
        ("logo", logo.rect()),
        ("trademark line", trademark.rect()),
        ("agreement notice", agreement.rect()),
    ];
    for (name, rect) in parts {
        if !screen.contains_rect(rect) {
            return Err(format!(
                "the {name} at {rect:?} is not inside the screen {screen:?}"
            ));
        }
    }
    for pair in parts.windows(2) {
        let [(first, a), (second, b)] = pair else {
            continue;
        };
        let distance = gap(*a, *b);
        if distance > MAX_PART_GAP {
            return Err(format!(
                "the {first} and the {second} are {distance} points apart"
            ));
        }
    }

    let shapes = painted_shapes(harness);

    for (name, text) in [
        ("trademark line", FAN_KIT_TRADEMARK_LINE),
        ("agreement notice", FAN_KIT_AGREEMENT_NOTICE),
    ] {
        let painted: Vec<_> = shapes
            .iter()
            .enumerate()
            .filter_map(|(index, painted)| match painted.shape {
                Shape::Text(text_shape) if text_shape.galley.job.text == text => {
                    Some((index, painted.clip, text_shape))
                }
                Shape::Noop
                | Shape::Vec(_)
                | Shape::Band(_)
                | Shape::Circle(_)
                | Shape::Ellipse(_)
                | Shape::LineSegment { .. }
                | Shape::Path(_)
                | Shape::Rect(_)
                | Shape::Text(_)
                | Shape::Mesh(_)
                | Shape::QuadraticBezier(_)
                | Shape::CubicBezier(_)
                | Shape::Callback(_) => None,
            })
            .collect();
        let [(index, clip, text_shape)] = painted.as_slice() else {
            return Err(format!(
                "the {name} is painted {} times as one text",
                painted.len()
            ));
        };
        if text_shape.galley.elided {
            return Err(format!("the {name} is truncated"));
        }
        let bounds = text_shape.visual_bounding_rect();
        if !clip.contains_rect(bounds) {
            return Err(format!("the {name} at {bounds:?} is clipped by {clip:?}"));
        }
        for section in &text_shape.galley.job.sections {
            let body_size = egui::TextStyle::Body
                .resolve(&harness.ctx.global_style())
                .size;
            if section.format.font_id.size < body_size {
                return Err(format!("the {name} is smaller than body text"));
            }
            let size = section.format.font_id.size * zoom;
            if size < MIN_NOTICE_FONT_SIZE {
                return Err(format!(
                    "the {name} uses font size {size} egui points at zoom {zoom}"
                ));
            }
        }
        let background = background_behind(&shapes, *index, bounds, panel_fill);
        for colour in text_colours(text_shape) {
            let seen = over(colour, text_shape.opacity_factor, background);
            let ratio = contrast(seen, background);
            if ratio < MIN_CONTRAST {
                return Err(format!(
                    "the {name} has contrast {ratio:.2} ({seen:?} on {background:?})"
                ));
            }
        }
        if let Some(cover) = occluder(&shapes, *index, bounds) {
            return Err(format!("the {name} is covered by a shape at {cover:?}"));
        }
    }

    let manager = harness.ctx.tex_manager();
    let is_logo = |id: TextureId| {
        manager
            .read()
            .meta(id)
            .is_some_and(|meta| meta.name == FAN_KIT_LOGO_TEXTURE_NAME)
    };
    let full_uv = Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0));
    let mut logo_paints = Vec::new();
    for (index, painted) in shapes.iter().enumerate() {
        match painted.shape {
            Shape::Rect(rect) => {
                let Some(brush) = rect.brush.as_ref() else {
                    continue;
                };
                if !is_logo(brush.fill_texture_id) {
                    continue;
                }
                if rect.fill != Color32::WHITE {
                    return Err(format!("the logo is tinted {:?}", rect.fill));
                }
                if brush.uv != full_uv {
                    return Err(format!(
                        "the logo is cropped or flipped by uv {:?}",
                        brush.uv
                    ));
                }
                logo_paints.push(LogoPaint {
                    index,
                    clip: painted.clip,
                    texture: brush.fill_texture_id,
                    rect: rect.rect,
                });
            }
            Shape::Mesh(mesh) if is_logo(mesh.texture_id) => {
                let bounds = mesh.calc_bounds();
                for vertex in &mesh.vertices {
                    if vertex.color != Color32::WHITE {
                        return Err(format!("the logo is tinted {:?}", vertex.color));
                    }
                    let expected = ((vertex.pos - bounds.min) / bounds.size()).to_pos2();
                    if (vertex.uv - expected).length() > UV_TOLERANCE {
                        return Err(format!(
                            "the logo is rotated, flipped or cropped: vertex {:?} has uv {:?}, expected {expected:?}",
                            vertex.pos, vertex.uv
                        ));
                    }
                }
                logo_paints.push(LogoPaint {
                    index,
                    clip: painted.clip,
                    texture: mesh.texture_id,
                    rect: bounds,
                });
            }
            Shape::Noop
            | Shape::Vec(_)
            | Shape::Band(_)
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Text(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => continue,
        }
    }
    let [paint] = logo_paints.as_slice() else {
        return Err(format!(
            "the logo texture is painted {} times",
            logo_paints.len()
        ));
    };
    if !paint.clip.contains_rect(paint.rect) || !screen.contains_rect(paint.rect) {
        return Err(format!(
            "the logo at {:?} is clipped or off screen",
            paint.rect
        ));
    }
    if !paint.rect.contains(logo.rect().center()) {
        return Err("the painted logo is not where the logo node is".to_owned());
    }
    let aspect = paint.rect.width() / paint.rect.height();
    if (aspect - 1.0).abs() > f32::EPSILON {
        return Err(format!("the logo is distorted to aspect ratio {aspect}"));
    }
    if let Some(cover) = occluder(&shapes, paint.index, paint.rect) {
        return Err(format!("the logo is covered by a shape at {cover:?}"));
    }
    let Some(uploaded) = textures.get(paint.texture) else {
        return Err("the logo texture was never uploaded in full".to_owned());
    };
    let (file, expected_sha) = if uploaded.size == [1418, 1418] {
        (
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/images/fankit/MadeByTheCommunity_White.png"
            ))
            .map_err(|error| error.to_string())?,
            "1a146fa257b648baa58840fe17149b7cce06c446d8861586a4749d813372a50b",
        )
    } else {
        (logo_file_on_disk()?, LOGO_FILE_SHA256)
    };
    if sha256_hex(&file) != expected_sha {
        return Err("the logo file on disk does not have the pinned SHA-256".to_owned());
    }
    let expected = decode_logo(&file).map_err(|error| error.to_string())?;
    if pixels_sha256(&uploaded) != pixels_sha256(&expected) {
        return Err(format!(
            "the uploaded logo pixels {} differ from the pinned file {}",
            pixels_sha256(&uploaded),
            pixels_sha256(&expected)
        ));
    }

    Ok(())
}

/// Activates every interactive node outside the unit, one at a time, and checks the unit after each.
///
/// # Errors
/// Returns the first control whose activation hides or breaks the unit.
pub fn check_fan_kit_unit_survives_every_control<State>(
    harness: &mut Harness<'_, State>,
    textures: &RecordedTextures,
) -> Result<(), String> {
    check_fan_kit_unit(harness, textures)?;
    let controls: Vec<_> = harness
        .query_all_by(|node| {
            matches!(
                node.role(),
                Role::Button
                    | Role::CheckBox
                    | Role::RadioButton
                    | Role::Link
                    | Role::ComboBox
                    | Role::Tab
                    | Role::MenuItem
            )
        })
        .map(|node| {
            (
                node.accesskit_node().id(),
                node.accesskit_node().label().unwrap_or_default(),
            )
        })
        .collect();
    if controls.is_empty() {
        return Ok(());
    }
    for (id, label) in controls {
        let Some(control) = harness.query_by(|node| node.id() == id) else {
            continue;
        };
        control.click();
        harness.run();
        check_fan_kit_unit(harness, textures)
            .map_err(|error| format!("activating {label:?} breaks the unit: {error}"))?;
    }
    Ok(())
}

fn single_node<'h, State>(
    harness: &'h Harness<'_, State>,
    label: &'h str,
    name: &str,
) -> Result<egui_kittest::Node<'h>, String> {
    let mut nodes: Vec<_> = harness.query_all_by_label(label).collect();
    match nodes.len() {
        1 => Ok(nodes.remove(0)),
        count => Err(format!(
            "{name} appears {count} times in the accessibility tree"
        )),
    }
}

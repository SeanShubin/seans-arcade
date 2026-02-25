//! Interactive demo of the four current Color constructor families in Bevy 0.18.
//!
//! Run with: `cargo run --example color_constructors`
//!
//! Each section has sliders for the constructor's parameters. Drag them to see
//! how each color space behaves. The constructor call updates live.
//!
//! Layout is adaptive: two columns when the window is wide enough, one column
//! when narrow (same flex-wrap pattern as the dashboard example).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, EguiPlugin::default(), ColorConstructorsPlugin))
        .run();
}

struct ColorConstructorsPlugin;

impl Plugin for ColorConstructorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ColorSliders>()
            .add_systems(Startup, setup)
            .add_systems(EguiPrimaryContextPass, ui_system);
    }
}

#[derive(Resource)]
struct ColorSliders {
    srgb: [f32; 4],
    hsl: [f32; 4],
    oklch: [f32; 4],
    linear: [f32; 4],
}

impl Default for ColorSliders {
    fn default() -> Self {
        Self {
            srgb: [1.0, 0.5, 0.0, 1.0],
            hsl: [200.0, 0.8, 0.5, 1.0],
            oklch: [0.7, 0.2, 150.0, 1.0],
            linear: [0.2, 0.6, 1.0, 1.0],
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

// ---------------------------------------------------------------------------
// Color preview swatch with checkerboard alpha background
// ---------------------------------------------------------------------------

fn color_preview(ui: &mut egui::Ui, color: Color) {
    let srgba = color.to_srgba();
    let egui_color = egui::Color32::from_rgba_unmultiplied(
        (srgba.red.clamp(0.0, 1.0) * 255.0) as u8,
        (srgba.green.clamp(0.0, 1.0) * 255.0) as u8,
        (srgba.blue.clamp(0.0, 1.0) * 255.0) as u8,
        (srgba.alpha.clamp(0.0, 1.0) * 255.0) as u8,
    );
    let size = egui::vec2(ui.available_width(), 60.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());

    let checker_size = 10.0;
    let painter = ui.painter_at(rect);
    let cols = (rect.width() / checker_size).ceil() as usize;
    let rows = (rect.height() / checker_size).ceil() as usize;
    for row in 0..rows {
        for col in 0..cols {
            let dark = (row + col) % 2 == 0;
            let c = if dark {
                egui::Color32::from_gray(180)
            } else {
                egui::Color32::from_gray(220)
            };
            let min = rect.min
                + egui::vec2(col as f32 * checker_size, row as f32 * checker_size);
            let max = egui::pos2(
                (min.x + checker_size).min(rect.max.x),
                (min.y + checker_size).min(rect.max.y),
            );
            painter.rect_filled(egui::Rect::from_min_max(min, max), 0.0, c);
        }
    }
    painter.rect_filled(rect, 0.0, egui_color);
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::GRAY),
        egui::StrokeKind::Outside,
    );
}

// ---------------------------------------------------------------------------
// Slider with [-] / [+] step buttons
// ---------------------------------------------------------------------------

fn stepped_slider(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) {
    let min = *range.start();
    let max = *range.end();
    let step = (max - min) / 10.0;

    // Snap to the next grid line in the given direction.
    // E.g. for step=0.1: 0.37 → minus gives 0.3, plus gives 0.4.
    let snap_down = |v: f32| ((v / step).floor() * step).max(min);
    let snap_up = |v: f32| ((v / step).ceil() * step).min(max);

    ui.horizontal(|ui| {
        if ui.small_button("-").clicked() {
            let snapped = snap_down(*value);
            // If already on a grid line, go one step further
            *value = if (snapped - *value).abs() < f32::EPSILON {
                (snapped - step).max(min)
            } else {
                snapped
            };
        }
        ui.add(egui::Slider::new(value, range).text(label));
        if ui.small_button("+").clicked() {
            let snapped = snap_up(*value);
            *value = if (snapped - *value).abs() < f32::EPSILON {
                (snapped + step).min(max)
            } else {
                snapped
            };
        }
    });
}

// ---------------------------------------------------------------------------
// Individual color-space panels
// ---------------------------------------------------------------------------

fn srgb_panel(ui: &mut egui::Ui, values: &mut [f32; 4]) {
    ui.strong("Color::srgb() / srgba()");
    ui.label("Specific colors matching hex values. Most common constructor.");
    ui.add_space(4.0);
    stepped_slider(ui, &mut values[0], 0.0..=1.0, "red");
    stepped_slider(ui, &mut values[1], 0.0..=1.0, "green");
    stepped_slider(ui, &mut values[2], 0.0..=1.0, "blue");
    stepped_slider(ui, &mut values[3], 0.0..=1.0, "alpha");
    let [r, g, b, a] = *values;
    ui.monospace(format!("Color::srgba({r:.2}, {g:.2}, {b:.2}, {a:.2})"));
    color_preview(ui, Color::srgba(r, g, b, a));
}

fn hsl_panel(ui: &mut egui::Ui, values: &mut [f32; 4]) {
    ui.strong("Color::hsl() / hsla()");
    ui.label("Rotate hue, adjust saturation and lightness.");
    ui.add_space(4.0);
    stepped_slider(ui, &mut values[0], 0.0..=360.0, "hue");
    stepped_slider(ui, &mut values[1], 0.0..=1.0, "saturation");
    stepped_slider(ui, &mut values[2], 0.0..=1.0, "lightness");
    stepped_slider(ui, &mut values[3], 0.0..=1.0, "alpha");
    let [h, s, l, a] = *values;
    ui.monospace(format!("Color::hsla({h:.0}, {s:.2}, {l:.2}, {a:.2})"));
    color_preview(ui, Color::hsla(h, s, l, a));
}

fn oklch_panel(ui: &mut egui::Ui, values: &mut [f32; 4]) {
    ui.strong("Color::oklch() / oklcha()");
    ui.label("Perceptually uniform. Great for palettes and gradients.");
    ui.add_space(4.0);
    stepped_slider(ui, &mut values[0], 0.0..=1.0, "lightness");
    stepped_slider(ui, &mut values[1], 0.0..=0.4, "chroma");
    stepped_slider(ui, &mut values[2], 0.0..=360.0, "hue");
    stepped_slider(ui, &mut values[3], 0.0..=1.0, "alpha");
    let [l, c, h, a] = *values;
    ui.monospace(format!("Color::oklcha({l:.2}, {c:.3}, {h:.0}, {a:.2})"));
    color_preview(ui, Color::oklcha(l, c, h, a));
}

fn linear_rgb_panel(ui: &mut egui::Ui, values: &mut [f32; 4]) {
    ui.strong("Color::linear_rgb() / linear_rgba()");
    ui.label("Linear color space. For shader math, lighting, and blending.");
    ui.add_space(4.0);
    stepped_slider(ui, &mut values[0], 0.0..=1.0, "red");
    stepped_slider(ui, &mut values[1], 0.0..=1.0, "green");
    stepped_slider(ui, &mut values[2], 0.0..=1.0, "blue");
    stepped_slider(ui, &mut values[3], 0.0..=1.0, "alpha");
    let [r, g, b, a] = *values;
    ui.monospace(format!("Color::linear_rgba({r:.2}, {g:.2}, {b:.2}, {a:.2})"));
    color_preview(ui, Color::linear_rgba(r, g, b, a));
}

// ---------------------------------------------------------------------------
// Adaptive layout: 2 columns when wide, 1 column when narrow
// ---------------------------------------------------------------------------

const TWO_COLUMN_THRESHOLD: f32 = 700.0;

fn ui_system(mut contexts: EguiContexts, mut sliders: ResMut<ColorSliders>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.spacing_mut().slider_width = ui.spacing().slider_width * 4.0;
        ui.heading("Bevy Color Constructors");
        ui.label("Drag sliders to explore each color space.");
        ui.add_space(8.0);

        let wide = ui.available_width() >= TWO_COLUMN_THRESHOLD;

        egui::ScrollArea::vertical().show(ui, |ui| {
            if wide {
                // 2x2 grid: top row, then bottom row
                ui.columns(2, |cols| {
                    cols[0].group(|ui| srgb_panel(ui, &mut sliders.srgb));
                    cols[1].group(|ui| hsl_panel(ui, &mut sliders.hsl));
                });
                ui.add_space(12.0);
                ui.columns(2, |cols| {
                    cols[0].group(|ui| oklch_panel(ui, &mut sliders.oklch));
                    cols[1].group(|ui| linear_rgb_panel(ui, &mut sliders.linear));
                });
            } else {
                // Single column stack
                ui.group(|ui| srgb_panel(ui, &mut sliders.srgb));
                ui.add_space(12.0);
                ui.group(|ui| hsl_panel(ui, &mut sliders.hsl));
                ui.add_space(12.0);
                ui.group(|ui| oklch_panel(ui, &mut sliders.oklch));
                ui.add_space(12.0);
                ui.group(|ui| linear_rgb_panel(ui, &mut sliders.linear));
            }
        });
    });
}

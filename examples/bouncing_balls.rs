//! Bouncing balls visual toy with egui controls.
//!
//! Run with: `cargo run --example bouncing_balls`

use std::collections::VecDeque;

use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use noise::{NoiseFn, Perlin};
use rand::RngExt;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            EguiPlugin::default(),
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
        ))
        .init_resource::<BounceConfig>()
        .init_resource::<PanelWidth>()
        .init_resource::<BallActions>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (manage_balls, sync_trail_entities, sync_ball_size, move_balls, apply_noise, update_trails, sync_trail_transforms, collect_balls, scatter_balls)
                .chain()
                .run_if(|config: Res<BounceConfig>| !config.use_fixed_update),
        )
        .add_systems(
            FixedUpdate,
            (manage_balls, sync_trail_entities, sync_ball_size, move_balls, apply_noise, update_trails, sync_trail_transforms, collect_balls, scatter_balls)
                .chain()
                .run_if(|config: Res<BounceConfig>| config.use_fixed_update),
        )
        .add_systems(EguiPrimaryContextPass, ui_system)
        .run();
}

#[derive(Resource)]
struct BounceConfig {
    ball_count_exp: i32,
    ball_size_exp: i32,
    trail_count_exp: i32,
    speed_exp: i32,
    trail_gap_exp: i32,
    dir_enabled: bool,
    dir_turn_rate_exp: i32,
    dir_coherence_exp: i32,
    hue_enabled: bool,
    hue_range_exp: i32,
    hue_coherence_exp: i32,
    use_fixed_update: bool,
}

impl Default for BounceConfig {
    fn default() -> Self {
        Self {
            ball_count_exp: 0,
            ball_size_exp: 0,
            trail_count_exp: 0,
            speed_exp: 0,
            trail_gap_exp: 0,
            dir_enabled: true,
            dir_turn_rate_exp: 7,
            dir_coherence_exp: 0,
            hue_enabled: true,
            hue_range_exp: 7,
            hue_coherence_exp: 0,
            use_fixed_update: false,
        }
    }
}

impl BounceConfig {
    fn ball_count(&self) -> usize {
        2_usize.pow(self.ball_count_exp as u32)
    }

    fn ball_size(&self) -> f32 {
        2_f32.powi(self.ball_size_exp)
    }

    fn trail_count(&self) -> usize {
        2_usize.pow(self.trail_count_exp as u32)
    }

    fn speed(&self) -> f32 {
        2_f32.powi(self.speed_exp)
    }

    fn trail_gap(&self) -> usize {
        2_usize.pow(self.trail_gap_exp as u32)
    }

    /// Direction turning rate in degrees per second.
    fn dir_turn_rate(&self) -> f64 {
        2_f64.powi(self.dir_turn_rate_exp)
    }

    /// Direction coherence time in seconds.
    fn dir_coherence(&self) -> f64 {
        2_f64.powi(self.dir_coherence_exp)
    }

    /// Hue range (max deviation from base) in degrees.
    fn hue_range(&self) -> f64 {
        2_f64.powi(self.hue_range_exp)
    }

    /// Hue coherence time in seconds.
    fn hue_coherence(&self) -> f64 {
        2_f64.powi(self.hue_coherence_exp)
    }
}

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct TrailDot;

#[derive(Component)]
struct Velocity(Vec2);

#[derive(Component)]
struct TrailHistory(VecDeque<Vec2>);

#[derive(Component)]
struct BallColor(Color);

#[derive(Component)]
struct TrailEntities(Vec<Entity>);

#[derive(Component)]
struct NoiseSeeds {
    hue: f64,
    direction: f64,
}

#[derive(Component)]
struct BaseHue(f32);

#[derive(Resource, Default)]
struct PanelWidth(f32);

#[derive(Resource, Default)]
struct BallActions {
    collect: bool,
    scatter: bool,
}

#[derive(Resource)]
struct CircleTexture(Handle<Image>);

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);

    // Generate a white circle texture using distance-from-center for alpha.
    const SIZE: u32 = 32;
    let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];
    let center = (SIZE - 1) as f32 / 2.0;
    let radius = SIZE as f32 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = if dist <= radius { 255 } else { 0 };
            let i = ((y * SIZE + x) * 4) as usize;
            pixels[i] = 255;
            pixels[i + 1] = 255;
            pixels[i + 2] = 255;
            pixels[i + 3] = alpha;
        }
    }
    let image = images.add(Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ));
    commands.insert_resource(CircleTexture(image));
}

fn manage_balls(
    mut commands: Commands,
    config: Res<BounceConfig>,
    balls: Query<(Entity, &TrailEntities), With<Ball>>,
    window: Query<&Window, With<PrimaryWindow>>,
    circle_tex: Res<CircleTexture>,
) {
    let target = config.ball_count();
    let current = balls.iter().count();

    if current < target {
        let Ok(win) = window.single() else { return };
        let half_w = win.width() / 2.0;
        let half_h = win.height() / 2.0;
        let size = config.ball_size();
        let trail_count = config.trail_count();
        let mut rng = rand::rng();

        for _ in 0..(target - current) {
            let x = rng.random_range(-half_w..half_w);
            let y = rng.random_range(-half_h..half_h);
            let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
            let vel = Vec2::new(angle.cos(), angle.sin());
            let hue = rng.random_range(0.0..360.0);
            let color = Color::oklcha(0.5, 0.4, hue, 1.0);
            let base_srgba = color.to_srgba();

            let mut trail_entities = Vec::with_capacity(trail_count);
            for i in 0..trail_count {
                let alpha = (1.0 - i as f32 / trail_count as f32) * base_srgba.alpha;
                let trail_color = Color::srgba(base_srgba.red, base_srgba.green, base_srgba.blue, alpha);
                let trail_entity = commands
                    .spawn((
                        TrailDot,
                        Sprite {
                            image: circle_tex.0.clone(),
                            color: trail_color,
                            custom_size: Some(Vec2::splat(size)),
                            ..default()
                        },
                        Transform::from_xyz(x, y, -(i as f32)),
                    ))
                    .id();
                trail_entities.push(trail_entity);
            }

            let noise_seeds = NoiseSeeds {
                hue: rng.random_range(0.0..1000.0),
                direction: rng.random_range(0.0..1000.0),
            };

            commands.spawn((
                Ball,
                Sprite {
                    image: circle_tex.0.clone(),
                    color,
                    custom_size: Some(Vec2::splat(size)),
                    ..default()
                },
                Transform::from_xyz(x, y, 100.0),
                Velocity(vel),
                TrailHistory(VecDeque::new()),
                BallColor(color),
                TrailEntities(trail_entities),
                noise_seeds,
                BaseHue(hue),
            ));
        }
    } else if current > target {
        for (entity, trail_ents) in balls.iter().take(current - target) {
            for &trail_entity in &trail_ents.0 {
                commands.entity(trail_entity).despawn();
            }
            commands.entity(entity).despawn();
        }
    }
}

fn sync_trail_entities(
    mut commands: Commands,
    config: Res<BounceConfig>,
    mut balls: Query<(&Transform, &BallColor, &mut TrailEntities), With<Ball>>,
    mut trail_dots: Query<&mut Sprite, (With<TrailDot>, Without<Ball>)>,
    circle_tex: Res<CircleTexture>,
) {
    if !config.is_changed() {
        return;
    }
    let trail_count = config.trail_count();
    let size = config.ball_size();

    for (transform, ball_color, mut trail_ents) in &mut balls {
        let base_srgba = ball_color.0.to_srgba();
        let current_count = trail_ents.0.len();

        if current_count > trail_count {
            for entity in trail_ents.0.drain(trail_count..) {
                commands.entity(entity).despawn();
            }
        }

        if current_count < trail_count {
            let pos = transform.translation;
            for i in current_count..trail_count {
                let alpha = (1.0 - i as f32 / trail_count as f32) * base_srgba.alpha;
                let trail_color =
                    Color::srgba(base_srgba.red, base_srgba.green, base_srgba.blue, alpha);
                let trail_entity = commands
                    .spawn((
                        TrailDot,
                        Sprite {
                            image: circle_tex.0.clone(),
                            color: trail_color,
                            custom_size: Some(Vec2::splat(size)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, -(i as f32)),
                    ))
                    .id();
                trail_ents.0.push(trail_entity);
            }
        }

        for (i, &entity) in trail_ents.0.iter().enumerate().take(trail_count) {
            if let Ok(mut sprite) = trail_dots.get_mut(entity) {
                let alpha = (1.0 - i as f32 / trail_count as f32) * base_srgba.alpha;
                sprite.color =
                    Color::srgba(base_srgba.red, base_srgba.green, base_srgba.blue, alpha);
            }
        }
    }
}

fn sync_ball_size(
    config: Res<BounceConfig>,
    mut entities: Query<&mut Sprite, Or<(With<Ball>, With<TrailDot>)>>,
) {
    if !config.is_changed() {
        return;
    }
    let size = config.ball_size();
    for mut sprite in &mut entities {
        sprite.custom_size = Some(Vec2::splat(size));
    }
}

fn move_balls(
    config: Res<BounceConfig>,
    panel: Res<PanelWidth>,
    mut balls: Query<(&mut Transform, &mut Velocity), With<Ball>>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(win) = window.single() else { return };
    let half_w = win.width() / 2.0;
    let half_h = win.height() / 2.0;
    let left_edge = -half_w + panel.0;
    let speed = config.speed();

    for (mut transform, mut vel) in &mut balls {
        let pos = &mut transform.translation;
        pos.x += vel.0.x * speed;
        pos.y += vel.0.y * speed;

        if pos.x < left_edge {
            pos.x = left_edge;
            vel.0.x = vel.0.x.abs();
        } else if pos.x > half_w {
            pos.x = half_w;
            vel.0.x = -vel.0.x.abs();
        }

        if pos.y < -half_h {
            pos.y = -half_h;
            vel.0.y = vel.0.y.abs();
        } else if pos.y > half_h {
            pos.y = half_h;
            vel.0.y = -vel.0.y.abs();
        }
    }
}

fn apply_noise(
    config: Res<BounceConfig>,
    time: Res<Time>,
    mut balls: Query<
        (
            &NoiseSeeds,
            &BaseHue,
            &mut Velocity,
            &mut BallColor,
            &mut Sprite,
            &TrailEntities,
        ),
        With<Ball>,
    >,
    mut trail_dots: Query<&mut Sprite, (With<TrailDot>, Without<Ball>)>,
) {
    let perlin = Perlin::new(0);
    let elapsed_secs = time.elapsed_secs_f64();
    let dt = time.delta_secs_f64();
    let dir_turn_rate_rad = config.dir_turn_rate().to_radians();
    let dir_freq = 1.0 / config.dir_coherence();
    let hue_range = config.hue_range();
    let hue_freq = 1.0 / config.hue_coherence();
    let trail_count = config.trail_count();

    let dir_enabled = config.dir_enabled;
    let hue_enabled = config.hue_enabled;

    for (seeds, base_hue, mut vel, mut ball_color, mut ball_sprite, trail_ents) in &mut balls {
        // Direction noise: rotate velocity by turning_rate * delta_time
        if dir_enabled {
            let dir_noise = perlin.get([elapsed_secs * dir_freq, seeds.direction]);
            let angle = dir_noise * dir_turn_rate_rad * dt;
            let (sin, cos) = (angle as f32).sin_cos();
            let v = vel.0;
            vel.0 = Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
        }

        // Hue noise: offset from base hue by range
        if hue_enabled {
            let hue_noise = perlin.get([elapsed_secs * hue_freq, seeds.hue]);
            let new_hue = (base_hue.0 + hue_noise as f32 * hue_range as f32).rem_euclid(360.0);
            let new_color = Color::oklcha(0.5, 0.4, new_hue, 1.0);
            ball_color.0 = new_color;

            // Update ball sprite color
            ball_sprite.color = new_color;

            // Update trail sprite colors (preserve per-dot alpha)
            let base_srgba = new_color.to_srgba();
            for (i, &entity) in trail_ents.0.iter().enumerate() {
                if let Ok(mut sprite) = trail_dots.get_mut(entity) {
                    let alpha = (1.0 - i as f32 / trail_count as f32) * base_srgba.alpha;
                    sprite.color =
                        Color::srgba(base_srgba.red, base_srgba.green, base_srgba.blue, alpha);
                }
            }
        }
    }
}

fn update_trails(
    config: Res<BounceConfig>,
    mut balls: Query<(&Transform, &mut TrailHistory), With<Ball>>,
) {
    let max = config.trail_count() * config.trail_gap();
    for (transform, mut trail) in &mut balls {
        trail.0.push_front(transform.translation.truncate());
        while trail.0.len() > max {
            trail.0.pop_back();
        }
    }
}

fn sync_trail_transforms(
    config: Res<BounceConfig>,
    balls: Query<(&TrailHistory, &TrailEntities, &Transform), With<Ball>>,
    mut trail_dots: Query<&mut Transform, (With<TrailDot>, Without<Ball>)>,
) {
    let gap = config.trail_gap();
    for (trail, trail_ents, ball_transform) in &balls {
        for (i, &entity) in trail_ents.0.iter().enumerate() {
            if let Ok(mut dot_transform) = trail_dots.get_mut(entity) {
                let history_index = i * gap;
                if let Some(pos) = trail.0.get(history_index) {
                    dot_transform.translation.x = pos.x;
                    dot_transform.translation.y = pos.y;
                } else {
                    dot_transform.translation.x = ball_transform.translation.x;
                    dot_transform.translation.y = ball_transform.translation.y;
                }
            }
        }
    }
}

fn collect_balls(
    mut actions: ResMut<BallActions>,
    panel: Res<PanelWidth>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut balls: Query<(&mut Transform, &mut TrailHistory), With<Ball>>,
) {
    if !actions.collect {
        return;
    }
    actions.collect = false;
    let Ok(win) = window.single() else { return };
    let center_x = (-win.width() / 2.0 + panel.0 + win.width() / 2.0) / 2.0;
    for (mut transform, mut trail) in &mut balls {
        transform.translation.x = center_x;
        transform.translation.y = 0.0;
        trail.0.clear();
    }
}

fn scatter_balls(
    mut actions: ResMut<BallActions>,
    mut balls: Query<&mut Velocity, With<Ball>>,
) {
    if !actions.scatter {
        return;
    }
    actions.scatter = false;
    let mut rng = rand::rng();
    for mut vel in &mut balls {
        let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
        vel.0 = Vec2::new(angle.cos(), angle.sin());
    }
}

fn ui_system(
    mut contexts: EguiContexts,
    mut config: ResMut<BounceConfig>,
    mut panel_width: ResMut<PanelWidth>,
    mut actions: ResMut<BallActions>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let response = egui::SidePanel::left("controls").show(ctx, |ui| {
        ui.heading("Bouncing Balls");
        ui.separator();

        // Balls
        ui.horizontal(|ui| {
            if ui.button("-").clicked() && config.ball_count_exp > 0 {
                config.ball_count_exp -= 1;
            }
            ui.label(format!("{}", config.ball_count()));
            if ui.button("+").clicked() {
                config.ball_count_exp += 1;
            }
            ui.label("Balls");
        });

        // Size
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                config.ball_size_exp -= 1;
            }
            ui.label(format!("{:.2} px", config.ball_size()));
            if ui.button("+").clicked() {
                config.ball_size_exp += 1;
            }
            ui.label("Size");
        });

        // Trail
        ui.horizontal(|ui| {
            if ui.button("-").clicked() && config.trail_count_exp > 0 {
                config.trail_count_exp -= 1;
            }
            ui.label(format!("{}", config.trail_count()));
            if ui.button("+").clicked() {
                config.trail_count_exp += 1;
            }
            ui.label("Trail");
        });

        // Trail gap
        ui.horizontal(|ui| {
            if ui.button("-").clicked() && config.trail_gap_exp > 0 {
                config.trail_gap_exp -= 1;
            }
            ui.label(format!("{} frames", config.trail_gap()));
            if ui.button("+").clicked() {
                config.trail_gap_exp += 1;
            }
            ui.label("Trail gap");
        });

        // Speed
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                config.speed_exp -= 1;
            }
            ui.label(format!("{:.2} px/f", config.speed()));
            if ui.button("+").clicked() {
                config.speed_exp += 1;
            }
            ui.label("Speed");
        });

        ui.checkbox(&mut config.dir_enabled, "Direction noise");

        // Direction turning rate
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                config.dir_turn_rate_exp -= 1;
            }
            ui.label(format!("{:.0} °/s", config.dir_turn_rate()));
            if ui.button("+").clicked() {
                config.dir_turn_rate_exp += 1;
            }
            ui.label("Turn rate");
        });

        // Direction coherence
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                config.dir_coherence_exp -= 1;
            }
            ui.label(format!("{:.2} s", config.dir_coherence()));
            if ui.button("+").clicked() {
                config.dir_coherence_exp += 1;
            }
            ui.label("Dir coherence");
        });

        ui.checkbox(&mut config.hue_enabled, "Hue noise");

        // Hue range
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                config.hue_range_exp -= 1;
            }
            ui.label(format!("{:.0}°", config.hue_range()));
            if ui.button("+").clicked() {
                config.hue_range_exp += 1;
            }
            ui.label("Hue range");
        });

        // Hue coherence
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                config.hue_coherence_exp -= 1;
            }
            ui.label(format!("{:.2} s", config.hue_coherence()));
            if ui.button("+").clicked() {
                config.hue_coherence_exp += 1;
            }
            ui.label("Hue coherence");
        });

        // Fixed update toggle
        ui.checkbox(&mut config.use_fixed_update, "Use FixedUpdate (64 Hz)");

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Collect").clicked() {
                actions.collect = true;
            }
            if ui.button("Scatter").clicked() {
                actions.scatter = true;
            }
        });

        ui.separator();
        ui.heading("Diagnostics");

        // FPS
        if let Some(fps) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed())
        {
            let color = if fps >= 55.0 {
                egui::Color32::GREEN
            } else if fps >= 30.0 {
                egui::Color32::YELLOW
            } else {
                egui::Color32::RED
            };
            ui.colored_label(color, format!("FPS: {fps:.0}"));
        }

        // Frame time
        if let Some(frame_time) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|d| d.smoothed())
        {
            ui.label(format!("Frame time: {frame_time:.2} ms"));
        }

        // Entity count
        if let Some(entities) = diagnostics
            .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
            .and_then(|d| d.value())
        {
            ui.label(format!("Entities: {entities:.0}"));
        }

        // Delta
        ui.label(format!("Delta: {:.2} ms", time.delta_secs() * 1000.0));

        // Trail meshes
        let trail_meshes = config.ball_count() * config.trail_count();
        ui.label(format!("Trail meshes: {trail_meshes}"));
    });
    panel_width.0 = response.response.rect.width();
}

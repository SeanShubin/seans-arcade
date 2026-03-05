//! Stress-test bouncing balls — sprites with shared texture for GPU batching.
//!
//! Run with: `cargo run --example stress_balls --release`

use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::ecs::batching::BatchingStrategy;
use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use rand::RngExt;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            EguiPlugin::default(),
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
        ))
        .init_resource::<StressConfig>()
        .init_resource::<PanelWidth>()
        .init_resource::<Playfield>()
        .init_resource::<BallActions>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (update_playfield, manage_balls, sync_ball_size, move_balls, collect_balls, scatter_balls)
                .chain()
                .run_if(|config: Res<StressConfig>| !config.use_fixed_update),
        )
        .add_systems(
            FixedUpdate,
            (update_playfield, manage_balls, sync_ball_size, move_balls, collect_balls, scatter_balls)
                .chain()
                .run_if(|config: Res<StressConfig>| config.use_fixed_update),
        )
        .add_systems(EguiPrimaryContextPass, ui_system)
        .run();
}

#[derive(Resource)]
struct StressConfig {
    ball_count_exp: i32,
    size_exp: i32,
    speed_exp: i32,
    use_fixed_update: bool,
}

impl Default for StressConfig {
    fn default() -> Self {
        Self {
            ball_count_exp: 0,
            size_exp: 2,
            speed_exp: 0,
            use_fixed_update: false,
        }
    }
}

impl StressConfig {
    fn ball_count(&self) -> usize {
        2_usize.pow(self.ball_count_exp as u32)
    }

    fn ball_size(&self) -> f32 {
        2_f32.powi(self.size_exp)
    }

    fn speed(&self) -> f32 {
        2_f32.powi(self.speed_exp)
    }
}

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Velocity(Vec2);

#[derive(Resource)]
struct WhitePixel(Handle<Image>);

#[derive(Resource, Default)]
struct BallActions {
    collect: bool,
    scatter: bool,
}

#[derive(Resource, Default)]
struct PanelWidth(f32);

#[derive(Resource, Default)]
struct Playfield {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Playfield {
    fn center(&self) -> Vec2 {
        Vec2::new(
            (self.left + self.right) / 2.0,
            (self.top + self.bottom) / 2.0,
        )
    }
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);
    let image = images.add(Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ));
    commands.insert_resource(WhitePixel(image));
}

fn update_playfield(
    mut playfield: ResMut<Playfield>,
    panel: Res<PanelWidth>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(win) = window.single() else { return };
    let half_w = win.width() / 2.0;
    let half_h = win.height() / 2.0;
    playfield.left = -half_w + panel.0;
    playfield.right = half_w;
    playfield.top = half_h;
    playfield.bottom = -half_h;
}

fn manage_balls(
    mut commands: Commands,
    config: Res<StressConfig>,
    balls: Query<Entity, With<Ball>>,
    playfield: Res<Playfield>,
    white_pixel: Res<WhitePixel>,
) {
    let target = config.ball_count();
    let current = balls.iter().count();

    if current < target {
        let mut rng = rand::rng();

        for _ in 0..(target - current) {
            let x = rng.random_range(playfield.left..playfield.right);
            let y = rng.random_range(playfield.bottom..playfield.top);
            let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
            let vel = Vec2::new(angle.cos(), angle.sin());
            let hue = rng.random_range(0.0..360.0);
            let color = Color::oklcha(0.7, 0.3, hue, 1.0);

            commands.spawn((
                Ball,
                Sprite {
                    image: white_pixel.0.clone(),
                    color,
                    custom_size: Some(Vec2::splat(config.ball_size())),
                    ..default()
                },
                Transform::from_xyz(x, y, 0.0),
                Velocity(vel),
            ));
        }
    } else if current > target {
        for entity in balls.iter().take(current - target) {
            commands.entity(entity).despawn();
        }
    }
}

fn sync_ball_size(
    config: Res<StressConfig>,
    mut balls: Query<&mut Sprite, With<Ball>>,
) {
    if !config.is_changed() {
        return;
    }
    let size = config.ball_size();
    for mut sprite in &mut balls {
        sprite.custom_size = Some(Vec2::splat(size));
    }
}

fn move_balls(
    config: Res<StressConfig>,
    playfield: Res<Playfield>,
    mut balls: Query<(&mut Transform, &mut Velocity), With<Ball>>,
) {
    let speed = config.speed();
    let left = playfield.left;
    let right = playfield.right;
    let top = playfield.top;
    let bottom = playfield.bottom;

    balls
        .par_iter_mut()
        .batching_strategy(BatchingStrategy::fixed(256))
        .for_each(|(mut transform, mut vel)| {
            let pos = &mut transform.translation;
            pos.x += vel.0.x * speed;
            pos.y += vel.0.y * speed;

            if pos.x < left {
                pos.x = left;
                vel.0.x = vel.0.x.abs();
            } else if pos.x > right {
                pos.x = right;
                vel.0.x = -vel.0.x.abs();
            }

            if pos.y < bottom {
                pos.y = bottom;
                vel.0.y = vel.0.y.abs();
            } else if pos.y > top {
                pos.y = top;
                vel.0.y = -vel.0.y.abs();
            }
        });
}

fn collect_balls(
    mut actions: ResMut<BallActions>,
    playfield: Res<Playfield>,
    mut balls: Query<&mut Transform, With<Ball>>,
) {
    if !actions.collect {
        return;
    }
    actions.collect = false;
    let center = playfield.center();
    for mut transform in &mut balls {
        transform.translation.x = center.x;
        transform.translation.y = center.y;
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
    mut config: ResMut<StressConfig>,
    mut panel_width: ResMut<PanelWidth>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
    mut actions: ResMut<BallActions>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let response = egui::SidePanel::left("controls").show(ctx, |ui| {
        ui.heading("Stress Balls");
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
            ui.label(format!("Balls (2^{})", config.ball_count_exp));
        });

        // Size
        ui.horizontal(|ui| {
            if ui.button("-").clicked() && config.size_exp > 0 {
                config.size_exp -= 1;
            }
            ui.label(format!("{} px", config.ball_size() as i32));
            if ui.button("+").clicked() {
                config.size_exp += 1;
            }
            ui.label(format!("Size (2^{})", config.size_exp));
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
            ui.label(format!("Speed (2^{})", config.speed_exp));
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

        if let Some(frame_time) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|d| d.smoothed())
        {
            ui.label(format!("Frame time: {frame_time:.2} ms"));
        }

        if let Some(entities) = diagnostics
            .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
            .and_then(|d| d.value())
        {
            ui.label(format!("Entities: {entities:.0}"));
        }

        ui.label(format!("Delta: {:.2} ms", time.delta_secs() * 1000.0));
    });
    panel_width.0 = response.response.rect.width();
}

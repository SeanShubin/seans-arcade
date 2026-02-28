//! Asset browser for Time Fantasy sprite sheets and character PNGs.
//!
//! Recursively scans a directory for `.png` files and displays them one at a
//! time with pan/zoom and an optional grid overlay. Useful for inspecting
//! sprite sheets that use a 16×16 tile grid.
//!
//! Controls:
//!   Left/Right arrows (or LB/RB) — prev/next image
//!   Scroll wheel — zoom in/out
//!   Click-drag — pan
//!   Home or R — reset pan/zoom
//!   G — toggle grid overlay
//!   +/- — subdivide grid (only even divisions, no remainder)
//!   Shift +/- — adjust grid height only
//!   Ctrl +/- — adjust grid width only
//!
//! Run with: `cargo run --example asset_browser`
//! Or:       `cargo run --example asset_browser -- D:\keep\assets`

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use std::path::{Path, PathBuf};

const DEFAULT_ROOT: &str = "assets";
const ZOOM_SPEED: f32 = 0.1;
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 50.0;
const GRID_COLOR: Color = Color::srgba(1.0, 1.0, 0.0, 0.4);
const HUD_FONT_SIZE: f32 = 18.0;

// ---------------------------------------------------------------------------
// XInput FFI (from sprite_walk.rs)
// ---------------------------------------------------------------------------

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    _left_trigger: u8,
    _right_trigger: u8,
    _thumb_lx: i16,
    _thumb_ly: i16,
    _thumb_rx: i16,
    _thumb_ry: i16,
}

#[repr(C)]
struct XInputState {
    _packet_number: u32,
    gamepad: XInputGamepad,
}

const XINPUT_GAMEPAD_LEFT_SHOULDER: u16 = 0x0100;
const XINPUT_GAMEPAD_RIGHT_SHOULDER: u16 = 0x0200;
const ERROR_SUCCESS: u32 = 0;

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

fn load_xinput() -> Option<XInputGetStateFn> {
    use std::ffi::CString;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut std::ffi::c_void;
        fn GetProcAddress(
            module: *mut std::ffi::c_void,
            name: *const u8,
        ) -> *mut std::ffi::c_void;
    }

    for dll in &[b"xinput1_4.dll\0" as &[u8], b"xinput9_1_0.dll\0"] {
        let module = unsafe { LoadLibraryA(dll.as_ptr()) };
        if module.is_null() {
            continue;
        }
        let proc_name = CString::new("XInputGetState").unwrap();
        let proc = unsafe { GetProcAddress(module, proc_name.as_ptr() as *const u8) };
        if !proc.is_null() {
            return Some(unsafe { std::mem::transmute(proc) });
        }
    }
    None
}

#[derive(Resource, Default)]
struct GamepadNav {
    prev_lb: bool,
    prev_rb: bool,
    lb_just: bool,
    rb_just: bool,
}

fn read_gamepad_input(
    mut state: ResMut<GamepadNav>,
    mut xinput_fn: Local<Option<Option<XInputGetStateFn>>>,
) {
    let get_state = match *xinput_fn {
        Some(Some(f)) => f,
        Some(None) => return,
        None => {
            let loaded = load_xinput();
            if loaded.is_none() {
                warn!("Failed to load XInput DLL — gamepad nav unavailable");
            }
            *xinput_fn = Some(loaded);
            match loaded {
                Some(f) => f,
                None => return,
            }
        }
    };

    let mut xs = std::mem::MaybeUninit::<XInputState>::uninit();
    let result = unsafe { get_state(0, xs.as_mut_ptr()) };

    if result != ERROR_SUCCESS {
        state.lb_just = false;
        state.rb_just = false;
        return;
    }

    let xs = unsafe { xs.assume_init() };
    let lb = xs.gamepad.buttons & XINPUT_GAMEPAD_LEFT_SHOULDER != 0;
    let rb = xs.gamepad.buttons & XINPUT_GAMEPAD_RIGHT_SHOULDER != 0;

    state.lb_just = lb && !state.prev_lb;
    state.rb_just = rb && !state.prev_rb;
    state.prev_lb = lb;
    state.prev_rb = rb;
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct ImageList {
    paths: Vec<PathBuf>,
    /// Paths relative to the asset root, usable with AssetServer.
    asset_paths: Vec<String>,
    current: usize,
}

#[derive(Resource)]
struct BrowserState {
    cell_w: u32,
    cell_h: u32,
    grid_visible: bool,
    zoom: f32,
    pan: Vec2,
    dragging: bool,
    last_cursor: Option<Vec2>,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            cell_w: 0,
            cell_h: 0,
            grid_visible: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
            dragging: false,
            last_cursor: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct BrowserSprite;

#[derive(Component)]
struct HudText;

// ---------------------------------------------------------------------------
// Directory scanning
// ---------------------------------------------------------------------------

fn discover_pngs(root: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    scan_dir(root, &mut results);
    results.sort();
    results
}

fn scan_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("png") {
            out.push(path);
        }
    }
}

/// Determine the asset root directory from CLI args or use default.
fn get_root_path() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from(DEFAULT_ROOT)
    }
}

/// Try to make `path` relative to `base`. Returns the relative portion as a
/// forward-slash string suitable for Bevy's AssetServer.
fn make_asset_path(path: &Path, asset_root: &Path) -> Option<String> {
    let canon_path = path.canonicalize().ok()?;
    let canon_root = asset_root.canonicalize().ok()?;
    let rel = canon_path.strip_prefix(&canon_root).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, images: Res<ImageList>) {
    commands.spawn(Camera2d);

    // Spawn sprite for current image
    let handle: Handle<Image> = asset_server.load(&images.asset_paths[images.current]);
    commands.spawn((BrowserSprite, Sprite::from_image(handle), NoFrustumCulling));

    // HUD text
    let mono_font: Handle<Font> = asset_server.load("local/fonts/FiraMono-Regular.ttf");
    commands.spawn((
        HudText,
        Text::new(format_hud(&images, None, 0, 0, false)),
        TextFont {
            font: mono_font,
            font_size: HUD_FONT_SIZE,
            ..default()
        },
        TextColor::WHITE,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
    ));
}

fn navigate_images(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad: Res<GamepadNav>,
    mut images: ResMut<ImageList>,
    mut browser: ResMut<BrowserState>,
    asset_server: Res<AssetServer>,
    mut sprite_q: Query<&mut Sprite, With<BrowserSprite>>,
) {
    let count = images.paths.len();
    if count == 0 {
        return;
    }

    let prev = images.current;

    if keyboard.just_pressed(KeyCode::ArrowLeft) || gamepad.lb_just {
        images.current = (images.current + count - 1) % count;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) || gamepad.rb_just {
        images.current = (images.current + 1) % count;
    }

    if images.current != prev {
        browser.pan = Vec2::ZERO;
        browser.cell_w = 0;
        browser.cell_h = 0;

        let handle: Handle<Image> = asset_server.load(&images.asset_paths[images.current]);
        for mut sprite in &mut sprite_q {
            sprite.image = handle.clone();
        }
    }
}

fn pan_zoom(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut browser: ResMut<BrowserState>,
) {
    // Reset
    if keyboard.just_pressed(KeyCode::Home) || keyboard.just_pressed(KeyCode::KeyR) {
        browser.zoom = 1.0;
        browser.pan = Vec2::ZERO;
    }

    // Zoom via scroll wheel
    for ev in scroll_events.read() {
        let delta = ev.y * ZOOM_SPEED * browser.zoom;
        browser.zoom = (browser.zoom + delta).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    // Pan via click-drag
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());

    if mouse_buttons.just_pressed(MouseButton::Left) {
        browser.dragging = true;
        browser.last_cursor = cursor;
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        browser.dragging = false;
        browser.last_cursor = None;
    }

    if browser.dragging {
        if let (Some(current), Some(last)) = (cursor, browser.last_cursor) {
            let delta = current - last;
            // Negate Y because screen Y is down but world Y is up
            let zoom = browser.zoom;
            browser.pan += Vec2::new(delta.x, -delta.y) / zoom;
        }
        browser.last_cursor = cursor;
    }
}

fn apply_camera(
    browser: Res<BrowserState>,
    mut camera_q: Query<&mut Transform, With<Camera2d>>,
) {
    for mut tf in &mut camera_q {
        tf.translation.x = -browser.pan.x;
        tf.translation.y = -browser.pan.y;
        tf.scale = Vec3::splat(1.0 / browser.zoom);
    }
}

/// Given a dimension, find the next smaller divisor below `current`.
fn prev_divisor(dim: u32, current: u32) -> u32 {
    (1..current).rev().find(|d| dim % d == 0).unwrap_or(current)
}

/// Given a dimension, find the next larger divisor above `current`.
fn next_divisor(dim: u32, current: u32) -> u32 {
    ((current + 1)..=dim).find(|d| dim % d == 0).unwrap_or(current)
}

fn init_grid_to_image(
    mut browser: ResMut<BrowserState>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<BrowserSprite>>,
) {
    let Some(sprite) = sprite_q.iter().next() else { return };
    let Some(img) = image_assets.get(&sprite.image) else { return };

    let w = img.width();
    let h = img.height();

    if browser.cell_w == 0 || browser.cell_w > w {
        browser.cell_w = w;
    }
    if browser.cell_h == 0 || browser.cell_h > h {
        browser.cell_h = h;
    }
}

fn toggle_grid(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut browser: ResMut<BrowserState>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<BrowserSprite>>,
) {
    if keyboard.just_pressed(KeyCode::KeyG) {
        browser.grid_visible = !browser.grid_visible;
    }

    let Some(sprite) = sprite_q.iter().next() else { return };
    let Some(img) = image_assets.get(&sprite.image) else { return };

    let img_w = img.width();
    let img_h = img.height();

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let adjust_w = ctrl || !shift; // plain or ctrl
    let adjust_h = shift || !ctrl; // plain or shift

    // minus = subdivide more (smaller cells)
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        if adjust_w {
            browser.cell_w = prev_divisor(img_w, browser.cell_w);
        }
        if adjust_h {
            browser.cell_h = prev_divisor(img_h, browser.cell_h);
        }
    }
    // plus = merge (larger cells)
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        if adjust_w {
            browser.cell_w = next_divisor(img_w, browser.cell_w);
        }
        if adjust_h {
            browser.cell_h = next_divisor(img_h, browser.cell_h);
        }
    }
}

fn draw_grid(
    browser: Res<BrowserState>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<BrowserSprite>>,
    mut gizmos: Gizmos,
) {
    if !browser.grid_visible {
        return;
    }

    let Some(sprite) = sprite_q.iter().next() else {
        return;
    };
    let Some(img) = image_assets.get(&sprite.image) else {
        return;
    };

    let w = img.width() as f32;
    let h = img.height() as f32;
    let cw = browser.cell_w as f32;
    let ch = browser.cell_h as f32;
    if cw == 0.0 || ch == 0.0 {
        return;
    }

    let cols = (w / cw).round() as i32;
    let rows = (h / ch).round() as i32;

    let left = -w / 2.0;
    let top = h / 2.0;

    // Vertical lines
    for c in 0..=cols {
        let x = left + c as f32 * cw;
        gizmos.line_2d(Vec2::new(x, top), Vec2::new(x, top - h), GRID_COLOR);
    }
    // Horizontal lines
    for r in 0..=rows {
        let y = top - r as f32 * ch;
        gizmos.line_2d(Vec2::new(left, y), Vec2::new(left + w, y), GRID_COLOR);
    }
}

fn update_hud(
    images: Res<ImageList>,
    browser: Res<BrowserState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<BrowserSprite>>,
    mut hud_q: Query<&mut Text, With<HudText>>,
) {
    // Compute grid cell under cursor
    let hover_cell = (|| {
        let window = windows.single().ok()?;
        let cursor = window.cursor_position()?;
        let (cam, cam_tf) = camera_q.single().ok()?;
        let world = cam.viewport_to_world_2d(cam_tf, cursor).ok()?;

        let sprite = sprite_q.iter().next()?;
        let img = image_assets.get(&sprite.image)?;

        let w = img.width() as f32;
        let h = img.height() as f32;

        // Image origin is center; top-left is (-w/2, h/2)
        let local_x = world.x + w / 2.0;
        let local_y = h / 2.0 - world.y;

        if local_x < 0.0 || local_y < 0.0 || local_x >= w || local_y >= h {
            return None;
        }

        if browser.cell_w == 0 || browser.cell_h == 0 {
            return None;
        }
        let col = (local_x / browser.cell_w as f32) as i32;
        let row = (local_y / browser.cell_h as f32) as i32;
        Some((col, row))
    })();

    for mut text in &mut hud_q {
        **text = format_hud(&images, hover_cell, browser.cell_w, browser.cell_h, browser.grid_visible);
    }
}

fn format_hud(
    images: &ImageList,
    hover_cell: Option<(i32, i32)>,
    cell_w: u32,
    cell_h: u32,
    grid_visible: bool,
) -> String {
    if images.paths.is_empty() {
        return "No PNG files found.".into();
    }

    let path = &images.paths[images.current];
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "?".into());
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let idx = images.current + 1;
    let total = images.paths.len();

    let mut s = format!("[{idx}/{total}] {parent}/{filename}");

    if grid_visible {
        s.push_str(&format!("  |  grid: {cell_w}x{cell_h}"));
        if let Some((col, row)) = hover_cell {
            s.push_str(&format!("  |  cell: ({col}, {row})"));
        }
    }

    s
}

fn update_window_title(
    images: Res<ImageList>,
    browser: Res<BrowserState>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<BrowserSprite>>,
    mut windows: Query<&mut Window>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };

    if images.paths.is_empty() {
        window.title = "Asset Browser — no images".into();
        return;
    }

    let path = &images.paths[images.current];
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "?".into());

    let size_str = sprite_q
        .iter()
        .next()
        .and_then(|s| image_assets.get(&s.image))
        .map(|img| format!("{}x{}", img.width(), img.height()))
        .unwrap_or_else(|| "?x?".into());

    let zoom_pct = (browser.zoom * 100.0).round() as i32;

    window.title = format!(
        "Asset Browser — {filename} — {size_str} — {zoom_pct}%"
    );
}

// ---------------------------------------------------------------------------
// Scroll bar (egui)
// ---------------------------------------------------------------------------

fn scrubber_ui(
    mut contexts: EguiContexts,
    mut images: ResMut<ImageList>,
    mut browser: ResMut<BrowserState>,
    asset_server: Res<AssetServer>,
    mut sprite_q: Query<&mut Sprite, With<BrowserSprite>>,
) {
    let count = images.paths.len();
    if count == 0 {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::TopBottomPanel::bottom("scrubber").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let path = &images.paths[images.current];
            let label = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "?".into());

            let mut index = images.current as f32;
            let max = (count - 1) as f32;

            ui.spacing_mut().slider_width = ui.available_width() - 200.0;

            let response = ui.add(
                egui::Slider::new(&mut index, 0.0..=max)
                    .step_by(1.0)
                    .show_value(false),
            );

            ui.label(format!("{}/{} {}", images.current + 1, count, label));

            if response.changed() {
                let new_index = index.round() as usize;
                if new_index != images.current {
                    images.current = new_index;
                    browser.pan = Vec2::ZERO;
                    browser.cell_w = 0;
                    browser.cell_h = 0;
                    let handle: Handle<Image> =
                        asset_server.load(&images.asset_paths[images.current]);
                    for mut sprite in &mut sprite_q {
                        sprite.image = handle.clone();
                    }
                }
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let root = get_root_path();
    let abs_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let pngs = if abs_root.is_file() {
        vec![abs_root.clone()]
    } else {
        discover_pngs(&abs_root)
    };

    // The Bevy assets/ directory for AssetServer
    let bevy_asset_root = std::fs::canonicalize("assets").ok();

    let mut asset_paths: Vec<String> = Vec::new();
    let mut valid_pngs: Vec<PathBuf> = Vec::new();

    for p in &pngs {
        if let Some(ref ar) = bevy_asset_root {
            if let Some(rel) = make_asset_path(p, ar) {
                asset_paths.push(rel);
                valid_pngs.push(p.clone());
            }
        }
    }

    // If scanning outside assets/, fall back to loading via absolute paths
    // by symlinking or warn. For now, only support assets under `assets/`.
    if valid_pngs.is_empty() && !pngs.is_empty() {
        eprintln!(
            "Found {} PNGs under {}, but none are under the Bevy `assets/` directory.",
            pngs.len(),
            abs_root.display()
        );
        eprintln!("Run from the project root or pass a path inside `assets/`.");
        std::process::exit(1);
    }

    if valid_pngs.is_empty() {
        eprintln!("No PNG files found under {}", abs_root.display());
        std::process::exit(1);
    }

    info!("Asset browser: found {} PNGs", valid_pngs.len());

    App::new()
        .add_plugins((
            DefaultPlugins.set(ImagePlugin::default_nearest()),
            EguiPlugin::default(),
        ))
        .insert_resource(ImageList {
            paths: valid_pngs,
            asset_paths,
            current: 0,
        })
        .init_resource::<BrowserState>()
        .init_resource::<GamepadNav>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                read_gamepad_input,
                navigate_images,
                pan_zoom,
                init_grid_to_image,
                toggle_grid,
                apply_camera,
                draw_grid,
                update_hud,
                update_window_title,
            )
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, scrubber_ui)
        .run();
}

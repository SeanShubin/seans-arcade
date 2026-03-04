//! Sprite grid — interactive Bevy tool for grid definition.
//!
//! Controls:
//!   Left/Right (or LB/RB) — prev/next image
//!   +/- — cycle valid cell sizes
//!   G — toggle grid overlay
//!   Right-click drag — select span (after applying grid)
//!   Scroll wheel — zoom
//!   Left-click drag — pan
//!   Home or R — reset pan/zoom
//!   Ctrl+S — save
//!
//! Run with:
//!   cargo run --example sprite_grid -- metadata.toml --pack-root D:/assets/SomePack

#[path = "shared/sprite_meta.rs"]
mod sprite_meta;
#[path = "shared/sprite_analysis.rs"]
mod sprite_analysis;

use bevy::camera::visibility::NoFrustumCulling;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use sprite_analysis::*;
use sprite_meta::{CatalogEntry, PipelineConfig, Sheet, Source, SpriteMetadata, verify};
use std::collections::BTreeMap;
use std::path::PathBuf;

const ZOOM_SPEED: f32 = 0.1;
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 50.0;
const GRID_COLOR: Color = Color::srgba(1.0, 1.0, 0.0, 0.4);
const EMPTY_CELL_COLOR: Color = Color::srgba(1.0, 0.2, 0.2, 0.25);
const SPAN_SELECT_COLOR: Color = Color::srgba(0.0, 1.0, 1.0, 0.6);
const SPAN_OUTLINE_COLOR: Color = Color::srgba(0.2, 1.0, 0.4, 0.7);

// ---------------------------------------------------------------------------
// XInput FFI
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
struct PipelineInfo {
    config: PipelineConfig,
    config_path: PathBuf,
    current_pack: usize,
}

#[derive(Resource)]
struct EditorState {
    meta: SpriteMetadata,
    meta_path: PathBuf,
    pack_root: PathBuf,
    config_path: Option<PathBuf>,
    // Grid mode
    image_keys: Vec<String>,
    current_image: usize,
    // General
    dirty: bool,
    status_message: Option<(String, f64)>,
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
    fit_requested: bool,
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
            fit_requested: true,
        }
    }
}

#[derive(Resource, Default)]
struct Occupancy {
    grid: Vec<bool>,
    cols: u32,
    rows: u32,
    cell_w: u32,
    cell_h: u32,
    image_key: String,
}

#[derive(Resource, Default)]
struct SpanSelection {
    start: Option<(u32, u32)>,
    end: Option<(u32, u32)>,
}

#[derive(Resource, Default)]
struct HoveredCell {
    cell: Option<(u32, u32)>,
}

#[derive(Resource, Default)]
struct CurrentRawImage {
    rgba: Option<image::RgbaImage>,
    image_key: String,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct EditorSprite;

// ---------------------------------------------------------------------------
// CLI parsing
// ---------------------------------------------------------------------------

enum CliMode {
    Single {
        meta_path: PathBuf,
        pack_root: PathBuf,
    },
    Config {
        config_path: PathBuf,
    },
}

fn parse_cli() -> CliMode {
    let args: Vec<String> = std::env::args().collect();

    let mut config_path = None;
    let mut meta_path = None;
    let mut pack_root = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = Some(PathBuf::from(&args[i]));
            }
            "--pack-root" => {
                i += 1;
                pack_root = Some(PathBuf::from(&args[i]));
            }
            other if !other.starts_with('-') && meta_path.is_none() => {
                meta_path = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if let Some(config_path) = config_path {
        CliMode::Config { config_path }
    } else if let Some(meta_path) = meta_path {
        CliMode::Single {
            meta_path,
            pack_root: pack_root.unwrap_or_else(|| {
                eprintln!("--pack-root is required");
                std::process::exit(1);
            }),
        }
    } else {
        eprintln!("Usage: sprite_grid <metadata.toml> --pack-root <dir>");
        eprintln!("       sprite_grid --config sprite-metadata/sprite-packs.toml");
        std::process::exit(1);
    }
}

fn load_pipeline_config(path: &std::path::Path) -> PipelineConfig {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Cannot read config {}: {e}", path.display());
        std::process::exit(1);
    });
    toml::from_str::<PipelineConfig>(&text).unwrap_or_else(|e| {
        eprintln!("Failed to parse config {}: {e}", path.display());
        std::process::exit(1);
    })
}

/// Build an EditorState from a metadata TOML path and pack root.
/// Returns None if the TOML file doesn't exist (pack not yet discovered).
fn build_editor_state(
    meta_path: &std::path::Path,
    pack_root: &std::path::Path,
    config_path: Option<&std::path::Path>,
) -> Option<EditorState> {
    let meta_text = match std::fs::read_to_string(meta_path) {
        Ok(t) => t,
        Err(_) => return None,
    };
    let meta: SpriteMetadata = match toml::from_str(&meta_text) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Cannot parse {}: {e}", meta_path.display());
            return None;
        }
    };

    let errors = verify(&meta);
    if !errors.is_empty() {
        eprintln!("Metadata validation warnings:");
        for err in &errors {
            eprintln!("  - {err}");
        }
        eprintln!("Continuing anyway...");
    }

    let image_keys: Vec<String> = meta.images.keys().cloned().collect();

    eprintln!(
        "Loaded: {} images, {} sheets, {} catalog entries",
        image_keys.len(),
        meta.sheets.len(),
        meta.catalog.len(),
    );

    Some(EditorState {
        meta,
        meta_path: meta_path.to_path_buf(),
        pack_root: pack_root.to_path_buf(),
        config_path: config_path.map(|p| p.to_path_buf()),
        image_keys,
        current_image: 0,
        dirty: false,
        status_message: None,
    })
}

// ---------------------------------------------------------------------------
// Helpers: sheet/image mapping
// ---------------------------------------------------------------------------

fn sheet_for_image(meta: &SpriteMetadata, image_key: &str) -> Option<String> {
    meta.sheets
        .iter()
        .find(|(_, s)| s.file == image_key)
        .map(|(id, _)| id.clone())
}

fn derive_sheet_id(image_key: &str, existing: &BTreeMap<String, Sheet>) -> String {
    let path = std::path::Path::new(image_key);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let base: String = stem
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if !existing.contains_key(&base) {
        return base;
    }

    let mut n = 2;
    loop {
        let candidate = format!("{base}_{n}");
        if !existing.contains_key(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

fn default_cell_size(valid: &[u32], dim: u32) -> u32 {
    if valid.contains(&16) {
        16
    } else if let Some(&first) = valid.first() {
        first
    } else {
        dim
    }
}

const MIN_CELL: u32 = 8;

fn next_valid_size(valid: &[u32], current: u32, dim: u32) -> u32 {
    if valid.is_empty() {
        return ((current + 1)..=dim)
            .find(|&d| d >= MIN_CELL && dim % d == 0)
            .unwrap_or(current);
    }
    valid
        .iter()
        .copied()
        .find(|&s| s > current)
        .unwrap_or(current)
}

fn prev_valid_size(valid: &[u32], current: u32, dim: u32) -> u32 {
    if valid.is_empty() {
        return (MIN_CELL..current)
            .rev()
            .find(|&d| dim % d == 0)
            .unwrap_or(current);
    }
    valid
        .iter()
        .copied()
        .rev()
        .find(|&s| s < current)
        .unwrap_or(current)
}

// ---------------------------------------------------------------------------
// Helpers: apply grid
// ---------------------------------------------------------------------------

fn apply_grid_to_image(
    meta: &mut SpriteMetadata,
    image_key: &str,
    rgba: &image::RgbaImage,
    cell_w: u32,
    cell_h: u32,
) -> (String, usize) {
    // Remove existing sheet + catalog for this image if re-gridding
    if let Some(old_sheet_id) = sheet_for_image(meta, image_key) {
        let prefix = format!("{old_sheet_id}.");
        let to_remove: Vec<String> = meta
            .catalog
            .keys()
            .filter(|k| k.starts_with(&prefix) || *k == &old_sheet_id)
            .cloned()
            .collect();
        for id in &to_remove {
            meta.catalog.remove(id);
        }
        meta.sheets.remove(&old_sheet_id);
    }

    let sheet_id = derive_sheet_id(image_key, &meta.sheets);
    let cols = rgba.width() / cell_w;
    let rows = rgba.height() / cell_h;

    let sheet = Sheet {
        file: image_key.to_string(),
        cell_w,
        cell_h,
        cols,
        rows,
        scale: None,
        color_count: None,
        transparent_pct: None,
        description: None,
    };
    meta.sheets.insert(sheet_id.clone(), sheet);

    // Create catalog entries for occupied cells
    let mut new_entries: Vec<(String, CatalogEntry)> = Vec::new();

    for row in 0..rows {
        for col in 0..cols {
            let x = col * cell_w;
            let y = row * cell_h;

            if !is_cell_occupied(rgba, x, y, cell_w, cell_h) {
                continue;
            }

            let entry_id = format!("{sheet_id}.{col}.{row}");
            let cell_img = crop_cell(rgba, col, row, cell_w, cell_h);
            let analysis = analyze_cell(&cell_img);

            let entry = CatalogEntry {
                sources: vec![Source::sheet_cell(&sheet_id, col, row)],
                derived_from: None,
                empty: Some(analysis.empty),
                bbox: analysis.bbox,
                pixels: analysis.pixels,
                colors: analysis.colors,
                hash: analysis.hash,
                duplicate_of: None,
            };

            new_entries.push((entry_id, entry));
        }
    }

    let entry_count = new_entries.len();

    // Hash-based dedup
    let mut hash_map: BTreeMap<String, String> = BTreeMap::new();
    for (id, entry) in &meta.catalog {
        if let Some(ref hash) = entry.hash {
            if entry.duplicate_of.is_none() {
                hash_map.entry(hash.clone()).or_insert_with(|| id.clone());
            }
        }
    }

    for (id, mut entry) in new_entries {
        if let Some(ref hash) = entry.hash {
            if let Some(existing_id) = hash_map.get(hash) {
                if existing_id != &id {
                    entry.duplicate_of = Some(existing_id.clone());
                }
            } else {
                hash_map.insert(hash.clone(), id.clone());
            }
        }
        meta.catalog.insert(id, entry);
    }

    (sheet_id, entry_count)
}

fn merge_span(
    meta: &mut SpriteMetadata,
    raw_image: &image::RgbaImage,
    sheet_id: &str,
    start: (u32, u32),
    end: (u32, u32),
) -> Option<String> {
    let min_col = start.0.min(end.0);
    let max_col = start.0.max(end.0);
    let min_row = start.1.min(end.1);
    let max_row = start.1.max(end.1);

    let col_span = max_col - min_col + 1;
    let row_span = max_row - min_row + 1;

    if col_span <= 1 && row_span <= 1 {
        return None;
    }

    let sheet = meta.sheets.get(sheet_id)?;
    let cell_w = sheet.cell_w;
    let cell_h = sheet.cell_h;

    // Remove individual entries in the span
    for row in min_row..=max_row {
        for col in min_col..=max_col {
            let id = format!("{sheet_id}.{col}.{row}");
            meta.catalog.remove(&id);
        }
    }

    // Create merged entry
    let merged_id = format!("{sheet_id}.{min_col}.{min_row}");
    let x = min_col * cell_w;
    let y = min_row * cell_h;
    let w = col_span * cell_w;
    let h = row_span * cell_h;

    let span_img = image::imageops::crop_imm(raw_image, x, y, w, h).to_image();
    let analysis = analyze_cell(&span_img);

    let entry = CatalogEntry {
        sources: vec![Source::sheet_span(
            sheet_id, min_col, min_row, col_span, row_span,
        )],
        derived_from: None,
        empty: Some(analysis.empty),
        bbox: analysis.bbox,
        pixels: analysis.pixels,
        colors: analysis.colors,
        hash: analysis.hash,
        duplicate_of: None,
    };

    meta.catalog.insert(merged_id.clone(), entry);
    Some(merged_id)
}

// ---------------------------------------------------------------------------
// Helpers: image loading
// ---------------------------------------------------------------------------

fn rgba_to_bevy_image(rgba: &image::RgbaImage) -> Image {
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: rgba.width(),
            height: rgba.height(),
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        rgba.as_raw().clone(),
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD | bevy::asset::RenderAssetUsages::MAIN_WORLD,
    )
}

fn load_grid_display(
    editor: &EditorState,
    raw_image: &mut CurrentRawImage,
    images: &mut Assets<Image>,
) -> Handle<Image> {
    let Some(image_key) = editor.image_keys.get(editor.current_image) else {
        return images.add(rgba_to_bevy_image(&image::RgbaImage::new(1, 1)));
    };

    let path = editor.pack_root.join(image_key);
    if let Ok(img) = image::open(&path) {
        let rgba = img.to_rgba8();
        let handle = images.add(rgba_to_bevy_image(&rgba));
        raw_image.rgba = Some(rgba);
        raw_image.image_key = image_key.clone();
        handle
    } else {
        raw_image.rgba = None;
        raw_image.image_key.clear();
        images.add(rgba_to_bevy_image(&image::RgbaImage::new(1, 1)))
    }
}

// ---------------------------------------------------------------------------
// Systems: setup
// ---------------------------------------------------------------------------

fn setup(
    mut commands: Commands,
    editor: Res<EditorState>,
    mut browser: ResMut<BrowserState>,
    mut raw_image: ResMut<CurrentRawImage>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn(Camera2d);

    // Initialize cell size for first image
    if let Some(image_key) = editor.image_keys.first() {
        if let Some(sheet_id) = sheet_for_image(&editor.meta, image_key) {
            let sheet = &editor.meta.sheets[&sheet_id];
            browser.cell_w = sheet.cell_w;
            browser.cell_h = sheet.cell_h;
        } else if let Some(img_entry) = editor.meta.images.get(image_key) {
            browser.cell_w = default_cell_size(&img_entry.valid_cell_widths, img_entry.width);
            browser.cell_h = default_cell_size(&img_entry.valid_cell_heights, img_entry.height);
        }
    }
    browser.grid_visible = true;
    let handle = load_grid_display(&editor, &mut raw_image, &mut images);

    commands.spawn((EditorSprite, Sprite::from_image(handle), NoFrustumCulling));
}

// ---------------------------------------------------------------------------
// Systems: navigate
// ---------------------------------------------------------------------------

fn navigate(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad: Res<GamepadNav>,
    mut editor: ResMut<EditorState>,
    mut browser: ResMut<BrowserState>,
    mut raw_image: ResMut<CurrentRawImage>,
    mut selection: ResMut<SpanSelection>,
    mut bevy_images: ResMut<Assets<Image>>,
    mut sprite_q: Query<&mut Sprite, With<EditorSprite>>,
) {
    let count = editor.image_keys.len();
    if count == 0 {
        return;
    }

    let prev = editor.current_image;

    if keyboard.just_pressed(KeyCode::ArrowLeft) || gamepad.lb_just {
        editor.current_image = (editor.current_image + count - 1) % count;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) || gamepad.rb_just {
        editor.current_image = (editor.current_image + 1) % count;
    }

    if editor.current_image != prev {
        browser.pan = Vec2::ZERO;
        browser.fit_requested = true;
        selection.start = None;
        selection.end = None;

        let handle = load_grid_display(&editor, &mut raw_image, &mut bevy_images);
        for mut sprite in &mut sprite_q {
            sprite.image = handle.clone();
        }

        // Set cell size from existing sheet or defaults
        let image_key = &editor.image_keys[editor.current_image];
        if let Some(sheet_id) = sheet_for_image(&editor.meta, image_key) {
            let sheet = &editor.meta.sheets[&sheet_id];
            browser.cell_w = sheet.cell_w;
            browser.cell_h = sheet.cell_h;
        } else if let Some(img_entry) = editor.meta.images.get(image_key) {
            browser.cell_w = default_cell_size(&img_entry.valid_cell_widths, img_entry.width);
            browser.cell_h = default_cell_size(&img_entry.valid_cell_heights, img_entry.height);
        }
        browser.grid_visible = true;
    }
}

// ---------------------------------------------------------------------------
// Systems: pan/zoom + camera
// ---------------------------------------------------------------------------

fn pan_zoom(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut browser: ResMut<BrowserState>,
) {
    if keyboard.just_pressed(KeyCode::Home) || keyboard.just_pressed(KeyCode::KeyR) {
        browser.fit_requested = true;
        browser.pan = Vec2::ZERO;
    }

    for ev in scroll_events.read() {
        let delta = ev.y * ZOOM_SPEED * browser.zoom;
        browser.zoom = (browser.zoom + delta).clamp(MIN_ZOOM, MAX_ZOOM);
    }

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
            let zoom = browser.zoom;
            browser.pan += Vec2::new(delta.x, -delta.y) / zoom;
        }
        browser.last_cursor = cursor;
    }
}

const SIDE_PANEL_WIDTH: f32 = 340.0;
const FIT_MARGIN: f32 = 16.0;

fn auto_fit_zoom(
    mut browser: ResMut<BrowserState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<EditorSprite>>,
) {
    if !browser.fit_requested {
        return;
    }

    let Some(sprite) = sprite_q.iter().next() else {
        return;
    };
    let Some(img) = image_assets.get(&sprite.image) else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    let img_w = img.width() as f32;
    let img_h = img.height() as f32;
    if img_w == 0.0 || img_h == 0.0 {
        return;
    }

    let viewport_w = (window.width() - SIDE_PANEL_WIDTH - FIT_MARGIN).max(1.0);
    let viewport_h = (window.height() - FIT_MARGIN).max(1.0);

    let zoom = (viewport_w / img_w).min(viewport_h / img_h);
    browser.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    browser.pan = Vec2::new(-SIDE_PANEL_WIDTH / (2.0 * browser.zoom), 0.0);
    browser.fit_requested = false;
}

fn apply_camera(browser: Res<BrowserState>, mut camera_q: Query<&mut Transform, With<Camera2d>>) {
    for mut tf in &mut camera_q {
        tf.translation.x = -browser.pan.x;
        tf.translation.y = -browser.pan.y;
        tf.scale = Vec3::splat(1.0 / browser.zoom);
    }
}

// ---------------------------------------------------------------------------
// Systems: grid overlay + occupancy
// ---------------------------------------------------------------------------

fn toggle_grid(
    keyboard: Res<ButtonInput<KeyCode>>,
    editor: Res<EditorState>,
    raw_image: Res<CurrentRawImage>,
    mut browser: ResMut<BrowserState>,
) {
    if keyboard.just_pressed(KeyCode::KeyG) {
        browser.grid_visible = !browser.grid_visible;
    }

    let img_w = raw_image
        .rgba
        .as_ref()
        .map(|r| r.width())
        .unwrap_or(0);
    let img_h = raw_image
        .rgba
        .as_ref()
        .map(|r| r.height())
        .unwrap_or(0);
    if img_w == 0 || img_h == 0 {
        return;
    }

    // Get valid cell sizes for the current image
    let (valid_w, valid_h) = editor
        .image_keys
        .get(editor.current_image)
        .and_then(|k| editor.meta.images.get(k))
        .map(|e| (e.valid_cell_widths.as_slice(), e.valid_cell_heights.as_slice()))
        .unwrap_or((&[], &[]));

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let adjust_w = ctrl || !shift;
    let adjust_h = shift || !ctrl;

    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        if adjust_w {
            browser.cell_w = prev_valid_size(valid_w, browser.cell_w, img_w);
        }
        if adjust_h {
            browser.cell_h = prev_valid_size(valid_h, browser.cell_h, img_h);
        }
    }
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        if adjust_w {
            browser.cell_w = next_valid_size(valid_w, browser.cell_w, img_w);
        }
        if adjust_h {
            browser.cell_h = next_valid_size(valid_h, browser.cell_h, img_h);
        }
    }
}

fn update_occupancy(
    editor: Res<EditorState>,
    browser: Res<BrowserState>,
    raw_image: Res<CurrentRawImage>,
    mut occupancy: ResMut<Occupancy>,
) {
    let Some(image_key) = editor.image_keys.get(editor.current_image) else {
        return;
    };

    if occupancy.image_key == *image_key
        && occupancy.cell_w == browser.cell_w
        && occupancy.cell_h == browser.cell_h
        && browser.cell_w > 0
        && browser.cell_h > 0
    {
        return;
    }

    let Some(ref rgba) = raw_image.rgba else {
        return;
    };

    if browser.cell_w == 0 || browser.cell_h == 0 {
        return;
    }

    let cols = rgba.width() / browser.cell_w;
    let rows = rgba.height() / browser.cell_h;

    let mut grid = Vec::with_capacity((cols * rows) as usize);
    for row in 0..rows {
        for col in 0..cols {
            let x = col * browser.cell_w;
            let y = row * browser.cell_h;
            grid.push(is_cell_occupied(rgba, x, y, browser.cell_w, browser.cell_h));
        }
    }

    occupancy.grid = grid;
    occupancy.cols = cols;
    occupancy.rows = rows;
    occupancy.cell_w = browser.cell_w;
    occupancy.cell_h = browser.cell_h;
    occupancy.image_key = image_key.clone();
}

fn draw_grid(
    browser: Res<BrowserState>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<EditorSprite>>,
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

    for c in 0..=cols {
        let x = left + c as f32 * cw;
        gizmos.line_2d(Vec2::new(x, top), Vec2::new(x, top - h), GRID_COLOR);
    }
    for r in 0..=rows {
        let y = top - r as f32 * ch;
        gizmos.line_2d(Vec2::new(left, y), Vec2::new(left + w, y), GRID_COLOR);
    }
}

fn draw_occupancy(
    browser: Res<BrowserState>,
    occupancy: Res<Occupancy>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<EditorSprite>>,
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

    if occupancy.cols == 0 || occupancy.rows == 0 {
        return;
    }

    let w = img.width() as f32;
    let h = img.height() as f32;
    let cw = occupancy.cell_w as f32;
    let ch = occupancy.cell_h as f32;
    let left = -w / 2.0;
    let top = h / 2.0;

    for row in 0..occupancy.rows {
        for col in 0..occupancy.cols {
            let idx = (row * occupancy.cols + col) as usize;
            if idx < occupancy.grid.len() && !occupancy.grid[idx] {
                // Draw X for empty cells
                let x0 = left + col as f32 * cw;
                let y0 = top - row as f32 * ch;
                let x1 = x0 + cw;
                let y1 = y0 - ch;
                gizmos.line_2d(Vec2::new(x0, y0), Vec2::new(x1, y1), EMPTY_CELL_COLOR);
                gizmos.line_2d(Vec2::new(x1, y0), Vec2::new(x0, y1), EMPTY_CELL_COLOR);
            }
        }
    }
}

fn draw_spans(
    editor: Res<EditorState>,
    browser: Res<BrowserState>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<EditorSprite>>,
    mut gizmos: Gizmos,
) {
    if !browser.grid_visible {
        return;
    }

    let Some(image_key) = editor.image_keys.get(editor.current_image) else {
        return;
    };
    let Some(sheet_id) = sheet_for_image(&editor.meta, image_key) else {
        return;
    };

    let Some(sprite) = sprite_q.iter().next() else {
        return;
    };
    let Some(img) = image_assets.get(&sprite.image) else {
        return;
    };

    let cw = browser.cell_w as f32;
    let ch = browser.cell_h as f32;
    if cw == 0.0 || ch == 0.0 {
        return;
    }

    let w = img.width() as f32;
    let h = img.height() as f32;
    let left = -w / 2.0;
    let top = h / 2.0;

    // Find catalog entries for this sheet that have col_span or row_span > 1
    for entry in editor.meta.catalog.values() {
        for source in &entry.sources {
            if source.sheet.as_deref() != Some(&sheet_id) {
                continue;
            }
            let cs = source.col_span.unwrap_or(1);
            let rs = source.row_span.unwrap_or(1);
            if cs <= 1 && rs <= 1 {
                continue;
            }

            let col = source.col.unwrap_or(0);
            let row = source.row.unwrap_or(0);

            let x0 = left + col as f32 * cw;
            let y0 = top - row as f32 * ch;
            let x1 = x0 + cs as f32 * cw;
            let y1 = y0 - rs as f32 * ch;

            // Draw thick outline (double lines for visibility)
            for offset in [0.0, 1.0] {
                let o = offset;
                gizmos.line_2d(
                    Vec2::new(x0 + o, y0 - o),
                    Vec2::new(x1 - o, y0 - o),
                    SPAN_OUTLINE_COLOR,
                );
                gizmos.line_2d(
                    Vec2::new(x1 - o, y0 - o),
                    Vec2::new(x1 - o, y1 + o),
                    SPAN_OUTLINE_COLOR,
                );
                gizmos.line_2d(
                    Vec2::new(x1 - o, y1 + o),
                    Vec2::new(x0 + o, y1 + o),
                    SPAN_OUTLINE_COLOR,
                );
                gizmos.line_2d(
                    Vec2::new(x0 + o, y1 + o),
                    Vec2::new(x0 + o, y0 - o),
                    SPAN_OUTLINE_COLOR,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Systems: span selection
// ---------------------------------------------------------------------------

fn span_select(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    browser: Res<BrowserState>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<EditorSprite>>,
    mut selection: ResMut<SpanSelection>,
    mut hovered: ResMut<HoveredCell>,
) {
    if browser.cell_w == 0 || browser.cell_h == 0 {
        hovered.cell = None;
        return;
    }

    let Ok((camera, camera_transform)) = camera_q.single() else {
        hovered.cell = None;
        return;
    };
    let Ok(window) = windows.single() else {
        hovered.cell = None;
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        hovered.cell = None;
        return;
    };

    let Some(sprite) = sprite_q.iter().next() else {
        hovered.cell = None;
        return;
    };
    let Some(img) = image_assets.get(&sprite.image) else {
        hovered.cell = None;
        return;
    };

    let img_w = img.width() as f32;
    let img_h = img.height() as f32;
    let cw = browser.cell_w as f32;
    let ch = browser.cell_h as f32;

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        hovered.cell = None;
        return;
    };

    let img_x = world_pos.x + img_w / 2.0;
    let img_y = img_h / 2.0 - world_pos.y;

    if img_x < 0.0 || img_y < 0.0 || img_x >= img_w || img_y >= img_h {
        hovered.cell = None;
        if mouse_buttons.just_released(MouseButton::Right) {
            selection.start = None;
            selection.end = None;
        }
        return;
    }

    let col = (img_x / cw) as u32;
    let row = (img_y / ch) as u32;
    hovered.cell = Some((col, row));

    if mouse_buttons.just_pressed(MouseButton::Right) {
        selection.start = Some((col, row));
        selection.end = Some((col, row));
    }

    if mouse_buttons.pressed(MouseButton::Right) {
        selection.end = Some((col, row));
    }
}

fn draw_span_selection(
    browser: Res<BrowserState>,
    selection: Res<SpanSelection>,
    image_assets: Res<Assets<Image>>,
    sprite_q: Query<&Sprite, With<EditorSprite>>,
    mut gizmos: Gizmos,
) {
    let (Some(start), Some(end)) = (selection.start, selection.end) else {
        return;
    };

    let Some(sprite) = sprite_q.iter().next() else {
        return;
    };
    let Some(img) = image_assets.get(&sprite.image) else {
        return;
    };

    let img_w = img.width() as f32;
    let img_h = img.height() as f32;
    let cw = browser.cell_w as f32;
    let ch = browser.cell_h as f32;
    if cw == 0.0 || ch == 0.0 {
        return;
    }

    let left = -img_w / 2.0;
    let top = img_h / 2.0;

    let min_col = start.0.min(end.0);
    let max_col = start.0.max(end.0);
    let min_row = start.1.min(end.1);
    let max_row = start.1.max(end.1);

    let x0 = left + min_col as f32 * cw;
    let y0 = top - min_row as f32 * ch;
    let x1 = left + (max_col + 1) as f32 * cw;
    let y1 = top - (max_row + 1) as f32 * ch;

    gizmos.line_2d(Vec2::new(x0, y0), Vec2::new(x1, y0), SPAN_SELECT_COLOR);
    gizmos.line_2d(Vec2::new(x1, y0), Vec2::new(x1, y1), SPAN_SELECT_COLOR);
    gizmos.line_2d(Vec2::new(x1, y1), Vec2::new(x0, y1), SPAN_SELECT_COLOR);
    gizmos.line_2d(Vec2::new(x0, y1), Vec2::new(x0, y0), SPAN_SELECT_COLOR);
}

// ---------------------------------------------------------------------------
// Systems: save + window title
// ---------------------------------------------------------------------------

fn save_metadata(keyboard: Res<ButtonInput<KeyCode>>, mut editor: ResMut<EditorState>) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !(ctrl && keyboard.just_pressed(KeyCode::KeyS)) {
        return;
    }

    do_save(&mut editor);
}

fn do_save(editor: &mut EditorState) {
    let errors = verify(&editor.meta);
    if !errors.is_empty() {
        let msg = format!("Validation failed: {}", errors[0]);
        editor.status_message = Some((msg, 5.0));
        return;
    }

    let output = toml::to_string_pretty(&editor.meta).unwrap_or_else(|e| {
        editor.status_message = Some((format!("Serialize error: {e}"), 5.0));
        String::new()
    });

    if output.is_empty() {
        return;
    }

    match std::fs::write(&editor.meta_path, &output) {
        Ok(()) => {
            editor.dirty = false;
            editor.status_message = Some(("Saved!".into(), 3.0));
        }
        Err(e) => {
            editor.status_message = Some((format!("Save failed: {e}"), 5.0));
        }
    }
}

fn update_window_title(
    editor: Res<EditorState>,
    browser: Res<BrowserState>,
    pipeline: Option<Res<PipelineInfo>>,
    mut windows: Query<&mut Window>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };

    let zoom_pct = (browser.zoom * 100.0).round() as i32;
    let dirty_marker = if editor.dirty { " *" } else { "" };
    let pack_prefix = pipeline
        .as_ref()
        .and_then(|p| p.config.packs.get(p.current_pack))
        .map(|pack| format!("[{}] ", pack.name))
        .unwrap_or_default();

    let current_label = editor
        .image_keys
        .get(editor.current_image)
        .cloned()
        .unwrap_or_else(|| "none".into());

    let gridded = editor
        .image_keys
        .iter()
        .filter(|k| sheet_for_image(&editor.meta, k).is_some())
        .count();

    window.title = format!(
        "{pack_prefix}Sprite Editor — [{}/{}] {current_label} — {gridded}/{} gridded — {zoom_pct}%{dirty_marker}",
        editor.current_image + 1,
        editor.image_keys.len(),
        editor.image_keys.len(),
    );
}

// ---------------------------------------------------------------------------
// egui: main UI
// ---------------------------------------------------------------------------

fn editor_ui(
    mut contexts: EguiContexts,
    mut editor: ResMut<EditorState>,
    mut browser: ResMut<BrowserState>,
    mut raw_image: ResMut<CurrentRawImage>,
    mut selection: ResMut<SpanSelection>,
    mut bevy_images: ResMut<Assets<Image>>,
    mut sprite_q: Query<&mut Sprite, With<EditorSprite>>,
    mut pipeline: Option<ResMut<PipelineInfo>>,
    hovered: Res<HoveredCell>,
    time: Res<Time>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Decay status message
    if let Some((_, ref mut ttl)) = editor.status_message {
        *ttl -= time.delta_secs_f64();
        if *ttl <= 0.0 {
            editor.status_message = None;
        }
    }

    egui::SidePanel::right("editor_panel")
        .default_width(340.0)
        .show(ctx, |ui| {
            // Pack selector (when PipelineInfo is present)
            if let Some(ref mut pipeline) = pipeline.as_deref_mut() {
                let packs = &pipeline.config.packs;
                let current_name = packs
                    .get(pipeline.current_pack)
                    .map(|p| p.name.as_str())
                    .unwrap_or("(none)");

                ui.horizontal(|ui| {
                    ui.label("Pack:");
                    let mut new_pack = pipeline.current_pack;
                    egui::ComboBox::from_id_salt("pack_selector")
                        .selected_text(current_name)
                        .show_ui(ui, |ui| {
                            for (i, pack) in packs.iter().enumerate() {
                                let meta_path = pipeline.config.meta_path(pack);
                                let exists = meta_path.exists();
                                let indicator = if exists { "[+]" } else { "[-]" };
                                let label = format!("{} {}", indicator, pack.name);
                                ui.selectable_value(&mut new_pack, i, label);
                            }
                        });

                    if new_pack != pipeline.current_pack {
                        // Auto-save if dirty
                        if editor.dirty {
                            do_save(&mut editor);
                        }

                        let pack = &packs[new_pack];
                        let meta_path = pipeline.config.meta_path(pack);
                        let pack_root = pipeline.config.pack_root(pack);

                        if let Some(new_state) = build_editor_state(
                            &meta_path,
                            &pack_root,
                            Some(&pipeline.config_path),
                        ) {
                            *editor = new_state;
                            pipeline.current_pack = new_pack;

                            // Reset browser state
                            browser.pan = Vec2::ZERO;
                            browser.fit_requested = true;
                            browser.cell_w = 0;
                            browser.cell_h = 0;
                            browser.grid_visible = false;
                            selection.start = None;
                            selection.end = None;
                            raw_image.rgba = None;
                            raw_image.image_key.clear();

                            // Reload display
                            if let Some(image_key) = editor.image_keys.first() {
                                if let Some(sheet_id) =
                                    sheet_for_image(&editor.meta, image_key)
                                {
                                    let sheet = &editor.meta.sheets[&sheet_id];
                                    browser.cell_w = sheet.cell_w;
                                    browser.cell_h = sheet.cell_h;
                                } else if let Some(img_entry) =
                                    editor.meta.images.get(image_key)
                                {
                                    browser.cell_w = default_cell_size(
                                        &img_entry.valid_cell_widths,
                                        img_entry.width,
                                    );
                                    browser.cell_h = default_cell_size(
                                        &img_entry.valid_cell_heights,
                                        img_entry.height,
                                    );
                                }
                            }
                            browser.grid_visible = true;
                            let handle =
                                load_grid_display(&editor, &mut raw_image, &mut bevy_images);
                            for mut sprite in &mut sprite_q {
                                sprite.image = handle.clone();
                            }
                        } else {
                            editor.status_message = Some((
                                format!(
                                    "TOML not found for '{}' — run discover first",
                                    pack.name
                                ),
                                5.0,
                            ));
                        }
                    }
                });

                // Pack stats
                if let Some(pack) = packs.get(pipeline.current_pack) {
                    let meta_path = pipeline.config.meta_path(pack);
                    if meta_path.exists() {
                        let img_count = editor.image_keys.len();
                        let gridded = editor
                            .image_keys
                            .iter()
                            .filter(|k| sheet_for_image(&editor.meta, k).is_some())
                            .count();
                        let catalog_count = editor.meta.catalog.len();
                        ui.label(format!(
                            "{img_count} images, {gridded} gridded, {catalog_count} catalog entries"
                        ));
                    }
                }

                ui.separator();
            }

            // Status message
            if let Some((ref msg, _)) = editor.status_message {
                ui.colored_label(egui::Color32::YELLOW, msg);
                ui.separator();
            }

            show_grid_panel(
                ui,
                &mut editor,
                &mut browser,
                &mut raw_image,
                &mut selection,
                &mut bevy_images,
                &mut sprite_q,
                &hovered,
            );

            // Save button
            ui.separator();
            let save_label = if editor.dirty {
                "Save (Ctrl+S) *"
            } else {
                "Save (Ctrl+S)"
            };
            if ui.button(save_label).clicked() {
                do_save(&mut editor);
            }
        });
}

// ---------------------------------------------------------------------------
// egui: grid panel
// ---------------------------------------------------------------------------

fn show_grid_panel(
    ui: &mut egui::Ui,
    editor: &mut ResMut<EditorState>,
    browser: &mut ResMut<BrowserState>,
    raw_image: &mut ResMut<CurrentRawImage>,
    selection: &mut ResMut<SpanSelection>,
    bevy_images: &mut ResMut<Assets<Image>>,
    sprite_q: &mut Query<&mut Sprite, With<EditorSprite>>,
    hovered: &HoveredCell,
) {
    ui.heading("Grid Editor");

    // Progress indicator
    let gridded = editor
        .image_keys
        .iter()
        .filter(|k| sheet_for_image(&editor.meta, k).is_some())
        .count();
    let total = editor.image_keys.len();
    ui.label(format!("Progress: {gridded}/{total} images gridded"));

    if gridded == total && total > 0 {
        ui.colored_label(
            egui::Color32::from_rgb(100, 255, 100),
            "All images gridded!",
        );
    }

    ui.separator();

    // Current image info
    let Some(image_key) = editor.image_keys.get(editor.current_image).cloned() else {
        ui.label("No images in metadata.");
        return;
    };

    ui.label(
        egui::RichText::new(&image_key)
            .strong()
            .size(14.0),
    );

    if let Some(img_entry) = editor.meta.images.get(&image_key) {
        ui.label(format!(
            "{}x{} px, {} colors, {}% transparent",
            img_entry.width, img_entry.height, img_entry.color_count, img_entry.transparent_pct
        ));
    }

    // Sheet status
    let existing_sheet = sheet_for_image(&editor.meta, &image_key);
    if let Some(ref sid) = existing_sheet {
        let entry_count = editor
            .meta
            .catalog
            .keys()
            .filter(|k| k.starts_with(&format!("{sid}.")))
            .count();
        ui.colored_label(
            egui::Color32::from_rgb(100, 200, 100),
            format!("Gridded: sheet \"{sid}\", {entry_count} catalog entries"),
        );
    } else {
        ui.colored_label(
            egui::Color32::from_rgb(200, 200, 100),
            "Not yet gridded",
        );
    }

    ui.separator();

    // Cell size controls
    ui.label("Cell Size:");

    let img_w = raw_image.rgba.as_ref().map(|r| r.width()).unwrap_or(0);
    let img_h = raw_image.rgba.as_ref().map(|r| r.height()).unwrap_or(0);

    let (valid_w, valid_h) = editor
        .meta
        .images
        .get(&image_key)
        .map(|e| {
            (
                e.valid_cell_widths.as_slice(),
                e.valid_cell_heights.as_slice(),
            )
        })
        .unwrap_or((&[], &[]));

    ui.horizontal(|ui| {
        ui.label("Width:");
        if ui.small_button("-").clicked() {
            browser.cell_w = prev_valid_size(valid_w, browser.cell_w, img_w);
        }
        ui.monospace(format!("{}", browser.cell_w));
        if ui.small_button("+").clicked() {
            browser.cell_w = next_valid_size(valid_w, browser.cell_w, img_w);
        }
        ui.label("px");
    });

    ui.horizontal(|ui| {
        ui.label("Height:");
        if ui.small_button("-").clicked() {
            browser.cell_h = prev_valid_size(valid_h, browser.cell_h, img_h);
        }
        ui.monospace(format!("{}", browser.cell_h));
        if ui.small_button("+").clicked() {
            browser.cell_h = next_valid_size(valid_h, browser.cell_h, img_h);
        }
        ui.label("px");
    });

    if browser.cell_w > 0 && browser.cell_h > 0 && img_w > 0 && img_h > 0 {
        let cols = img_w / browser.cell_w;
        let rows = img_h / browser.cell_h;
        ui.label(format!("Grid: {cols} cols x {rows} rows"));

        if let Some((col, row)) = hovered.cell {
            ui.monospace(format!("Cell: ({col}, {row})"));
        }

        // Show valid sizes as reference
        if !valid_w.is_empty() {
            ui.label(format!(
                "Valid widths: {:?}",
                valid_w
            ));
        }
        if !valid_h.is_empty() {
            ui.label(format!(
                "Valid heights: {:?}",
                valid_h
            ));
        }
    }

    ui.separator();

    // Apply Grid button
    if browser.cell_w > 0 && browser.cell_h > 0 {
        let label = if existing_sheet.is_some() {
            "Re-apply Grid (replaces existing)"
        } else {
            "Apply Grid"
        };
        if ui.button(label).clicked() {
            if let Some(ref rgba) = raw_image.rgba.clone() {
                let (sheet_id, count) = apply_grid_to_image(
                    &mut editor.meta,
                    &image_key,
                    rgba,
                    browser.cell_w,
                    browser.cell_h,
                );
                editor.dirty = true;
                editor.status_message = Some((
                    format!("Applied grid \"{sheet_id}\": {count} occupied cells"),
                    4.0,
                ));
            }
        }
    }

    // Clear Grid button
    if let Some(ref sid) = existing_sheet {
        if ui.button("Clear Grid").clicked() {
            let prefix = format!("{sid}.");
            let to_remove: Vec<String> = editor
                .meta
                .catalog
                .keys()
                .filter(|k| k.starts_with(&prefix) || *k == sid)
                .cloned()
                .collect();
            for id in &to_remove {
                editor.meta.catalog.remove(id);
            }
            editor.meta.sheets.remove(sid.as_str());
            editor.dirty = true;
            editor.status_message = Some(("Grid cleared.".into(), 3.0));
        }
    }

    ui.separator();

    // Span selection
    ui.label("Span Selection:");
    ui.label("Right-click drag on grid to select");

    if let (Some(start), Some(end)) = (selection.start, selection.end) {
        let min_col = start.0.min(end.0);
        let max_col = start.0.max(end.0);
        let min_row = start.1.min(end.1);
        let max_row = start.1.max(end.1);
        let col_span = max_col - min_col + 1;
        let row_span = max_row - min_row + 1;

        ui.label(format!(
            "Selected: ({},{}) to ({},{}) — {}x{} cells",
            min_col, min_row, max_col, max_row, col_span, row_span
        ));

        if col_span > 1 || row_span > 1 {
            if let Some(ref sid) = existing_sheet {
                if ui.button("Merge into Span").clicked() {
                    if let Some(ref rgba) = raw_image.rgba.clone() {
                        if let Some(merged_id) =
                            merge_span(&mut editor.meta, rgba, sid, start, end)
                        {
                            editor.dirty = true;
                            editor.status_message = Some((
                                format!("Merged {col_span}x{row_span} cells → {merged_id}"),
                                4.0,
                            ));
                        }
                    }
                    selection.start = None;
                    selection.end = None;
                }
            } else {
                ui.label("Apply grid first to create spans.");
            }
        }

        if ui.button("Clear Selection").clicked() {
            selection.start = None;
            selection.end = None;
        }
    } else {
        ui.label("No selection");
    }

    // Navigation slider
    ui.separator();
    if !editor.image_keys.is_empty() {
        let count = editor.image_keys.len();
        let mut idx = editor.current_image as f32;
        let max = (count - 1) as f32;
        let changed = ui
            .add(
                egui::Slider::new(&mut idx, 0.0..=max)
                    .step_by(1.0)
                    .text(format!("{}/{}", editor.current_image + 1, count)),
            )
            .changed();

        if changed {
            let new_idx = idx.round() as usize;
            if new_idx != editor.current_image {
                editor.current_image = new_idx;
                browser.pan = Vec2::ZERO;
                browser.fit_requested = true;
                selection.start = None;
                selection.end = None;

                let handle = load_grid_display(editor, raw_image, bevy_images);
                for mut sprite in &mut *sprite_q {
                    sprite.image = handle.clone();
                }

                let image_key = &editor.image_keys[editor.current_image];
                if let Some(sheet_id) = sheet_for_image(&editor.meta, image_key) {
                    let sheet = &editor.meta.sheets[&sheet_id];
                    browser.cell_w = sheet.cell_w;
                    browser.cell_h = sheet.cell_h;
                } else if let Some(img_entry) = editor.meta.images.get(image_key) {
                    browser.cell_w =
                        default_cell_size(&img_entry.valid_cell_widths, img_entry.width);
                    browser.cell_h =
                        default_cell_size(&img_entry.valid_cell_heights, img_entry.height);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = parse_cli();

    let (editor_state, pipeline_info) = match cli {
        CliMode::Config { config_path } => {
            let config = load_pipeline_config(&config_path);

            // Find first pack with an existing TOML
            let mut initial_pack = 0;
            for (i, pack) in config.packs.iter().enumerate() {
                if config.meta_path(pack).exists() {
                    initial_pack = i;
                    break;
                }
            }

            let pack = &config.packs[initial_pack];
            let meta_path = config.meta_path(pack);
            let pack_root = config.pack_root(pack);

            let state = build_editor_state(&meta_path, &pack_root, Some(&config_path))
                .unwrap_or_else(|| {
                    eprintln!(
                        "Cannot load pack '{}' — TOML not found at {}",
                        pack.name,
                        meta_path.display()
                    );
                    eprintln!("Run sprite_discover first:");
                    eprintln!(
                        "  cargo run --example sprite_discover -- --config {}",
                        config_path.display()
                    );
                    std::process::exit(1);
                });

            let pipeline = PipelineInfo {
                config,
                config_path,
                current_pack: initial_pack,
            };

            (state, Some(pipeline))
        }
        CliMode::Single {
            meta_path,
            pack_root,
        } => {
            let state = build_editor_state(&meta_path, &pack_root, None).unwrap_or_else(|| {
                eprintln!("Cannot load {}", meta_path.display());
                std::process::exit(1);
            });
            (state, None)
        }
    };

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        EguiPlugin::default(),
    ))
    .insert_resource(editor_state)
    .init_resource::<BrowserState>()
    .init_resource::<GamepadNav>()
    .init_resource::<Occupancy>()
    .init_resource::<SpanSelection>()
    .init_resource::<HoveredCell>()
    .init_resource::<CurrentRawImage>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            read_gamepad_input,
            navigate,
            pan_zoom,
            auto_fit_zoom,
            toggle_grid,
            update_occupancy,
            span_select,
            apply_camera,
            draw_grid,
            draw_occupancy,
            draw_spans,
            draw_span_selection,
            save_metadata,
            update_window_title,
        )
            .chain(),
    )
    .add_systems(EguiPrimaryContextPass, editor_ui);

    if let Some(pipeline) = pipeline_info {
        app.insert_resource(pipeline);
    }

    app.run();
}

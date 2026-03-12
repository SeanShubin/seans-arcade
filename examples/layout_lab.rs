//! Interactive layout lab — one layout config at a time, filling the window.
//!
//! - Left/Right: cycle configs within a category
//! - Up/Down: cycle categories
//!
//! Run with: `cargo run --example layout_lab`

use bevy::prelude::*;
use bevy::ui::{MaxTrackSizingFunction, MinTrackSizingFunction};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, LayoutLabPlugin))
        .run();
}

struct LayoutLabPlugin;

impl Plugin for LayoutLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LabState {
            category: 0,
            config: 0,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, handle_input);
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct LabState {
    category: usize,
    config: usize,
}

#[derive(Component)]
struct LayoutContainer;

#[derive(Component)]
struct LayoutArea;

#[derive(Component)]
struct HudText;

// ---------------------------------------------------------------------------
// Colors & constants
// ---------------------------------------------------------------------------

const FONT_SIZE: f32 = 16.0;
const BOX_SIZE: f32 = 200.0;

const BORDER_COLOR: Color = Color::srgb(0.4, 0.4, 0.4);
const CONTAINER_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);

const RED: Color = Color::srgb(1.0, 0.2, 0.2);
const ORANGE: Color = Color::srgb(1.0, 0.6, 0.2);
const YELLOW: Color = Color::srgb(1.0, 0.9, 0.2);
const GREEN: Color = Color::srgb(0.2, 0.8, 0.2);
const BLUE: Color = Color::srgb(0.3, 0.5, 1.0);
const VIOLET: Color = Color::srgb(0.6, 0.3, 0.9);

const COLORS: [Color; 6] = [RED, ORANGE, YELLOW, GREEN, BLUE, VIOLET];

// ---------------------------------------------------------------------------
// Category / Config registry
// ---------------------------------------------------------------------------

struct Category {
    name: &'static str,
    configs: &'static [Config],
}

struct Config {
    name: &'static str,
    spawn: fn(&mut Commands, Entity),
}

fn categories() -> &'static [Category] {
    &CATEGORIES
}

static CATEGORIES: [Category; 16] = [
    Category { name: "Flex Direction", configs: &FLEX_DIRECTION },
    Category { name: "Justify Content", configs: &JUSTIFY_CONTENT },
    Category { name: "Align Items", configs: &ALIGN_ITEMS },
    Category { name: "Flex Sizing", configs: &FLEX_SIZING },
    Category { name: "Flex Wrap", configs: &FLEX_WRAP },
    Category { name: "Align Content", configs: &ALIGN_CONTENT },
    Category { name: "Align Self", configs: &ALIGN_SELF },
    Category { name: "Gaps & Spacing", configs: &GAPS_SPACING },
    Category { name: "Grid Tracks", configs: &GRID_TRACKS },
    Category { name: "Grid Mixed", configs: &GRID_MIXED },
    Category { name: "Grid Rows", configs: &GRID_ROWS },
    Category { name: "Grid Repeat", configs: &GRID_REPEAT },
    Category { name: "Grid Alignment", configs: &GRID_ALIGNMENT },
    Category { name: "Grid Placement", configs: &GRID_PLACEMENT },
    Category { name: "Grid Auto Flow", configs: &GRID_AUTO_FLOW },
    Category { name: "Nesting & Patterns", configs: &NESTING_PATTERNS },
];

// ---------------------------------------------------------------------------
// Helper: spawn colored boxes
// ---------------------------------------------------------------------------

fn spawn_box(commands: &mut Commands, parent: Entity, color: Color, w: f32, h: f32) {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(w),
                height: Val::Px(h),
                ..default()
            },
            BackgroundColor(color),
        ))
        .id();
    commands.entity(parent).add_child(b);
}

fn spawn_boxes(commands: &mut Commands, parent: Entity, count: usize) {
    for i in 0..count {
        spawn_box(commands, parent, COLORS[i % COLORS.len()], BOX_SIZE, BOX_SIZE);
    }
}

fn spawn_numbered_box(commands: &mut Commands, color: Color, num: usize) -> Entity {
    let parent = commands
        .spawn((
            Node {
                width: Val::Px(BOX_SIZE),
                height: Val::Px(BOX_SIZE),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(color),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(num.to_string()),
            TextFont::from_font_size(FONT_SIZE),
            TextColor(Color::BLACK),
        ))
        .id();
    commands.entity(parent).add_child(text);
    parent
}

fn spawn_numbered_boxes(commands: &mut Commands, parent: Entity, count: usize) {
    for i in 0..count {
        let b = spawn_numbered_box(commands, COLORS[i % COLORS.len()], i + 1);
        commands.entity(parent).add_child(b);
    }
}

fn spawn_labeled_region(
    commands: &mut Commands,
    color: Color,
    label: &str,
    node: Node,
) -> Entity {
    let parent = commands
        .spawn((
            Node {
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..node
            },
            BackgroundColor(color),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            TextFont::from_font_size(14.0),
            TextColor(Color::BLACK),
        ))
        .id();
    commands.entity(parent).add_child(text);
    parent
}

fn spawn_sized_box(
    commands: &mut Commands,
    color: Color,
    grow: f32,
    shrink: f32,
    basis: Val,
    width: Val,
) -> Entity {
    commands
        .spawn((
            Node {
                width,
                height: Val::Px(BOX_SIZE),
                flex_grow: grow,
                flex_shrink: shrink,
                flex_basis: basis,
                ..default()
            },
            BackgroundColor(color),
        ))
        .id()
}

/// Full-window flex container (the layout being demonstrated).
fn full_flex(commands: &mut Commands, parent: Entity, node: Node) -> Entity {
    let c = commands
        .spawn((
            LayoutContainer,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..node
            },
            BackgroundColor(CONTAINER_BG),
            BorderColor::all(BORDER_COLOR),
        ))
        .id();
    commands.entity(parent).add_child(c);
    c
}

/// Full-window grid container.
fn full_grid(commands: &mut Commands, parent: Entity, node: Node) -> Entity {
    let c = commands
        .spawn((
            LayoutContainer,
            Node {
                display: Display::Grid,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..node
            },
            BackgroundColor(CONTAINER_BG),
            BorderColor::all(BORDER_COLOR),
        ))
        .id();
    commands.entity(parent).add_child(c);
    c
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

fn setup(mut commands: Commands, state: Res<LabState>) {
    commands.spawn(Camera2d);

    // Root: column layout, full window
    let root = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        })
        .id();

    // HUD bar — normal flow at the top
    let hud_bar = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
        ))
        .id();
    let hud_text = commands
        .spawn((
            HudText,
            Text::new(hud_label(&state)),
            TextFont::from_font_size(14.0),
            TextColor::WHITE,
        ))
        .id();
    commands.entity(hud_bar).add_child(hud_text);
    commands.entity(root).add_child(hud_bar);

    // Layout area — fills the remaining space below the HUD
    let area = commands
        .spawn((
            LayoutArea,
            Node {
                flex_grow: 1.0,
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(area);

    // Spawn initial layout into the area
    let cats = categories();
    (cats[state.category].configs[state.config].spawn)(&mut commands, area);
}

fn hud_label(state: &LabState) -> String {
    let cats = categories();
    let cat = &cats[state.category];
    let cfg = &cat.configs[state.config];
    format!(
        "{} ({}/{}) > {} ({}/{})\n[Up/Down] category  [Left/Right] config",
        cat.name,
        state.category + 1,
        cats.len(),
        cfg.name,
        state.config + 1,
        cat.configs.len(),
    )
}

// ---------------------------------------------------------------------------
// Input handling
// ---------------------------------------------------------------------------

fn handle_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LabState>,
    layout_q: Query<Entity, With<LayoutContainer>>,
    mut hud_q: Query<&mut Text, With<HudText>>,
    area_q: Query<Entity, With<LayoutArea>>,
) {
    let cats = categories();
    let mut changed = false;

    if keys.just_pressed(KeyCode::ArrowRight) {
        let max = cats[state.category].configs.len();
        state.config = (state.config + 1) % max;
        changed = true;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        let max = cats[state.category].configs.len();
        state.config = (state.config + max - 1) % max;
        changed = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        state.category = (state.category + 1) % cats.len();
        state.config = 0;
        changed = true;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        state.category = (state.category + cats.len() - 1) % cats.len();
        state.config = 0;
        changed = true;
    }

    if !changed {
        return;
    }

    // Despawn old layout
    for entity in layout_q.iter() {
        commands.entity(entity).despawn();
    }

    // Update HUD
    for mut text in hud_q.iter_mut() {
        *text = Text::new(hud_label(&state));
    }

    // Spawn new layout into the layout area
    if let Some(area) = area_q.iter().next() {
        (cats[state.category].configs[state.config].spawn)(&mut commands, area);
    }
}

// ===========================================================================
// FLEX CATEGORIES
// ===========================================================================

// ---- Flex Direction ----

static FLEX_DIRECTION: [Config; 4] = [
    Config { name: "Row", spawn: flex_dir_row },
    Config { name: "Column", spawn: flex_dir_col },
    Config { name: "RowReverse", spawn: flex_dir_row_rev },
    Config { name: "ColumnReverse", spawn: flex_dir_col_rev },
];

fn flex_dir_row(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), ..default() });
    spawn_boxes(commands, c, 5);
}
fn flex_dir_col(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() });
    spawn_boxes(commands, c, 5);
}
fn flex_dir_row_rev(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::RowReverse, column_gap: Val::Px(8.0), ..default() });
    spawn_boxes(commands, c, 5);
}
fn flex_dir_col_rev(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::ColumnReverse, row_gap: Val::Px(8.0), ..default() });
    spawn_boxes(commands, c, 5);
}

// ---- Justify Content ----

static JUSTIFY_CONTENT: [Config; 6] = [
    Config { name: "Start", spawn: jc_start },
    Config { name: "End", spawn: jc_end },
    Config { name: "Center", spawn: jc_center },
    Config { name: "SpaceBetween", spawn: jc_between },
    Config { name: "SpaceEvenly", spawn: jc_evenly },
    Config { name: "SpaceAround", spawn: jc_around },
];

fn jc_start(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { justify_content: JustifyContent::Start, column_gap: Val::Px(8.0), ..default() }); spawn_boxes(c, p, 5); }
fn jc_end(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { justify_content: JustifyContent::End, column_gap: Val::Px(8.0), ..default() }); spawn_boxes(c, p, 5); }
fn jc_center(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { justify_content: JustifyContent::Center, column_gap: Val::Px(8.0), ..default() }); spawn_boxes(c, p, 5); }
fn jc_between(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { justify_content: JustifyContent::SpaceBetween, ..default() }); spawn_boxes(c, p, 5); }
fn jc_evenly(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { justify_content: JustifyContent::SpaceEvenly, ..default() }); spawn_boxes(c, p, 5); }
fn jc_around(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { justify_content: JustifyContent::SpaceAround, ..default() }); spawn_boxes(c, p, 5); }

// ---- Align Items ----

static ALIGN_ITEMS: [Config; 5] = [
    Config { name: "Start", spawn: ai_start },
    Config { name: "End", spawn: ai_end },
    Config { name: "Center", spawn: ai_center },
    Config { name: "Baseline", spawn: ai_baseline },
    Config { name: "Stretch", spawn: ai_stretch },
];

fn ai_boxes(commands: &mut Commands, parent: Entity) {
    let heights = [80.0, 160.0, 120.0, 200.0, 60.0];
    for (i, &h) in heights.iter().enumerate() {
        spawn_box(commands, parent, COLORS[i % COLORS.len()], BOX_SIZE, h);
    }
}
fn ai_start(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { align_items: AlignItems::Start, column_gap: Val::Px(8.0), ..default() }); ai_boxes(c, p); }
fn ai_end(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { align_items: AlignItems::End, column_gap: Val::Px(8.0), ..default() }); ai_boxes(c, p); }
fn ai_center(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }); ai_boxes(c, p); }
fn ai_baseline(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { align_items: AlignItems::Baseline, column_gap: Val::Px(8.0), ..default() }); ai_boxes(c, p); }
fn ai_stretch(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { align_items: AlignItems::Stretch, column_gap: Val::Px(8.0), ..default() }); ai_boxes(c, p); }

// ---- Flex Sizing ----

static FLEX_SIZING: [Config; 6] = [
    Config { name: "grow=0", spawn: fs_grow0 },
    Config { name: "grow=1", spawn: fs_grow1 },
    Config { name: "grow 1:2:1", spawn: fs_grow121 },
    Config { name: "shrink=0", spawn: fs_shrink0 },
    Config { name: "shrink 1:3:1", spawn: fs_shrink131 },
    Config { name: "basis Auto/Px/Pct", spawn: fs_basis },
];

fn fs_grow0(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::Row, ..default() });
    for i in 0..3 { let b = spawn_sized_box(commands, COLORS[i], 0.0, 1.0, Val::Auto, Val::Px(BOX_SIZE)); commands.entity(c).add_child(b); }
}
fn fs_grow1(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::Row, ..default() });
    let grows = [1.0, 0.0, 0.0];
    for i in 0..3 { let b = spawn_sized_box(commands, COLORS[i], grows[i], 1.0, Val::Auto, Val::Px(BOX_SIZE)); commands.entity(c).add_child(b); }
}
fn fs_grow121(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::Row, ..default() });
    let grows = [1.0, 2.0, 1.0];
    for i in 0..3 { let b = spawn_sized_box(commands, COLORS[i], grows[i], 1.0, Val::Auto, Val::Px(BOX_SIZE)); commands.entity(c).add_child(b); }
}
fn fs_shrink0(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::Row, ..default() });
    let shrinks = [0.0, 1.0, 1.0];
    for i in 0..3 { let b = spawn_sized_box(commands, COLORS[i], 0.0, shrinks[i], Val::Auto, Val::Px(600.0)); commands.entity(c).add_child(b); }
}
fn fs_shrink131(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::Row, ..default() });
    let shrinks = [1.0, 3.0, 1.0];
    for i in 0..3 { let b = spawn_sized_box(commands, COLORS[i], 0.0, shrinks[i], Val::Auto, Val::Px(600.0)); commands.entity(c).add_child(b); }
}
fn fs_basis(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), ..default() });
    let b1 = spawn_sized_box(commands, COLORS[0], 0.0, 1.0, Val::Auto, Val::Px(200.0));
    let b2 = spawn_sized_box(commands, COLORS[1], 0.0, 1.0, Val::Px(400.0), Val::Auto);
    let b3 = spawn_sized_box(commands, COLORS[2], 0.0, 1.0, Val::Percent(30.0), Val::Auto);
    commands.entity(c).add_child(b1);
    commands.entity(c).add_child(b2);
    commands.entity(c).add_child(b3);
}

// ---- Flex Wrap ----

static FLEX_WRAP: [Config; 3] = [
    Config { name: "NoWrap", spawn: fw_nowrap },
    Config { name: "Wrap", spawn: fw_wrap },
    Config { name: "WrapReverse", spawn: fw_wrap_rev },
];

fn fw_nowrap(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { flex_wrap: FlexWrap::NoWrap, column_gap: Val::Px(8.0), ..default() }); spawn_boxes(c, p, 12); }
fn fw_wrap(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(8.0), row_gap: Val::Px(8.0), ..default() }); spawn_boxes(c, p, 12); }
fn fw_wrap_rev(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { flex_wrap: FlexWrap::WrapReverse, column_gap: Val::Px(8.0), row_gap: Val::Px(8.0), ..default() }); spawn_boxes(c, p, 12); }

// ---- Align Content ----

static ALIGN_CONTENT: [Config; 6] = [
    Config { name: "Start", spawn: ac_start },
    Config { name: "End", spawn: ac_end },
    Config { name: "Center", spawn: ac_center },
    Config { name: "SpaceBetween", spawn: ac_between },
    Config { name: "SpaceEvenly", spawn: ac_evenly },
    Config { name: "Stretch", spawn: ac_stretch },
];

fn ac_base(c: &mut Commands, r: Entity, align: AlignContent) {
    let p = full_flex(c, r, Node { flex_wrap: FlexWrap::Wrap, align_content: align, column_gap: Val::Px(8.0), row_gap: Val::Px(8.0), ..default() });
    spawn_boxes(c, p, 12);
}
fn ac_start(c: &mut Commands, r: Entity) { ac_base(c, r, AlignContent::Start); }
fn ac_end(c: &mut Commands, r: Entity) { ac_base(c, r, AlignContent::End); }
fn ac_center(c: &mut Commands, r: Entity) { ac_base(c, r, AlignContent::Center); }
fn ac_between(c: &mut Commands, r: Entity) { ac_base(c, r, AlignContent::SpaceBetween); }
fn ac_evenly(c: &mut Commands, r: Entity) { ac_base(c, r, AlignContent::SpaceEvenly); }
fn ac_stretch(c: &mut Commands, r: Entity) { ac_base(c, r, AlignContent::Stretch); }

// ---- Align Self ----

static ALIGN_SELF: [Config; 5] = [
    Config { name: "Start", spawn: as_start },
    Config { name: "End", spawn: as_end },
    Config { name: "Center", spawn: as_center },
    Config { name: "Baseline", spawn: as_baseline },
    Config { name: "Stretch", spawn: as_stretch },
];

fn as_base(commands: &mut Commands, root: Entity, align: AlignSelf) {
    let c = full_flex(commands, root, Node { column_gap: Val::Px(8.0), align_items: AlignItems::Start, ..default() });
    let heights = [80.0, 160.0, 120.0, 200.0, 60.0];
    for (i, &h) in heights.iter().enumerate() {
        let b = commands.spawn((
            Node {
                width: Val::Px(BOX_SIZE),
                height: Val::Px(h),
                align_self: align,
                ..default()
            },
            BackgroundColor(COLORS[i % COLORS.len()]),
        )).id();
        commands.entity(c).add_child(b);
    }
}
fn as_start(c: &mut Commands, r: Entity) { as_base(c, r, AlignSelf::Start); }
fn as_end(c: &mut Commands, r: Entity) { as_base(c, r, AlignSelf::End); }
fn as_center(c: &mut Commands, r: Entity) { as_base(c, r, AlignSelf::Center); }
fn as_baseline(c: &mut Commands, r: Entity) { as_base(c, r, AlignSelf::Baseline); }
fn as_stretch(c: &mut Commands, r: Entity) { as_base(c, r, AlignSelf::Stretch); }

// ---- Gaps & Spacing ----

static GAPS_SPACING: [Config; 6] = [
    Config { name: "No gap", spawn: gap_none },
    Config { name: "col-gap 10", spawn: gap_col10 },
    Config { name: "col-gap 30", spawn: gap_col30 },
    Config { name: "row-gap + wrap", spawn: gap_row_wrap },
    Config { name: "padding 20", spawn: gap_padding },
    Config { name: "margin 10", spawn: gap_margin },
];

fn gap_none(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { ..default() }); spawn_boxes(c, p, 6); }
fn gap_col10(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { column_gap: Val::Px(10.0), ..default() }); spawn_boxes(c, p, 6); }
fn gap_col30(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { column_gap: Val::Px(30.0), ..default() }); spawn_boxes(c, p, 6); }
fn gap_row_wrap(c: &mut Commands, r: Entity) { let p = full_flex(c, r, Node { flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(10.0), column_gap: Val::Px(10.0), ..default() }); spawn_boxes(c, p, 12); }
fn gap_padding(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { padding: UiRect::all(Val::Px(20.0)), column_gap: Val::Px(8.0), ..default() });
    spawn_boxes(commands, c, 6);
}
fn gap_margin(commands: &mut Commands, root: Entity) {
    let c = full_flex(commands, root, Node { column_gap: Val::Px(4.0), ..default() });
    for i in 0..6 {
        let b = commands.spawn((
            Node {
                width: Val::Px(BOX_SIZE),
                height: Val::Px(BOX_SIZE),
                margin: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(COLORS[i % COLORS.len()]),
        )).id();
        commands.entity(c).add_child(b);
    }
}

// ===========================================================================
// GRID CATEGORIES
// ===========================================================================

// ---- Grid Tracks ----

static GRID_TRACKS: [Config; 6] = [
    Config { name: "px(100)", spawn: gt_px },
    Config { name: "flex(1)", spawn: gt_flex },
    Config { name: "auto", spawn: gt_auto },
    Config { name: "percent(25)", spawn: gt_percent },
    Config { name: "min_content", spawn: gt_min },
    Config { name: "max_content", spawn: gt_max },
];

fn gt_px(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::px(100.0); 4], ..default() });
    spawn_numbered_boxes(c, p, 4);
}
fn gt_flex(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::flex(1.0); 4], ..default() });
    spawn_numbered_boxes(c, p, 8);
}
fn gt_auto(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::auto(); 4], ..default() });
    spawn_numbered_boxes(c, p, 4);
}
fn gt_percent(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::percent(25.0); 4], ..default() });
    spawn_numbered_boxes(c, p, 4);
}
fn gt_min(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::min_content(); 4], ..default() });
    spawn_numbered_boxes(c, p, 4);
}
fn gt_max(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::max_content(); 4], ..default() });
    spawn_numbered_boxes(c, p, 4);
}

// ---- Grid Mixed ----

static GRID_MIXED: [Config; 3] = [
    Config { name: "sidebar+flex+sidebar", spawn: gm_sidebar },
    Config { name: "1:2:1", spawn: gm_121 },
    Config { name: "auto+flex+fixed", spawn: gm_auto_flex_fixed },
];

fn gm_sidebar(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::px(100.0), GridTrack::flex(1.0), GridTrack::px(100.0)], ..default() });
    spawn_numbered_boxes(c, p, 6);
}
fn gm_121(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::flex(1.0), GridTrack::flex(2.0), GridTrack::flex(1.0)], ..default() });
    spawn_numbered_boxes(c, p, 6);
}
fn gm_auto_flex_fixed(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![GridTrack::auto(), GridTrack::flex(1.0), GridTrack::px(200.0)], ..default() });
    spawn_numbered_boxes(c, p, 6);
}

// ---- Grid Rows ----

static GRID_ROWS: [Config; 3] = [
    Config { name: "fixed heights", spawn: gr_fixed },
    Config { name: "proportional", spawn: gr_proportional },
    Config { name: "auto+flex", spawn: gr_auto_flex },
];

fn gr_fixed(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![GridTrack::px(80.0), GridTrack::px(80.0)],
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}
fn gr_proportional(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![GridTrack::flex(1.0), GridTrack::flex(2.0)],
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}
fn gr_auto_flex(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![GridTrack::auto(), GridTrack::flex(1.0)],
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}

// ---- Grid Repeat ----

static GRID_REPEAT: [Config; 3] = [
    Config { name: "repeat flex", spawn: grep_flex },
    Config { name: "repeat minmax", spawn: grep_minmax },
    Config { name: "fixed+repeated", spawn: grep_fixed_rep },
];

fn grep_flex(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node { grid_template_columns: vec![RepeatedGridTrack::flex(4, 1.0)], ..default() });
    spawn_numbered_boxes(c, p, 8);
}
fn grep_minmax(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![RepeatedGridTrack::minmax(3, MinTrackSizingFunction::Px(80.0), MaxTrackSizingFunction::Px(200.0))],
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}
fn grep_fixed_rep(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![GridTrack::px(100.0), RepeatedGridTrack::flex(3, 1.0)],
        ..default()
    });
    spawn_numbered_boxes(c, p, 8);
}

// ---- Grid Alignment ----

static GRID_ALIGNMENT: [Config; 6] = [
    Config { name: "JustifyItems::Start", spawn: ga_ji_start },
    Config { name: "JustifyItems::Center", spawn: ga_ji_center },
    Config { name: "AlignItems::Center", spawn: ga_ai_center },
    Config { name: "JustifyContent::Center", spawn: ga_jc_center },
    Config { name: "AlignContent::Center", spawn: ga_ac_center },
    Config { name: "Self overrides", spawn: ga_self },
];

fn ga_ji_start(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::flex(2, 1.0)],
        justify_items: JustifyItems::Start,
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}
fn ga_ji_center(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::flex(2, 1.0)],
        justify_items: JustifyItems::Center,
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}
fn ga_ai_center(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::flex(2, 1.0)],
        align_items: AlignItems::Center,
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}
fn ga_jc_center(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![GridTrack::px(80.0), GridTrack::px(80.0), GridTrack::px(80.0)],
        grid_template_rows: vec![GridTrack::px(60.0), GridTrack::px(60.0)],
        justify_content: JustifyContent::Center,
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}
fn ga_ac_center(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![GridTrack::px(60.0), GridTrack::px(60.0)],
        align_content: AlignContent::Center,
        ..default()
    });
    spawn_numbered_boxes(c, p, 6);
}
fn ga_self(commands: &mut Commands, root: Entity) {
    let p = full_grid(commands, root, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::flex(2, 1.0)],
        ..default()
    });
    let positions: [(JustifySelf, AlignSelf); 4] = [
        (JustifySelf::Start, AlignSelf::Start),
        (JustifySelf::End, AlignSelf::Start),
        (JustifySelf::Start, AlignSelf::End),
        (JustifySelf::End, AlignSelf::End),
    ];
    for (i, (js, als)) in positions.iter().enumerate() {
        let b = commands.spawn((
            Node {
                width: Val::Px(BOX_SIZE),
                height: Val::Px(BOX_SIZE),
                justify_self: *js,
                align_self: *als,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(COLORS[i]),
        )).id();
        let text = commands.spawn((
            Text::new((i + 1).to_string()),
            TextFont::from_font_size(FONT_SIZE),
            TextColor(Color::BLACK),
        )).id();
        commands.entity(b).add_child(text);
        commands.entity(p).add_child(b);
    }
}

// ---- Grid Placement ----

static GRID_PLACEMENT: [Config; 5] = [
    Config { name: "auto", spawn: gp_auto },
    Config { name: "start", spawn: gp_start },
    Config { name: "span", spawn: gp_span },
    Config { name: "start_end", spawn: gp_start_end },
    Config { name: "row+col combined", spawn: gp_combined },
];

fn grid_4x3_node() -> Node {
    Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(4, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::flex(3, 1.0)],
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(4.0),
        ..default()
    }
}

fn box_node() -> Node {
    Node {
        width: Val::Px(BOX_SIZE),
        height: Val::Px(BOX_SIZE),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn spawn_box_with_node(commands: &mut Commands, color: Color, num: usize, node: Node) -> Entity {
    let parent = commands.spawn((node, BackgroundColor(color))).id();
    let text = commands.spawn((
        Text::new(num.to_string()),
        TextFont::from_font_size(FONT_SIZE),
        TextColor(Color::BLACK),
    )).id();
    commands.entity(parent).add_child(text);
    parent
}

fn gp_auto(c: &mut Commands, r: Entity) {
    let p = full_grid(c, r, grid_4x3_node());
    spawn_numbered_boxes(c, p, 8);
}

fn gp_start(commands: &mut Commands, root: Entity) {
    let p = full_grid(commands, root, grid_4x3_node());
    let b1 = spawn_box_with_node(commands, COLORS[0], 1, Node { grid_column: GridPlacement::start(1), ..box_node() });
    let b2 = spawn_box_with_node(commands, COLORS[1], 2, Node { grid_column: GridPlacement::start(3), ..box_node() });
    let b3 = spawn_box_with_node(commands, COLORS[2], 3, Node { grid_column: GridPlacement::start(2), grid_row: GridPlacement::start(2), ..box_node() });
    let b4 = spawn_box_with_node(commands, COLORS[3], 4, Node { grid_column: GridPlacement::start(4), grid_row: GridPlacement::start(2), ..box_node() });
    for b in [b1, b2, b3, b4] { commands.entity(p).add_child(b); }
}

fn gp_span(commands: &mut Commands, root: Entity) {
    let p = full_grid(commands, root, grid_4x3_node());
    let b1 = spawn_box_with_node(commands, COLORS[0], 1, Node { grid_column: GridPlacement::span(2), height: Val::Px(BOX_SIZE), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() });
    let b2 = spawn_numbered_box(commands, COLORS[1], 2);
    let b3 = spawn_numbered_box(commands, COLORS[2], 3);
    let b4 = spawn_numbered_box(commands, COLORS[3], 4);
    for b in [b1, b2, b3, b4] { commands.entity(p).add_child(b); }
}

fn gp_start_end(commands: &mut Commands, root: Entity) {
    let p = full_grid(commands, root, grid_4x3_node());
    let b1 = spawn_box_with_node(commands, COLORS[0], 1, Node { grid_column: GridPlacement::start_end(1, 3), height: Val::Px(BOX_SIZE), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() });
    let b2 = spawn_numbered_box(commands, COLORS[1], 2);
    let b3 = spawn_numbered_box(commands, COLORS[2], 3);
    let b4 = spawn_numbered_box(commands, COLORS[3], 4);
    for b in [b1, b2, b3, b4] { commands.entity(p).add_child(b); }
}

fn gp_combined(commands: &mut Commands, root: Entity) {
    let p = full_grid(commands, root, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::flex(3, 1.0)],
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(4.0),
        ..default()
    });
    let b1 = spawn_box_with_node(commands, COLORS[0], 1, Node { grid_column: GridPlacement::start(1), grid_row: GridPlacement::start(1), ..box_node() });
    let b2 = spawn_box_with_node(commands, COLORS[1], 2, Node { grid_column: GridPlacement::start(3), grid_row: GridPlacement::start(1), ..box_node() });
    let b3 = spawn_box_with_node(commands, COLORS[2], 3, Node { grid_column: GridPlacement::start(2), grid_row: GridPlacement::start(2), ..box_node() });
    let b4 = spawn_box_with_node(commands, COLORS[3], 4, Node { grid_column: GridPlacement::start(3), grid_row: GridPlacement::start(3), ..box_node() });
    for b in [b1, b2, b3, b4] { commands.entity(p).add_child(b); }
}

// ---- Grid Auto Flow ----

static GRID_AUTO_FLOW: [Config; 4] = [
    Config { name: "Row", spawn: gaf_row },
    Config { name: "Column", spawn: gaf_col },
    Config { name: "RowDense", spawn: gaf_row_dense },
    Config { name: "ColumnDense", spawn: gaf_col_dense },
];

fn gaf_base(commands: &mut Commands, root: Entity, flow: GridAutoFlow) {
    let p = full_grid(commands, root, Node {
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_auto_flow: flow,
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(4.0),
        ..default()
    });
    // Place box 1 explicitly at col 2, leaving a gap
    let b1 = spawn_box_with_node(commands, COLORS[0], 1, Node { grid_column: GridPlacement::start(2), grid_row: GridPlacement::start(1), ..box_node() });
    commands.entity(p).add_child(b1);
    for i in 2..=7 {
        let b = spawn_numbered_box(commands, COLORS[(i - 1) % COLORS.len()], i);
        commands.entity(p).add_child(b);
    }
}
fn gaf_row(c: &mut Commands, r: Entity) { gaf_base(c, r, GridAutoFlow::Row); }
fn gaf_col(c: &mut Commands, r: Entity) { gaf_base(c, r, GridAutoFlow::Column); }
fn gaf_row_dense(c: &mut Commands, r: Entity) { gaf_base(c, r, GridAutoFlow::RowDense); }
fn gaf_col_dense(c: &mut Commands, r: Entity) { gaf_base(c, r, GridAutoFlow::ColumnDense); }

// ---- Nesting & Patterns ----

static NESTING_PATTERNS: [Config; 4] = [
    Config { name: "Holy Grail", spawn: np_holy_grail },
    Config { name: "Dashboard", spawn: np_dashboard },
    Config { name: "Photo Gallery", spawn: np_gallery },
    Config { name: "Grid+Flex mix", spawn: np_grid_flex },
];

fn np_holy_grail(commands: &mut Commands, root: Entity) {
    let c = commands.spawn((
        LayoutContainer,
        Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(CONTAINER_BG),
    )).id();
    commands.entity(root).add_child(c);

    let header = spawn_labeled_region(commands, RED, "Header", Node { width: Val::Percent(100.0), height: Val::Px(50.0), ..default() });
    let middle = commands.spawn(Node { flex_direction: FlexDirection::Row, flex_grow: 1.0, column_gap: Val::Px(4.0), ..default() }).id();
    let left = spawn_labeled_region(commands, ORANGE, "Left", Node { width: Val::Px(120.0), ..default() });
    let content = spawn_labeled_region(commands, YELLOW, "Content", Node { flex_grow: 1.0, ..default() });
    let right = spawn_labeled_region(commands, GREEN, "Right", Node { width: Val::Px(120.0), ..default() });
    commands.entity(middle).add_child(left);
    commands.entity(middle).add_child(content);
    commands.entity(middle).add_child(right);
    let footer = spawn_labeled_region(commands, BLUE, "Footer", Node { width: Val::Percent(100.0), height: Val::Px(50.0), ..default() });

    commands.entity(c).add_child(header);
    commands.entity(c).add_child(middle);
    commands.entity(c).add_child(footer);
}

fn np_dashboard(commands: &mut Commands, root: Entity) {
    let c = commands.spawn((
        LayoutContainer,
        Node {
            display: Display::Grid,
            grid_template_columns: vec![GridTrack::px(120.0), GridTrack::flex(1.0), GridTrack::flex(1.0)],
            grid_template_rows: vec![GridTrack::px(50.0), GridTrack::flex(1.0), GridTrack::flex(1.0)],
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(CONTAINER_BG),
    )).id();
    commands.entity(root).add_child(c);

    let header = spawn_labeled_region(commands, RED, "Header", Node { grid_column: GridPlacement::span(3), ..default() });
    let sidebar = spawn_labeled_region(commands, ORANGE, "Sidebar", Node { grid_row: GridPlacement::span(2), ..default() });
    let card1 = spawn_labeled_region(commands, YELLOW, "Card 1", Node::default());
    let card2 = spawn_labeled_region(commands, GREEN, "Card 2", Node::default());
    let card3 = spawn_labeled_region(commands, BLUE, "Card 3", Node::default());
    let card4 = spawn_labeled_region(commands, VIOLET, "Card 4", Node::default());

    for e in [header, sidebar, card1, card2, card3, card4] { commands.entity(c).add_child(e); }
}

fn np_gallery(commands: &mut Commands, root: Entity) {
    let c = commands.spawn((
        LayoutContainer,
        Node {
            display: Display::Grid,
            grid_template_columns: vec![RepeatedGridTrack::flex(4, 1.0)],
            grid_auto_rows: vec![GridTrack::flex(1.0)],
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(CONTAINER_BG),
    )).id();
    commands.entity(root).add_child(c);

    let p1 = spawn_labeled_region(commands, RED, "Wide", Node { grid_column: GridPlacement::span(2), ..default() });
    let p2 = spawn_labeled_region(commands, ORANGE, "1x1", Node::default());
    let p3 = spawn_labeled_region(commands, YELLOW, "1x1", Node::default());
    let p4 = spawn_labeled_region(commands, GREEN, "1x1", Node::default());
    let p5 = spawn_labeled_region(commands, BLUE, "Tall", Node { grid_row: GridPlacement::span(2), ..default() });
    let p6 = spawn_labeled_region(commands, VIOLET, "Large", Node { grid_column: GridPlacement::span(2), grid_row: GridPlacement::span(2), ..default() });
    let p7 = spawn_labeled_region(commands, RED, "1x1", Node::default());
    let p8 = spawn_labeled_region(commands, ORANGE, "1x1", Node::default());

    for e in [p1, p2, p3, p4, p5, p6, p7, p8] { commands.entity(c).add_child(e); }
}

fn np_grid_flex(commands: &mut Commands, root: Entity) {
    let c = commands.spawn((
        LayoutContainer,
        Node {
            display: Display::Grid,
            grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
            grid_template_rows: vec![GridTrack::flex(1.0)],
            column_gap: Val::Px(8.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(CONTAINER_BG),
    )).id();
    commands.entity(root).add_child(c);

    // Left cell: flex row
    let flex_row = commands.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.03)),
    )).id();
    for i in 0..4 {
        let b = spawn_numbered_box(commands, COLORS[i], i + 1);
        commands.entity(flex_row).add_child(b);
    }

    // Right cell: flex column
    let flex_col = commands.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            align_items: AlignItems::Start,
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.03)),
    )).id();
    for i in 0..4 {
        let b = spawn_numbered_box(commands, COLORS[(i + 2) % COLORS.len()], i + 5);
        commands.entity(flex_col).add_child(b);
    }

    commands.entity(c).add_child(flex_row);
    commands.entity(c).add_child(flex_col);
}

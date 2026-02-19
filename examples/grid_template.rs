//! Demonstrates grid track definitions with GridTrack and RepeatedGridTrack.
//!
//! Run with: `cargo run --example grid_template`

use bevy::prelude::*;
use bevy::ui::{MaxTrackSizingFunction, MinTrackSizingFunction};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, GridTemplatePlugin))
        .run();
}

struct GridTemplatePlugin;

impl Plugin for GridTemplatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui);
    }
}

const FONT_SIZE: f32 = 16.0;
const BOX_SIZE: f32 = 40.0;
const SECTION_GAP: f32 = 10.0;

const BORDER_COLOR: Color = Color::srgb(0.4, 0.4, 0.4);
const CONTAINER_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);
const HEADER_COLOR: Color = Color::srgb(1.0, 0.9, 0.4);

const CORAL: Color = Color::srgb(1.0, 0.5, 0.31);
const SKY_BLUE: Color = Color::srgb(0.53, 0.81, 0.92);
const LIME: Color = Color::srgb(0.2, 0.8, 0.2);
const GOLD: Color = Color::srgb(1.0, 0.84, 0.0);

const BOX_COLORS: [Color; 4] = [CORAL, SKY_BLUE, LIME, GOLD];

fn spawn_box(commands: &mut Commands, color: Color, num: usize) -> Entity {
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

fn spawn_boxes(commands: &mut Commands, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|i| spawn_box(commands, BOX_COLORS[i % 4], i + 1))
        .collect()
}

fn spawn_section(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    container_node: Node,
    children: &[Entity],
) {
    let section = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();

    let label_entity = commands
        .spawn((
            Text::new(label),
            TextFont::from_font_size(FONT_SIZE),
            TextColor::WHITE,
        ))
        .id();

    let container = commands
        .spawn((
            container_node,
            BackgroundColor(CONTAINER_BG),
            BorderColor::all(BORDER_COLOR),
        ))
        .id();

    for &child in children {
        commands.entity(container).add_child(child);
    }

    commands.entity(section).add_child(label_entity);
    commands.entity(section).add_child(container);
    commands.entity(parent).add_child(section);
}

fn spawn_part_header(commands: &mut Commands, parent: Entity, text: &str) {
    let header = commands
        .spawn((
            Text::new(text),
            TextFont::from_font_size(FONT_SIZE + 4.0),
            TextColor(HEADER_COLOR),
            Node {
                margin: UiRect {
                    top: Val::Px(12.0),
                    bottom: Val::Px(4.0),
                    ..default()
                },
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();
    commands.entity(parent).add_child(header);
}

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    let root = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(16.0)),
            row_gap: Val::Px(SECTION_GAP),
            overflow: Overflow::scroll_y(),
            ..default()
        })
        .id();

    // ---- Part A: GridTrack types ----
    spawn_part_header(&mut commands, root, "GridTrack Types");

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "GridTrack::px(100.0) — 4 fixed 100px columns",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::px(100.0),
                GridTrack::px(100.0),
                GridTrack::px(100.0),
                GridTrack::px(100.0),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "GridTrack::flex(1.0) — 4 equal fr columns",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "GridTrack::auto() — 4 auto-sized columns",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::auto(),
                GridTrack::auto(),
                GridTrack::auto(),
                GridTrack::auto(),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "GridTrack::percent(25.0) — 4 columns at 25% each",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::percent(25.0),
                GridTrack::percent(25.0),
                GridTrack::percent(25.0),
                GridTrack::percent(25.0),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "GridTrack::min_content() — 4 min-content columns",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::min_content(),
                GridTrack::min_content(),
                GridTrack::min_content(),
                GridTrack::min_content(),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "GridTrack::max_content() — 4 max-content columns",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::max_content(),
                GridTrack::max_content(),
                GridTrack::max_content(),
                GridTrack::max_content(),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // ---- Part B: Mixed track sizes ----
    spawn_part_header(&mut commands, root, "Mixed Track Sizes");

    let boxes = spawn_boxes(&mut commands, 3);
    spawn_section(
        &mut commands,
        root,
        "[px(100), flex(1), px(100)] — fixed sidebars, flexible center",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::px(100.0),
                GridTrack::flex(1.0),
                GridTrack::px(100.0),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "[flex(1), flex(2), flex(1)] — proportional columns (1:2:1)",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::flex(1.0),
                GridTrack::flex(2.0),
                GridTrack::flex(1.0),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 3);
    spawn_section(
        &mut commands,
        root,
        "[auto, flex(1), px(200)] — auto + flexible + fixed",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::auto(),
                GridTrack::flex(1.0),
                GridTrack::px(200.0),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // ---- Part C: Rows ----
    spawn_part_header(&mut commands, root, "Row Definitions");

    let boxes = spawn_boxes(&mut commands, 6);
    spawn_section(
        &mut commands,
        root,
        "grid_template_rows: [px(60), px(60)] — fixed row heights",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
            ],
            grid_template_rows: vec![GridTrack::px(60.0), GridTrack::px(60.0)],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 6);
    spawn_section(
        &mut commands,
        root,
        "grid_template_rows: [flex(1), flex(2)] — proportional rows (container 200px)",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
            ],
            grid_template_rows: vec![GridTrack::flex(1.0), GridTrack::flex(2.0)],
            width: Val::Percent(100.0),
            height: Val::Px(200.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 6);
    spawn_section(
        &mut commands,
        root,
        "grid_template_rows: [auto, flex(1)] — auto first row, flexible second",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
                GridTrack::flex(1.0),
            ],
            grid_template_rows: vec![GridTrack::auto(), GridTrack::flex(1.0)],
            width: Val::Percent(100.0),
            height: Val::Px(200.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // ---- Part D: RepeatedGridTrack ----
    spawn_part_header(&mut commands, root, "RepeatedGridTrack");

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "RepeatedGridTrack::flex(4, 1.0) — 4 equal columns via repeat",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![RepeatedGridTrack::flex(4, 1.0)],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 3);
    spawn_section(
        &mut commands,
        root,
        "RepeatedGridTrack::minmax(3, Px(80), Px(200)) — 3 columns with min/max",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![RepeatedGridTrack::minmax(
                3,
                MinTrackSizingFunction::Px(80.0),
                MaxTrackSizingFunction::Px(200.0),
            )],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = spawn_boxes(&mut commands, 4);
    spawn_section(
        &mut commands,
        root,
        "[GridTrack::px(100), RepeatedGridTrack::flex(3, 1.0)] — fixed + repeated",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::px(100.0),
                RepeatedGridTrack::flex(3, 1.0),
            ],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );
}

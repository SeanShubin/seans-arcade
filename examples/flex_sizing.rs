//! Demonstrates flex_grow, flex_shrink, and flex_basis.
//!
//! Run with: `cargo run --example flex_sizing`

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FlexSizingPlugin))
        .run();
}

struct FlexSizingPlugin;

impl Plugin for FlexSizingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui);
    }
}

const FONT_SIZE: f32 = 16.0;
const BOX_SIZE: f32 = 40.0;
const SECTION_GAP: f32 = 10.0;
const CONTAINER_HEIGHT: f32 = 60.0;

const BORDER_COLOR: Color = Color::srgb(0.4, 0.4, 0.4);
const CONTAINER_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);
const HEADER_COLOR: Color = Color::srgb(1.0, 0.9, 0.4);

const CORAL: Color = Color::srgb(1.0, 0.5, 0.31);
const SKY_BLUE: Color = Color::srgb(0.53, 0.81, 0.92);
const LIME: Color = Color::srgb(0.2, 0.8, 0.2);
const GOLD: Color = Color::srgb(1.0, 0.84, 0.0);

const BOX_COLORS: [Color; 4] = [CORAL, SKY_BLUE, LIME, GOLD];

fn spawn_section(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    container_node: Node,
    boxes: &[Entity],
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

    for &box_entity in boxes {
        commands.entity(container).add_child(box_entity);
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

fn row_container() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        width: Val::Percent(100.0),
        height: Val::Px(CONTAINER_HEIGHT),
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    }
}

fn sized_box(commands: &mut Commands, color: Color, grow: f32, shrink: f32, basis: Val, width: Val) -> Entity {
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

    // ---- Part A: flex_grow ----
    spawn_part_header(&mut commands, root, "flex_grow");

    // All grow: 0 — boxes stay at natural size
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], 0.0, 1.0, Val::Auto, Val::Px(BOX_SIZE)))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "All flex_grow: 0 (boxes stay at natural size)",
        row_container(),
        &boxes,
    );

    // First grow: 1, others 0
    let grows = [1.0, 0.0, 0.0];
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], grows[i], 1.0, Val::Auto, Val::Px(BOX_SIZE)))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "flex_grow: 1, 0, 0 (first fills remaining space)",
        row_container(),
        &boxes,
    );

    // Grow: 1, 2, 1
    let grows = [1.0, 2.0, 1.0];
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], grows[i], 1.0, Val::Auto, Val::Px(BOX_SIZE)))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "flex_grow: 1, 2, 1 (space distributed proportionally)",
        row_container(),
        &boxes,
    );

    // ---- Part B: flex_shrink ----
    spawn_part_header(&mut commands, root, "flex_shrink");

    // All shrink: 1 — boxes shrink equally
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], 0.0, 1.0, Val::Auto, Val::Px(250.0)))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "All flex_shrink: 1 (boxes shrink equally, each 250px requested)",
        row_container(),
        &boxes,
    );

    // First shrink: 0, others 1
    let shrinks = [0.0, 1.0, 1.0];
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], 0.0, shrinks[i], Val::Auto, Val::Px(250.0)))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "flex_shrink: 0, 1, 1 (first keeps size, others shrink)",
        row_container(),
        &boxes,
    );

    // Shrink: 1, 3, 1
    let shrinks = [1.0, 3.0, 1.0];
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], 0.0, shrinks[i], Val::Auto, Val::Px(250.0)))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "flex_shrink: 1, 3, 1 (middle shrinks more)",
        row_container(),
        &boxes,
    );

    // ---- Part C: flex_basis ----
    spawn_part_header(&mut commands, root, "flex_basis");

    // Auto with varying widths
    let widths = [60.0, 100.0, 40.0];
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], 0.0, 1.0, Val::Auto, Val::Px(widths[i])))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "flex_basis: Auto (boxes use their width: 60, 100, 40)",
        row_container(),
        &boxes,
    );

    // Px(100.0) — all boxes start at 100px
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], 0.0, 1.0, Val::Px(100.0), Val::Auto))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "flex_basis: Px(100) (all boxes start at 100px)",
        row_container(),
        &boxes,
    );

    // Percent(30.0) — boxes start at 30% of container
    let boxes: Vec<Entity> = (0..3)
        .map(|i| sized_box(&mut commands, BOX_COLORS[i], 0.0, 1.0, Val::Percent(30.0), Val::Auto))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "flex_basis: Percent(30) (boxes start at 30% of container)",
        row_container(),
        &boxes,
    );
}

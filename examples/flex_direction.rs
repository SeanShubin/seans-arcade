//! Demonstrates FlexDirection, JustifyContent, and AlignItems.
//!
//! Run with: `cargo run --example flex_direction`

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FlexDirectionPlugin))
        .run();
}

struct FlexDirectionPlugin;

impl Plugin for FlexDirectionPlugin {
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

fn spawn_box(commands: &mut Commands, color: Color, width: f32, height: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                ..default()
            },
            BackgroundColor(color),
        ))
        .id()
}

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

fn boxes_3(commands: &mut Commands) -> Vec<Entity> {
    (0..3)
        .map(|i| spawn_box(commands, BOX_COLORS[i], BOX_SIZE, BOX_SIZE))
        .collect()
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

    // ---- Part A: FlexDirection ----
    spawn_part_header(&mut commands, root, "FlexDirection");

    let boxes = boxes_3(&mut commands);
    spawn_section(
        &mut commands,
        root,
        "FlexDirection::Row",
        Node {
            flex_direction: FlexDirection::Row,
            width: Val::Percent(100.0),
            height: Val::Px(CONTAINER_HEIGHT),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = boxes_3(&mut commands);
    spawn_section(
        &mut commands,
        root,
        "FlexDirection::Column",
        Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            height: Val::Px(150.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = boxes_3(&mut commands);
    spawn_section(
        &mut commands,
        root,
        "FlexDirection::RowReverse",
        Node {
            flex_direction: FlexDirection::RowReverse,
            width: Val::Percent(100.0),
            height: Val::Px(CONTAINER_HEIGHT),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    let boxes = boxes_3(&mut commands);
    spawn_section(
        &mut commands,
        root,
        "FlexDirection::ColumnReverse",
        Node {
            flex_direction: FlexDirection::ColumnReverse,
            width: Val::Percent(100.0),
            height: Val::Px(150.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // ---- Part B: JustifyContent ----
    spawn_part_header(&mut commands, root, "JustifyContent");

    for (label, justify) in [
        ("JustifyContent::Start", JustifyContent::Start),
        ("JustifyContent::End", JustifyContent::End),
        ("JustifyContent::Center", JustifyContent::Center),
        ("JustifyContent::SpaceBetween", JustifyContent::SpaceBetween),
        ("JustifyContent::SpaceEvenly", JustifyContent::SpaceEvenly),
        ("JustifyContent::SpaceAround", JustifyContent::SpaceAround),
    ] {
        let boxes = boxes_3(&mut commands);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: justify,
                width: Val::Percent(100.0),
                height: Val::Px(CONTAINER_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &boxes,
        );
    }

    // ---- Part C: AlignItems ----
    spawn_part_header(&mut commands, root, "AlignItems");

    for (label, align) in [
        ("AlignItems::Start", AlignItems::Start),
        ("AlignItems::End", AlignItems::End),
        ("AlignItems::Center", AlignItems::Center),
        ("AlignItems::Baseline", AlignItems::Baseline),
        ("AlignItems::Stretch", AlignItems::Stretch),
    ] {
        let b1 = spawn_box(&mut commands, BOX_COLORS[0], BOX_SIZE, 20.0);
        let b2 = spawn_box(&mut commands, BOX_COLORS[1], BOX_SIZE, 40.0);
        let b3 = spawn_box(&mut commands, BOX_COLORS[2], BOX_SIZE, 30.0);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: align,
                width: Val::Percent(100.0),
                height: Val::Px(CONTAINER_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &[b1, b2, b3],
        );
    }
}

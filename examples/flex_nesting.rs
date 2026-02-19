//! Demonstrates nested flex containers, row_gap/column_gap, and margin/padding.
//!
//! Run with: `cargo run --example flex_nesting`

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FlexNestingPlugin))
        .run();
}

struct FlexNestingPlugin;

impl Plugin for FlexNestingPlugin {
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

fn boxes_4(commands: &mut Commands) -> Vec<Entity> {
    (0..4)
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

    // ---- Part A: Gaps ----
    spawn_part_header(&mut commands, root, "Gaps");

    // column_gap: 0
    let boxes = boxes_4(&mut commands);
    spawn_section(
        &mut commands,
        root,
        "column_gap: 0",
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Px(CONTAINER_HEIGHT),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // column_gap: 10px
    let boxes = boxes_4(&mut commands);
    spawn_section(
        &mut commands,
        root,
        "column_gap: 10px",
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            width: Val::Percent(100.0),
            height: Val::Px(CONTAINER_HEIGHT),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // column_gap: 30px
    let boxes = boxes_4(&mut commands);
    spawn_section(
        &mut commands,
        root,
        "column_gap: 30px",
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(30.0),
            width: Val::Percent(100.0),
            height: Val::Px(CONTAINER_HEIGHT),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // row_gap with Wrap
    let boxes: Vec<Entity> = (0..8)
        .map(|i| spawn_box(&mut commands, BOX_COLORS[i % 4], BOX_SIZE, BOX_SIZE))
        .collect();
    spawn_section(
        &mut commands,
        root,
        "row_gap: 10px + column_gap: 10px (with Wrap)",
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(10.0),
            column_gap: Val::Px(10.0),
            width: Val::Px(300.0),
            height: Val::Px(120.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // ---- Part B: Margin & Padding ----
    spawn_part_header(&mut commands, root, "Margin & Padding");

    // Boxes with margin: 10px
    let boxes: Vec<Entity> = (0..4)
        .map(|i| {
            commands
                .spawn((
                    Node {
                        width: Val::Px(BOX_SIZE),
                        height: Val::Px(BOX_SIZE),
                        margin: UiRect::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(BOX_COLORS[i]),
                ))
                .id()
        })
        .collect();
    spawn_section(
        &mut commands,
        root,
        "Box margin: 10px (space outside each box)",
        Node {
            flex_direction: FlexDirection::Row,
            width: Val::Percent(100.0),
            height: Val::Px(80.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // Container with padding: 20px
    let boxes = boxes_4(&mut commands);
    spawn_section(
        &mut commands,
        root,
        "Container padding: 20px (space inside container)",
        Node {
            flex_direction: FlexDirection::Row,
            padding: UiRect::all(Val::Px(20.0)),
            width: Val::Percent(100.0),
            height: Val::Px(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // Combined: container padding + box margin
    let boxes: Vec<Entity> = (0..4)
        .map(|i| {
            commands
                .spawn((
                    Node {
                        width: Val::Px(BOX_SIZE),
                        height: Val::Px(BOX_SIZE),
                        margin: UiRect::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(BOX_COLORS[i]),
                ))
                .id()
        })
        .collect();
    spawn_section(
        &mut commands,
        root,
        "Container padding: 16px + Box margin: 8px (combined)",
        Node {
            flex_direction: FlexDirection::Row,
            padding: UiRect::all(Val::Px(16.0)),
            width: Val::Percent(100.0),
            height: Val::Px(110.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // ---- Part C: Nesting ----
    spawn_part_header(&mut commands, root, "Nesting");

    // Horizontal row of 3 vertical columns, each with 3 stacked boxes
    {
        let section = commands
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                flex_shrink: 0.0,
                ..default()
            })
            .id();

        let label = commands
            .spawn((
                Text::new("Row of 3 Columns (each column has 3 stacked boxes)"),
                TextFont::from_font_size(FONT_SIZE),
                TextColor::WHITE,
            ))
            .id();

        let outer = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(16.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(160.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(CONTAINER_BG),
                BorderColor::all(BORDER_COLOR),
            ))
            .id();

        for col_idx in 0..3 {
            let column = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        flex_grow: 1.0,
                        border: UiRect::all(Val::Px(1.0)),
                        padding: UiRect::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.03)),
                    BorderColor::all(Color::srgb(0.3, 0.3, 0.3)),
                ))
                .id();

            for row_idx in 0..3 {
                let color_idx = (col_idx * 3 + row_idx) % 4;
                let b = commands
                    .spawn((
                        Node {
                            height: Val::Px(BOX_SIZE),
                            ..default()
                        },
                        BackgroundColor(BOX_COLORS[color_idx]),
                    ))
                    .id();
                commands.entity(column).add_child(b);
            }

            commands.entity(outer).add_child(column);
        }

        commands.entity(section).add_child(label);
        commands.entity(section).add_child(outer);
        commands.entity(root).add_child(section);
    }

    // Holy grail layout
    {
        let section = commands
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                flex_shrink: 0.0,
                ..default()
            })
            .id();

        let label = commands
            .spawn((
                Text::new("Holy Grail: header, sidebar + content + sidebar, footer"),
                TextFont::from_font_size(FONT_SIZE),
                TextColor::WHITE,
            ))
            .id();

        let outer = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Percent(100.0),
                    height: Val::Px(250.0),
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(CONTAINER_BG),
                BorderColor::all(BORDER_COLOR),
            ))
            .id();

        // Header
        let header = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(40.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(CORAL),
            ))
            .id();
        let header_text = commands
            .spawn((
                Text::new("Header"),
                TextFont::from_font_size(14.0),
                TextColor(Color::BLACK),
            ))
            .id();
        commands.entity(header).add_child(header_text);

        // Middle row: sidebar + content + sidebar
        let middle = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .id();

        let left_sidebar = commands
            .spawn((
                Node {
                    width: Val::Px(80.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(SKY_BLUE),
            ))
            .id();
        let left_text = commands
            .spawn((
                Text::new("Left"),
                TextFont::from_font_size(14.0),
                TextColor(Color::BLACK),
            ))
            .id();
        commands.entity(left_sidebar).add_child(left_text);

        let content = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(LIME),
            ))
            .id();
        let content_text = commands
            .spawn((
                Text::new("Content"),
                TextFont::from_font_size(14.0),
                TextColor(Color::BLACK),
            ))
            .id();
        commands.entity(content).add_child(content_text);

        let right_sidebar = commands
            .spawn((
                Node {
                    width: Val::Px(80.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(GOLD),
            ))
            .id();
        let right_text = commands
            .spawn((
                Text::new("Right"),
                TextFont::from_font_size(14.0),
                TextColor(Color::BLACK),
            ))
            .id();
        commands.entity(right_sidebar).add_child(right_text);

        commands.entity(middle).add_child(left_sidebar);
        commands.entity(middle).add_child(content);
        commands.entity(middle).add_child(right_sidebar);

        // Footer
        let footer = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(40.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(CORAL),
            ))
            .id();
        let footer_text = commands
            .spawn((
                Text::new("Footer"),
                TextFont::from_font_size(14.0),
                TextColor(Color::BLACK),
            ))
            .id();
        commands.entity(footer).add_child(footer_text);

        commands.entity(outer).add_child(header);
        commands.entity(outer).add_child(middle);
        commands.entity(outer).add_child(footer);

        commands.entity(section).add_child(label);
        commands.entity(section).add_child(outer);
        commands.entity(root).add_child(section);
    }
}

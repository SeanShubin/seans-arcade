//! Demonstrates FlexWrap, AlignContent, and AlignSelf.
//!
//! Run with: `cargo run --example flex_wrap`

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FlexWrapPlugin))
        .run();
}

struct FlexWrapPlugin;

impl Plugin for FlexWrapPlugin {
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

fn boxes_8(commands: &mut Commands) -> Vec<Entity> {
    (0..8)
        .map(|i| spawn_box(commands, BOX_COLORS[i % 4], BOX_SIZE, BOX_SIZE))
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

    // ---- Part A: FlexWrap ----
    spawn_part_header(&mut commands, root, "FlexWrap");

    for (label, wrap) in [
        ("FlexWrap::NoWrap", FlexWrap::NoWrap),
        ("FlexWrap::Wrap", FlexWrap::Wrap),
        ("FlexWrap::WrapReverse", FlexWrap::WrapReverse),
    ] {
        let boxes = boxes_8(&mut commands);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: wrap,
                width: Val::Px(300.0),
                height: Val::Px(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &boxes,
        );
    }

    // ---- Part B: AlignContent ----
    spawn_part_header(&mut commands, root, "AlignContent (with FlexWrap::Wrap)");

    for (label, align) in [
        ("AlignContent::Start", AlignContent::Start),
        ("AlignContent::End", AlignContent::End),
        ("AlignContent::Center", AlignContent::Center),
        ("AlignContent::SpaceBetween", AlignContent::SpaceBetween),
        ("AlignContent::SpaceEvenly", AlignContent::SpaceEvenly),
        ("AlignContent::Stretch", AlignContent::Stretch),
    ] {
        let boxes = boxes_8(&mut commands);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_content: align,
                width: Val::Px(300.0),
                height: Val::Px(200.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &boxes,
        );
    }

    // ---- Part C: AlignSelf ----
    spawn_part_header(&mut commands, root, "AlignSelf");

    let self_values: [(AlignSelf, &str); 5] = [
        (AlignSelf::Start, "Start"),
        (AlignSelf::End, "End"),
        (AlignSelf::Center, "Center"),
        (AlignSelf::Baseline, "Baseline"),
        (AlignSelf::Stretch, "Stretch"),
    ];

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
            Text::new("Each box has a different AlignSelf (container: AlignItems::Start)"),
            TextFont::from_font_size(FONT_SIZE),
            TextColor::WHITE,
        ))
        .id();

    let container = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Start,
                column_gap: Val::Px(8.0),
                width: Val::Percent(100.0),
                height: Val::Px(120.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(CONTAINER_BG),
            BorderColor::all(BORDER_COLOR),
        ))
        .id();

    for (i, (align_self, name)) in self_values.iter().enumerate() {
        let box_wrapper = commands
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                align_self: *align_self,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .id();

        let colored_box = commands
            .spawn((
                Node {
                    width: Val::Px(BOX_SIZE),
                    height: Val::Px(BOX_SIZE),
                    ..default()
                },
                BackgroundColor(BOX_COLORS[i % 4]),
            ))
            .id();

        let box_label = commands
            .spawn((
                Text::new(*name),
                TextFont::from_font_size(12.0),
                TextColor::WHITE,
            ))
            .id();

        commands.entity(box_wrapper).add_child(colored_box);
        commands.entity(box_wrapper).add_child(box_label);
        commands.entity(container).add_child(box_wrapper);
    }

    commands.entity(section).add_child(label_entity);
    commands.entity(section).add_child(container);
    commands.entity(root).add_child(section);
}

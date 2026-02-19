//! Demonstrates grid alignment properties: JustifyItems, AlignItems,
//! JustifyContent, AlignContent, JustifySelf, and AlignSelf.
//!
//! Run with: `cargo run --example grid_alignment`

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, GridAlignmentPlugin))
        .run();
}

struct GridAlignmentPlugin;

impl Plugin for GridAlignmentPlugin {
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

fn spawn_box_with_node(
    commands: &mut Commands,
    color: Color,
    num: usize,
    node: Node,
) -> Entity {
    let parent = commands.spawn((node, BackgroundColor(color))).id();
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

    // ---- Part A: JustifyItems ----
    spawn_part_header(&mut commands, root, "JustifyItems");

    for (label, justify) in [
        ("JustifyItems::Start", JustifyItems::Start),
        ("JustifyItems::End", JustifyItems::End),
        ("JustifyItems::Center", JustifyItems::Center),
        ("JustifyItems::Stretch", JustifyItems::Stretch),
        ("JustifyItems::Default", JustifyItems::Default),
    ] {
        let boxes = spawn_boxes(&mut commands, 4);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
                grid_template_rows: vec![RepeatedGridTrack::px(2, 60.0)],
                justify_items: justify,
                width: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &boxes,
        );
    }

    // ---- Part B: AlignItems ----
    spawn_part_header(&mut commands, root, "AlignItems");

    for (label, align) in [
        ("AlignItems::Start", AlignItems::Start),
        ("AlignItems::End", AlignItems::End),
        ("AlignItems::Center", AlignItems::Center),
        ("AlignItems::Stretch", AlignItems::Stretch),
        ("AlignItems::Default", AlignItems::Default),
    ] {
        let boxes = spawn_boxes(&mut commands, 4);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
                grid_template_rows: vec![RepeatedGridTrack::px(2, 80.0)],
                align_items: align,
                width: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &boxes,
        );
    }

    // ---- Part C: JustifyContent ----
    spawn_part_header(&mut commands, root, "JustifyContent");

    for (label, justify) in [
        ("JustifyContent::Start", JustifyContent::Start),
        ("JustifyContent::End", JustifyContent::End),
        ("JustifyContent::Center", JustifyContent::Center),
        ("JustifyContent::SpaceBetween", JustifyContent::SpaceBetween),
        ("JustifyContent::SpaceEvenly", JustifyContent::SpaceEvenly),
        ("JustifyContent::SpaceAround", JustifyContent::SpaceAround),
    ] {
        let boxes = spawn_boxes(&mut commands, 3);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                display: Display::Grid,
                grid_template_columns: vec![
                    GridTrack::px(60.0),
                    GridTrack::px(60.0),
                    GridTrack::px(60.0),
                ],
                justify_content: justify,
                width: Val::Percent(100.0),
                height: Val::Px(60.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &boxes,
        );
    }

    // ---- Part D: AlignContent ----
    spawn_part_header(&mut commands, root, "AlignContent");

    for (label, align) in [
        ("AlignContent::Start", AlignContent::Start),
        ("AlignContent::End", AlignContent::End),
        ("AlignContent::Center", AlignContent::Center),
        ("AlignContent::SpaceBetween", AlignContent::SpaceBetween),
        ("AlignContent::SpaceEvenly", AlignContent::SpaceEvenly),
        ("AlignContent::Stretch", AlignContent::Stretch),
    ] {
        let boxes = spawn_boxes(&mut commands, 6);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
                grid_template_rows: vec![GridTrack::px(40.0), GridTrack::px(40.0)],
                align_content: align,
                width: Val::Percent(100.0),
                height: Val::Px(200.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &boxes,
        );
    }

    // ---- Part E: JustifySelf / AlignSelf ----
    spawn_part_header(&mut commands, root, "JustifySelf / AlignSelf");

    {
        let b1 = spawn_box_with_node(&mut commands, BOX_COLORS[0], 1, Node {
            width: Val::Px(BOX_SIZE),
            height: Val::Px(BOX_SIZE),
            justify_self: JustifySelf::Start,
            align_self: AlignSelf::Start,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
        let b2 = spawn_box_with_node(&mut commands, BOX_COLORS[1], 2, Node {
            width: Val::Px(BOX_SIZE),
            height: Val::Px(BOX_SIZE),
            justify_self: JustifySelf::End,
            align_self: AlignSelf::Start,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
        let b3 = spawn_box_with_node(&mut commands, BOX_COLORS[2], 3, Node {
            width: Val::Px(BOX_SIZE),
            height: Val::Px(BOX_SIZE),
            justify_self: JustifySelf::Start,
            align_self: AlignSelf::End,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
        let b4 = spawn_box_with_node(&mut commands, BOX_COLORS[3], 4, Node {
            width: Val::Px(BOX_SIZE),
            height: Val::Px(BOX_SIZE),
            justify_self: JustifySelf::End,
            align_self: AlignSelf::End,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
        spawn_section(
            &mut commands,
            root,
            "Per-item self-alignment: 1=Start/Start, 2=End/Start, 3=Start/End, 4=End/End",
            Node {
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
                grid_template_rows: vec![RepeatedGridTrack::px(2, 80.0)],
                width: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &[b1, b2, b3, b4],
        );
    }
}

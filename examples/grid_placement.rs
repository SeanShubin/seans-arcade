//! Demonstrates grid item placement with GridPlacement and GridAutoFlow.
//!
//! Run with: `cargo run --example grid_placement`

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, GridPlacementPlugin))
        .run();
}

struct GridPlacementPlugin;

impl Plugin for GridPlacementPlugin {
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

fn grid_4x3() -> Node {
    Node {
        display: Display::Grid,
        grid_template_columns: vec![RepeatedGridTrack::flex(4, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::px(3, 50.0)],
        width: Val::Percent(100.0),
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    }
}

fn grid_3x3() -> Node {
    Node {
        display: Display::Grid,
        grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
        grid_template_rows: vec![RepeatedGridTrack::px(3, 50.0)],
        width: Val::Percent(100.0),
        border: UiRect::all(Val::Px(1.0)),
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

    // ---- Part A: GridPlacement basics ----
    spawn_part_header(&mut commands, root, "GridPlacement Basics");

    // Default auto-placement
    let boxes = spawn_boxes(&mut commands, 6);
    spawn_section(
        &mut commands,
        root,
        "Default auto-placement — 6 boxes fill in row order",
        grid_4x3(),
        &boxes,
    );

    // GridPlacement::start(n) — boxes at specific column starts
    {
        let b1 = spawn_box_with_node(&mut commands, BOX_COLORS[0], 1, Node {
            grid_column: GridPlacement::start(1),
            ..box_node()
        });
        let b2 = spawn_box_with_node(&mut commands, BOX_COLORS[1], 2, Node {
            grid_column: GridPlacement::start(3),
            ..box_node()
        });
        let b3 = spawn_box_with_node(&mut commands, BOX_COLORS[2], 3, Node {
            grid_column: GridPlacement::start(2),
            grid_row: GridPlacement::start(2),
            ..box_node()
        });
        let b4 = spawn_box_with_node(&mut commands, BOX_COLORS[3], 4, Node {
            grid_column: GridPlacement::start(4),
            grid_row: GridPlacement::start(2),
            ..box_node()
        });
        spawn_section(
            &mut commands,
            root,
            "GridPlacement::start(n) — boxes at specific column starts",
            grid_4x3(),
            &[b1, b2, b3, b4],
        );
    }

    // GridPlacement::span(2) — a box spanning 2 columns
    {
        let b1 = spawn_box_with_node(&mut commands, BOX_COLORS[0], 1, Node {
            grid_column: GridPlacement::span(2),
            height: Val::Px(BOX_SIZE),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
        let b2 = spawn_box(&mut commands, BOX_COLORS[1], 2);
        let b3 = spawn_box(&mut commands, BOX_COLORS[2], 3);
        let b4 = spawn_box(&mut commands, BOX_COLORS[3], 4);
        spawn_section(
            &mut commands,
            root,
            "GridPlacement::span(2) — first box spans 2 columns",
            grid_4x3(),
            &[b1, b2, b3, b4],
        );
    }

    // GridPlacement::start_end(1, 3)
    {
        let b1 = spawn_box_with_node(&mut commands, BOX_COLORS[0], 1, Node {
            grid_column: GridPlacement::start_end(1, 3),
            height: Val::Px(BOX_SIZE),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
        let b2 = spawn_box(&mut commands, BOX_COLORS[1], 2);
        let b3 = spawn_box(&mut commands, BOX_COLORS[2], 3);
        let b4 = spawn_box(&mut commands, BOX_COLORS[3], 4);
        spawn_section(
            &mut commands,
            root,
            "GridPlacement::start_end(1, 3) — box from column line 1 to 3",
            grid_4x3(),
            &[b1, b2, b3, b4],
        );
    }

    // ---- Part B: Row placement ----
    spawn_part_header(&mut commands, root, "Row Placement");

    // grid_row: GridPlacement::start(2) — item in second row
    {
        let b1 = spawn_box_with_node(&mut commands, BOX_COLORS[0], 1, Node {
            grid_row: GridPlacement::start(2),
            ..box_node()
        });
        let b2 = spawn_box(&mut commands, BOX_COLORS[1], 2);
        let b3 = spawn_box(&mut commands, BOX_COLORS[2], 3);
        let b4 = spawn_box(&mut commands, BOX_COLORS[3], 4);
        spawn_section(
            &mut commands,
            root,
            "grid_row: start(2) — box 1 placed in second row",
            grid_3x3(),
            &[b1, b2, b3, b4],
        );
    }

    // grid_row: GridPlacement::span(2) — item spanning 2 rows
    {
        let b1 = spawn_box_with_node(&mut commands, BOX_COLORS[0], 1, Node {
            grid_row: GridPlacement::span(2),
            width: Val::Px(BOX_SIZE),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        });
        let b2 = spawn_box(&mut commands, BOX_COLORS[1], 2);
        let b3 = spawn_box(&mut commands, BOX_COLORS[2], 3);
        let b4 = spawn_box(&mut commands, BOX_COLORS[3], 4);
        let b5 = spawn_box(&mut commands, BOX_COLORS[0], 5);
        spawn_section(
            &mut commands,
            root,
            "grid_row: span(2) — box 1 spans 2 rows",
            grid_3x3(),
            &[b1, b2, b3, b4, b5],
        );
    }

    // Combined column + row placement
    {
        let b1 = spawn_box_with_node(&mut commands, BOX_COLORS[0], 1, Node {
            grid_column: GridPlacement::start(1),
            grid_row: GridPlacement::start(1),
            ..box_node()
        });
        let b2 = spawn_box_with_node(&mut commands, BOX_COLORS[1], 2, Node {
            grid_column: GridPlacement::start(3),
            grid_row: GridPlacement::start(1),
            ..box_node()
        });
        let b3 = spawn_box_with_node(&mut commands, BOX_COLORS[2], 3, Node {
            grid_column: GridPlacement::start(2),
            grid_row: GridPlacement::start(2),
            ..box_node()
        });
        let b4 = spawn_box_with_node(&mut commands, BOX_COLORS[3], 4, Node {
            grid_column: GridPlacement::start(3),
            grid_row: GridPlacement::start(3),
            ..box_node()
        });
        spawn_section(
            &mut commands,
            root,
            "Combined column + row — items at specific grid cells",
            grid_3x3(),
            &[b1, b2, b3, b4],
        );
    }

    // ---- Part C: GridAutoFlow ----
    spawn_part_header(&mut commands, root, "GridAutoFlow");

    for (label, flow) in [
        ("GridAutoFlow::Row — auto items fill rows first", GridAutoFlow::Row),
        ("GridAutoFlow::Column — auto items fill columns first", GridAutoFlow::Column),
        ("GridAutoFlow::RowDense — dense packing fills row gaps", GridAutoFlow::RowDense),
        ("GridAutoFlow::ColumnDense — dense packing fills column gaps", GridAutoFlow::ColumnDense),
    ] {
        // Create a 3x3 grid with one explicitly placed item leaving a gap
        let b1 = spawn_box_with_node(&mut commands, BOX_COLORS[0], 1, Node {
            grid_column: GridPlacement::start(2),
            grid_row: GridPlacement::start(1),
            ..box_node()
        });
        let b2 = spawn_box(&mut commands, BOX_COLORS[1], 2);
        let b3 = spawn_box(&mut commands, BOX_COLORS[2], 3);
        let b4 = spawn_box(&mut commands, BOX_COLORS[3], 4);
        let b5 = spawn_box(&mut commands, BOX_COLORS[0], 5);
        let b6 = spawn_box(&mut commands, BOX_COLORS[1], 6);
        spawn_section(
            &mut commands,
            root,
            label,
            Node {
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
                grid_template_rows: vec![RepeatedGridTrack::px(3, 50.0)],
                grid_auto_flow: flow,
                width: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &[b1, b2, b3, b4, b5, b6],
        );
    }

    // ---- Part D: Implicit tracks ----
    spawn_part_header(&mut commands, root, "Implicit Tracks");

    // grid_auto_rows — overflow items get 50px rows
    let boxes = spawn_boxes(&mut commands, 8);
    spawn_section(
        &mut commands,
        root,
        "grid_auto_rows: [px(50)] — overflow items get 50px rows (2 col grid, 8 items)",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
            grid_auto_rows: vec![GridTrack::px(50.0)],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // grid_auto_columns — overflow items get equal columns
    let boxes = spawn_boxes(&mut commands, 6);
    spawn_section(
        &mut commands,
        root,
        "grid_auto_columns: [flex(1)] — overflow items get equal columns (2 rows defined)",
        Node {
            display: Display::Grid,
            grid_template_rows: vec![RepeatedGridTrack::px(2, 50.0)],
            grid_auto_flow: GridAutoFlow::Column,
            grid_auto_columns: vec![GridTrack::flex(1.0)],
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );
}

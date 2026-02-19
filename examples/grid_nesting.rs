//! Demonstrates grid gaps, spanning items, nested grids, and real-world layouts.
//!
//! Run with: `cargo run --example grid_nesting`

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, GridNestingPlugin))
        .run();
}

struct GridNestingPlugin;

impl Plugin for GridNestingPlugin {
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

    // ---- Part A: Gaps ----
    spawn_part_header(&mut commands, root, "Gaps");

    let boxes = spawn_boxes(&mut commands, 6);
    spawn_section(
        &mut commands,
        root,
        "row_gap: 0, column_gap: 0 — no gaps",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
            grid_template_rows: vec![RepeatedGridTrack::px(2, 50.0)],
            row_gap: Val::Px(0.0),
            column_gap: Val::Px(0.0),
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
        "row_gap: 10px, column_gap: 10px — uniform gaps",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
            grid_template_rows: vec![RepeatedGridTrack::px(2, 50.0)],
            row_gap: Val::Px(10.0),
            column_gap: Val::Px(10.0),
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
        "row_gap: 4px, column_gap: 20px — asymmetric gaps",
        Node {
            display: Display::Grid,
            grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
            grid_template_rows: vec![RepeatedGridTrack::px(2, 50.0)],
            row_gap: Val::Px(4.0),
            column_gap: Val::Px(20.0),
            width: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        &boxes,
    );

    // ---- Part B: Spanning items ----
    spawn_part_header(&mut commands, root, "Spanning Items");

    // Header spanning full width
    {
        let header = spawn_labeled_region(
            &mut commands,
            CORAL,
            "Header (span 3)",
            Node {
                grid_column: GridPlacement::span(3),
                height: Val::Px(40.0),
                ..default()
            },
        );
        let b1 = spawn_box(&mut commands, SKY_BLUE, 1);
        let b2 = spawn_box(&mut commands, LIME, 2);
        let b3 = spawn_box(&mut commands, GOLD, 3);
        spawn_section(
            &mut commands,
            root,
            "Header spanning full width: top item spans all 3 columns",
            Node {
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(3, 1.0)],
                grid_template_rows: vec![GridTrack::px(40.0), GridTrack::px(50.0)],
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                width: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &[header, b1, b2, b3],
        );
    }

    // Sidebar layout
    {
        let sidebar = spawn_labeled_region(
            &mut commands,
            CORAL,
            "Sidebar",
            Node {
                grid_row: GridPlacement::span(3),
                ..default()
            },
        );
        let c1 = spawn_labeled_region(&mut commands, SKY_BLUE, "A", Node::default());
        let c2 = spawn_labeled_region(&mut commands, LIME, "B", Node::default());
        let c3 = spawn_labeled_region(&mut commands, GOLD, "C", Node::default());
        let c4 = spawn_labeled_region(&mut commands, SKY_BLUE, "D", Node::default());
        let c5 = spawn_labeled_region(&mut commands, LIME, "E", Node::default());
        spawn_section(
            &mut commands,
            root,
            "Sidebar layout: left item spans 3 rows, content fills 2x3",
            Node {
                display: Display::Grid,
                grid_template_columns: vec![GridTrack::px(80.0), GridTrack::flex(1.0), GridTrack::flex(1.0)],
                grid_template_rows: vec![RepeatedGridTrack::px(3, 50.0)],
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                width: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            &[sidebar, c1, c2, c3, c4, c5],
        );
    }

    // ---- Part C: Nested grids ----
    spawn_part_header(&mut commands, root, "Nested Grids");

    // Outer 2x2 grid where each cell is a 2x2 grid
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
                Text::new("Outer 2x2 grid, each cell is a nested 2x2 grid"),
                TextFont::from_font_size(FONT_SIZE),
                TextColor::WHITE,
            ))
            .id();

        let outer = commands
            .spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
                    grid_template_rows: vec![RepeatedGridTrack::px(2, 110.0)],
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(8.0),
                    width: Val::Percent(100.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(CONTAINER_BG),
                BorderColor::all(BORDER_COLOR),
            ))
            .id();

        for cell_idx in 0..4 {
            let inner = commands
                .spawn((
                    Node {
                        display: Display::Grid,
                        grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
                        grid_template_rows: vec![RepeatedGridTrack::flex(2, 1.0)],
                        column_gap: Val::Px(2.0),
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.03)),
                    BorderColor::all(Color::srgb(0.3, 0.3, 0.3)),
                ))
                .id();

            for box_idx in 0..4 {
                let num = cell_idx * 4 + box_idx + 1;
                let b = spawn_box(&mut commands, BOX_COLORS[(cell_idx + box_idx) % 4], num);
                commands.entity(inner).add_child(b);
            }

            commands.entity(outer).add_child(inner);
        }

        commands.entity(section).add_child(label);
        commands.entity(section).add_child(outer);
        commands.entity(root).add_child(section);
    }

    // Grid + flex mix
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
                Text::new("Outer grid: left cell = flex row, right cell = flex column"),
                TextFont::from_font_size(FONT_SIZE),
                TextColor::WHITE,
            ))
            .id();

        let outer = commands
            .spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
                    grid_template_rows: vec![GridTrack::px(120.0)],
                    column_gap: Val::Px(8.0),
                    width: Val::Percent(100.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(CONTAINER_BG),
                BorderColor::all(BORDER_COLOR),
            ))
            .id();

        // Left cell: flex row
        let flex_row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.03)),
            ))
            .id();
        for i in 0..3 {
            let b = spawn_box(&mut commands, BOX_COLORS[i], i + 1);
            commands.entity(flex_row).add_child(b);
        }

        // Right cell: flex column
        let flex_col = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    align_items: AlignItems::Start,
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.03)),
            ))
            .id();
        for i in 0..3 {
            let b = spawn_box(&mut commands, BOX_COLORS[i + 1], i + 4);
            commands.entity(flex_col).add_child(b);
        }

        commands.entity(outer).add_child(flex_row);
        commands.entity(outer).add_child(flex_col);

        commands.entity(section).add_child(label);
        commands.entity(section).add_child(outer);
        commands.entity(root).add_child(section);
    }

    // ---- Part D: Real-world layouts ----
    spawn_part_header(&mut commands, root, "Real-World Layouts");

    // Dashboard layout
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
                Text::new("Dashboard: header, sidebar (2 rows), 4 content cards (2x2)"),
                TextFont::from_font_size(FONT_SIZE),
                TextColor::WHITE,
            ))
            .id();

        let grid = commands
            .spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: vec![
                        GridTrack::px(100.0),
                        GridTrack::flex(1.0),
                        GridTrack::flex(1.0),
                    ],
                    grid_template_rows: vec![
                        GridTrack::px(40.0),
                        GridTrack::px(80.0),
                        GridTrack::px(80.0),
                    ],
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    width: Val::Percent(100.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(CONTAINER_BG),
                BorderColor::all(BORDER_COLOR),
            ))
            .id();

        // Header (spans all 3 columns)
        let header = spawn_labeled_region(
            &mut commands,
            CORAL,
            "Header",
            Node {
                grid_column: GridPlacement::span(3),
                ..default()
            },
        );

        // Sidebar (spans 2 rows)
        let sidebar = spawn_labeled_region(
            &mut commands,
            SKY_BLUE,
            "Sidebar",
            Node {
                grid_row: GridPlacement::span(2),
                ..default()
            },
        );

        // 4 content cards
        let card1 = spawn_labeled_region(&mut commands, LIME, "Card 1", Node::default());
        let card2 = spawn_labeled_region(&mut commands, GOLD, "Card 2", Node::default());
        let card3 = spawn_labeled_region(&mut commands, LIME, "Card 3", Node::default());
        let card4 = spawn_labeled_region(&mut commands, GOLD, "Card 4", Node::default());

        commands.entity(grid).add_child(header);
        commands.entity(grid).add_child(sidebar);
        commands.entity(grid).add_child(card1);
        commands.entity(grid).add_child(card2);
        commands.entity(grid).add_child(card3);
        commands.entity(grid).add_child(card4);

        commands.entity(section).add_child(label);
        commands.entity(section).add_child(grid);
        commands.entity(root).add_child(section);
    }

    // Photo gallery
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
                Text::new("Photo gallery: items spanning 1x1, 2x1, 1x2, and 2x2"),
                TextFont::from_font_size(FONT_SIZE),
                TextColor::WHITE,
            ))
            .id();

        let grid = commands
            .spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: vec![RepeatedGridTrack::flex(4, 1.0)],
                    grid_auto_rows: vec![GridTrack::px(60.0)],
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    width: Val::Percent(100.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(CONTAINER_BG),
                BorderColor::all(BORDER_COLOR),
            ))
            .id();

        // Photo 1: spans 2 columns (wide)
        let p1 = spawn_labeled_region(
            &mut commands,
            CORAL,
            "Wide",
            Node {
                grid_column: GridPlacement::span(2),
                ..default()
            },
        );

        // Photo 2: 1x1
        let p2 = spawn_labeled_region(&mut commands, SKY_BLUE, "1x1", Node::default());

        // Photo 3: 1x1
        let p3 = spawn_labeled_region(&mut commands, LIME, "1x1", Node::default());

        // Photo 4: 1x1
        let p4 = spawn_labeled_region(&mut commands, GOLD, "1x1", Node::default());

        // Photo 5: spans 2 rows (tall)
        let p5 = spawn_labeled_region(
            &mut commands,
            CORAL,
            "Tall",
            Node {
                grid_row: GridPlacement::span(2),
                ..default()
            },
        );

        // Photo 6: spans 2 columns and 2 rows (large)
        let p6 = spawn_labeled_region(
            &mut commands,
            SKY_BLUE,
            "Large",
            Node {
                grid_column: GridPlacement::span(2),
                grid_row: GridPlacement::span(2),
                ..default()
            },
        );

        // Photo 7: 1x1
        let p7 = spawn_labeled_region(&mut commands, LIME, "1x1", Node::default());

        // Photo 8: 1x1
        let p8 = spawn_labeled_region(&mut commands, GOLD, "1x1", Node::default());

        commands.entity(grid).add_child(p1);
        commands.entity(grid).add_child(p2);
        commands.entity(grid).add_child(p3);
        commands.entity(grid).add_child(p4);
        commands.entity(grid).add_child(p5);
        commands.entity(grid).add_child(p6);
        commands.entity(grid).add_child(p7);
        commands.entity(grid).add_child(p8);

        commands.entity(section).add_child(label);
        commands.entity(section).add_child(grid);
        commands.entity(root).add_child(section);
    }
}

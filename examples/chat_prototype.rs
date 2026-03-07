//! Prototype chat client UI.
//!
//! Type messages, press Enter to send, scroll through chat history.
//! Mouse wheel scrolls. Scrollbar thumb is draggable. Click the track to jump.
//! This is a local-only prototype — no networking.
//!
//! Run: `cargo run --example chat_prototype`

use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::WindowResolution;

const SCROLLBAR_WIDTH: f32 = 14.0;
const THUMB_MIN_HEIGHT: f32 = 20.0;
const SCROLL_LINE_HEIGHT: f32 = 30.0;
const MESSAGE_FONT_SIZE: f32 = 18.0;
const THUMB_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);
const THUMB_HOVER_COLOR: Color = Color::srgb(0.7, 0.7, 0.7);
const THUMB_DRAG_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
const TRACK_COLOR: Color = Color::srgb(0.2, 0.2, 0.2);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Chat Prototype".into(),
                resolution: WindowResolution::new(600, 500),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<ChatState>()
        .init_resource::<ScrollbarDragState>()
        .add_systems(Startup, setup_ui)
        .add_systems(
            Update,
            (
                handle_keyboard_input,
                spawn_new_messages,
                handle_mouse_wheel,
                handle_scrollbar_interaction,
                update_scrollbar_thumb,
                update_input_display,
                auto_scroll_on_new_message,
            ),
        )
        .run();
}

#[derive(Resource, Default)]
struct ChatState {
    messages: Vec<String>,
    input_buffer: String,
    spawned_count: usize,
}

#[derive(Resource, Default)]
struct ScrollbarDragState {
    dragging: bool,
    drag_start_cursor_y: f32,
    drag_start_scroll: f32,
}

#[derive(Component)]
struct ChatHistoryContainer;

#[derive(Component)]
struct InputText;

#[derive(Component)]
struct ScrollbarTrack;

#[derive(Component)]
struct ScrollbarThumb;

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|root| {
            // Chat area row: history + scrollbar
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                min_height: Val::Px(0.0), // Required for scroll container to constrain
                ..default()
            })
            .with_children(|chat_row| {
                // Chat history — scrollable, each message is a separate child entity
                chat_row.spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        padding: UiRect::all(Val::Px(12.0)),
                        min_height: Val::Px(0.0), // Required for scroll container to constrain
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
                    ChatHistoryContainer,
                ));

                // Scrollbar track
                chat_row
                    .spawn((
                        Node {
                            width: Val::Px(SCROLLBAR_WIDTH),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(TRACK_COLOR),
                        ScrollbarTrack,
                        Interaction::default(),
                    ))
                    .with_child((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(THUMB_MIN_HEIGHT),
                            position_type: PositionType::Absolute,
                            top: Val::Px(0.0),
                            ..default()
                        },
                        BackgroundColor(THUMB_COLOR),
                        ScrollbarThumb,
                        Interaction::default(),
                    ));
            });

            // Input bar
            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(48.0),
                min_height: Val::Px(48.0),
                padding: UiRect::all(Val::Px(12.0)),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_child((
                Text::new("> _"),
                TextFont::from_font_size(20.0),
                TextColor(Color::srgb(0.3, 1.0, 0.3)),
                InputText,
            ));
        });
}

fn handle_keyboard_input(
    mut events: MessageReader<KeyboardInput>,
    mut chat: ResMut<ChatState>,
) {
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }

        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let trimmed = chat.input_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    chat.messages.push(trimmed);
                    chat.input_buffer.clear();
                }
            }
            KeyCode::Backspace => {
                chat.input_buffer.pop();
            }
            _ => {
                if let Some(ref text) = event.text {
                    chat.input_buffer.push_str(text);
                }
            }
        }
    }
}

fn spawn_new_messages(
    mut commands: Commands,
    mut chat: ResMut<ChatState>,
    container_query: Query<Entity, With<ChatHistoryContainer>>,
) {
    if chat.spawned_count >= chat.messages.len() {
        return;
    }

    let Ok(container) = container_query.single() else {
        return;
    };

    for i in chat.spawned_count..chat.messages.len() {
        let msg = &chat.messages[i];
        let child = commands
            .spawn((
                Text::new(format!("you: {msg}")),
                TextFont::from_font_size(MESSAGE_FONT_SIZE),
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ))
            .id();
        commands.entity(container).add_child(child);
    }
    chat.spawned_count = chat.messages.len();
}

fn update_input_display(
    chat: Res<ChatState>,
    mut query: Query<&mut Text, With<InputText>>,
) {
    if !chat.is_changed() {
        return;
    }

    let Ok(mut text) = query.single_mut() else {
        return;
    };

    if chat.input_buffer.is_empty() {
        **text = "> _".into();
    } else {
        **text = format!("> {}_", chat.input_buffer);
    }
}

fn max_scroll(content_h: f32, viewport_h: f32) -> f32 {
    (content_h - viewport_h).max(0.0)
}

fn thumb_height(track_h: f32, content_h: f32, viewport_h: f32) -> f32 {
    if content_h <= viewport_h {
        return track_h;
    }
    (viewport_h / content_h * track_h).max(THUMB_MIN_HEIGHT)
}

fn handle_mouse_wheel(
    mut wheel_events: MessageReader<MouseWheel>,
    mut container_query: Query<
        (&mut ScrollPosition, &ComputedNode),
        With<ChatHistoryContainer>,
    >,
) {
    let total_y: f32 = wheel_events.read().map(|e| e.y).sum();
    if total_y == 0.0 {
        return;
    }

    let Ok((mut scroll, computed)) = container_query.single_mut() else {
        return;
    };

    let viewport_h = computed.size().y;
    let content_h = computed.content_size().y;
    let max = max_scroll(content_h, viewport_h);
    scroll.y = (scroll.y - total_y * SCROLL_LINE_HEIGHT).clamp(0.0, max);
}

fn handle_scrollbar_interaction(
    mut drag: ResMut<ScrollbarDragState>,
    mut thumb_query: Query<(&Interaction, &mut BackgroundColor), With<ScrollbarThumb>>,
    track_query: Query<
        (&Interaction, &UiGlobalTransform, &ComputedNode),
        (With<ScrollbarTrack>, Without<ScrollbarThumb>),
    >,
    mut container_query: Query<
        (&mut ScrollPosition, &ComputedNode),
        With<ChatHistoryContainer>,
    >,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
) {
    let Ok((thumb_interaction, mut thumb_color)) = thumb_query.single_mut() else {
        return;
    };
    let Ok((track_interaction, track_transform, track_computed)) = track_query.single() else {
        return;
    };

    let track_h = track_computed.size().y;

    let cursor_pos = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position());
    let cursor_y = cursor_pos
        .and_then(|pos| track_computed.normalize_point(*track_transform, pos))
        .map(|n| (n.y + 0.5) * track_h);

    // End drag on mouse release
    if drag.dragging && !mouse_button.pressed(MouseButton::Left) {
        drag.dragging = false;
    }

    // Start drag on thumb press
    if *thumb_interaction == Interaction::Pressed && !drag.dragging {
        let Ok((scroll, _)) = container_query.single() else {
            return;
        };
        if let Some(cy) = cursor_y {
            drag.dragging = true;
            drag.drag_start_cursor_y = cy;
            drag.drag_start_scroll = scroll.y;
        }
    }

    // Track click (not on thumb) — jump to position
    if !drag.dragging
        && *track_interaction == Interaction::Pressed
        && *thumb_interaction == Interaction::None
    {
        if let Some(cy) = cursor_y {
            let Ok((mut scroll, computed)) = container_query.single_mut() else {
                return;
            };
            let viewport_h = computed.size().y;
            let content_h = computed.content_size().y;
            let max = max_scroll(content_h, viewport_h);
            let thumb_h = thumb_height(track_h, content_h, viewport_h);
            let usable_track = track_h - thumb_h;
            let ratio = if usable_track > 0.0 {
                (cy - thumb_h / 2.0) / usable_track
            } else {
                0.0
            };
            scroll.y = (ratio * max).clamp(0.0, max);
            thumb_color.0 = THUMB_COLOR;
            return;
        }
    }

    // Update thumb color
    if drag.dragging {
        thumb_color.0 = THUMB_DRAG_COLOR;
    } else {
        match *thumb_interaction {
            Interaction::Hovered => thumb_color.0 = THUMB_HOVER_COLOR,
            Interaction::Pressed => thumb_color.0 = THUMB_DRAG_COLOR,
            Interaction::None => thumb_color.0 = THUMB_COLOR,
        }
    }

    // Process drag movement
    if !drag.dragging {
        return;
    }

    let Some(cy) = cursor_y else {
        return;
    };

    let Ok((mut scroll, computed)) = container_query.single_mut() else {
        return;
    };

    let viewport_h = computed.size().y;
    let content_h = computed.content_size().y;
    let max = max_scroll(content_h, viewport_h);
    if max <= 0.0 {
        return;
    }

    let thumb_h = thumb_height(track_h, content_h, viewport_h);
    let usable_track = track_h - thumb_h;
    if usable_track <= 0.0 {
        return;
    }

    let delta_pixels = cy - drag.drag_start_cursor_y;
    let scroll_per_pixel = max / usable_track;
    scroll.y = (drag.drag_start_scroll + delta_pixels * scroll_per_pixel).clamp(0.0, max);
}

fn update_scrollbar_thumb(
    container_query: Query<
        (&ScrollPosition, &ComputedNode),
        With<ChatHistoryContainer>,
    >,
    track_query: Query<&ComputedNode, (With<ScrollbarTrack>, Without<ChatHistoryContainer>)>,
    mut thumb_query: Query<&mut Node, With<ScrollbarThumb>>,
) {
    let Ok((scroll, computed)) = container_query.single() else {
        return;
    };
    let Ok(track_computed) = track_query.single() else {
        return;
    };
    let Ok(mut thumb_node) = thumb_query.single_mut() else {
        return;
    };

    let viewport_h = computed.size().y;
    let content_h = computed.content_size().y;
    let track_h = track_computed.size().y;
    let thumb_h = thumb_height(track_h, content_h, viewport_h);
    let max = max_scroll(content_h, viewport_h);

    let ratio = if max > 0.0 { scroll.y / max } else { 0.0 };
    let thumb_top = ratio * (track_h - thumb_h);

    thumb_node.height = Val::Px(thumb_h);
    thumb_node.top = Val::Px(thumb_top);
}

fn auto_scroll_on_new_message(
    chat: Res<ChatState>,
    mut container_query: Query<
        (&mut ScrollPosition, &ComputedNode),
        With<ChatHistoryContainer>,
    >,
) {
    if !chat.is_changed() {
        return;
    }

    let Ok((mut scroll, computed)) = container_query.single_mut() else {
        return;
    };

    let viewport_h = computed.size().y;
    let content_h = computed.content_size().y;
    scroll.y = max_scroll(content_h, viewport_h);
}

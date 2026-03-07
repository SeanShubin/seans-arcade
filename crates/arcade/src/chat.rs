//! Chat UI plugin.
//!
//! Reuses the scroll/scrollbar system from `examples/chat_prototype.rs`.
//! Messages come from the network instead of being local-only.

use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use protocol::{ChatPayload, RelayMessage, deserialize};

use crate::fraktur::to_fraktur;

use crate::config::{generate_identity_secret, save_config};
use crate::net::{
    ClientConfig, ConnectionState, IncomingMessages, NetSocket, PeerList, send_chat_message,
    start_connection,
};

const SCROLLBAR_WIDTH: f32 = 14.0;
const THUMB_MIN_HEIGHT: f32 = 20.0;
const SCROLL_LINE_HEIGHT: f32 = 30.0;
const MESSAGE_FONT_SIZE: f32 = 18.0;
const THUMB_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);
const THUMB_HOVER_COLOR: Color = Color::srgb(0.7, 0.7, 0.7);
const THUMB_DRAG_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
const TRACK_COLOR: Color = Color::srgb(0.2, 0.2, 0.2);

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatState>()
            .init_resource::<ScrollbarDragState>()
            .add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    handle_keyboard_input,
                    process_incoming_messages,
                    spawn_new_messages,
                    handle_mouse_wheel,
                    handle_scrollbar_interaction,
                    update_scrollbar_thumb,
                    update_input_display,
                    update_status_bar,
                    auto_scroll_on_new_message,
                ),
            );
    }
}

#[derive(Resource, Default)]
struct ChatState {
    messages: Vec<ChatMessage>,
    input_buffer: String,
    spawned_count: usize,
    input_mode: InputMode,
}

#[derive(Default, PartialEq, Eq)]
enum InputMode {
    #[default]
    Chat,
    RelaySecretEntry,
    NameEntry,
    SecretEntry,
}

struct ChatMessage {
    from: String,
    text: String,
    is_system: bool,
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
struct StatusBar;

#[derive(Component)]
struct ScrollbarTrack;

#[derive(Resource)]
struct FrakturFont(Handle<Font>);

#[derive(Component)]
struct ScrollbarThumb;

fn setup_ui(mut commands: Commands, state: Res<ConnectionState>, mut chat: ResMut<ChatState>, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    let fraktur_font = asset_server.load("fonts/NotoSansMath-Regular.ttf");
    commands.insert_resource(FrakturFont(fraktur_font));

    if *state == ConnectionState::FirstLaunch {
        chat.input_mode = InputMode::NameEntry;
        chat.messages.push(ChatMessage {
            from: String::new(),
            text: "Welcome! Enter your name:".into(),
            is_system: true,
        });
    } else if *state == ConnectionState::NeedsRelaySecret {
        chat.input_mode = InputMode::RelaySecretEntry;
        chat.messages.push(ChatMessage {
            from: String::new(),
            text: "Enter the relay secret:".into(),
            is_system: true,
        });
    }

    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|root| {
            // Status bar
            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                min_height: Val::Px(32.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_child((
                Text::new("Connecting..."),
                TextFont::from_font_size(14.0),
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                StatusBar,
            ));

            // Chat area row: history + scrollbar
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                min_height: Val::Px(0.0),
                ..default()
            })
            .with_children(|chat_row| {
                // Chat history — scrollable
                chat_row.spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        padding: UiRect::all(Val::Px(12.0)),
                        min_height: Val::Px(0.0),
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
    mut commands: Commands,
    mut config: ResMut<ClientConfig>,
    mut conn_state: ResMut<ConnectionState>,
    net: Option<Res<NetSocket>>,
) {
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }

        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let trimmed = chat.input_buffer.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }

                match chat.input_mode {
                    InputMode::NameEntry => {
                        // Validate: only a-z, A-Z
                        if trimmed.chars().all(|c| c.is_ascii_alphabetic()) && trimmed.len() <= 20
                        {
                            config.config.identity_name = trimmed.clone();
                            config.config.identity_secret = generate_identity_secret();
                            save_config(&config.data_dir, &config.config);

                            chat.messages.push(ChatMessage {
                                from: trimmed.clone(),
                                text: " registered. Enter the relay secret:".into(),
                                is_system: true,
                            });
                            chat.input_mode = InputMode::RelaySecretEntry;
                        } else {
                            chat.messages.push(ChatMessage {
                                from: String::new(),
                                text: "Name must be 1-20 letters (a-z, A-Z) only.".into(),
                                is_system: true,
                            });
                        }
                        chat.input_buffer.clear();
                    }
                    InputMode::RelaySecretEntry => {
                        bevy::log::info!("relay secret entered, connecting...");
                        config.config.relay_secret = Some(trimmed);
                        // Don't save yet — wait for Welcome to confirm the secret is correct
                        chat.messages.push(ChatMessage {
                            from: String::new(),
                            text: "Connecting...".into(),
                            is_system: true,
                        });
                        chat.input_mode = InputMode::Chat;
                        start_connection(
                            &mut commands,
                            &mut config,
                            &mut conn_state,
                        );
                        chat.input_buffer.clear();
                    }
                    InputMode::SecretEntry => {
                        config.config.identity_secret = trimmed;
                        save_config(&config.data_dir, &config.config);

                        chat.messages.push(ChatMessage {
                            from: String::new(),
                            text: "Secret updated. Reconnecting...".into(),
                            is_system: true,
                        });
                        chat.input_mode = InputMode::Chat;
                        *conn_state = ConnectionState::Connecting;
                        chat.input_buffer.clear();
                    }
                    InputMode::Chat => {
                        if let Some(ref net) = net {
                            send_chat_message(net, &trimmed);
                        }
                        chat.input_buffer.clear();
                    }
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

fn process_incoming_messages(
    mut incoming: ResMut<IncomingMessages>,
    mut chat: ResMut<ChatState>,
    mut conn_state: ResMut<ConnectionState>,
    mut config: ResMut<ClientConfig>,
) {
    for msg in incoming.0.drain(..) {
        match msg {
            RelayMessage::Broadcast { from, payload } => {
                if payload.is_empty() {
                    continue; // keepalive
                }
                if let Some(ChatPayload::Text(text)) = deserialize::<ChatPayload>(&payload) {
                    chat.messages.push(ChatMessage {
                        from: from.clone(),
                        text,
                        is_system: false,
                    });
                }
            }
            RelayMessage::PeerJoined { name } => {
                chat.messages.push(ChatMessage {
                    from: name,
                    text: " joined".into(),
                    is_system: true,
                });
            }
            RelayMessage::PeerLeft { name } => {
                chat.messages.push(ChatMessage {
                    from: name,
                    text: " left".into(),
                    is_system: true,
                });
            }
            RelayMessage::RejectSecret => {
                bevy::log::info!("received RejectSecret from relay");
                config.config.relay_secret = None;
                chat.messages.push(ChatMessage {
                    from: String::new(),
                    text: "Relay secret rejected. Try again:".into(),
                    is_system: true,
                });
                chat.input_mode = InputMode::RelaySecretEntry;
                *conn_state = ConnectionState::NeedsRelaySecret;
            }
            RelayMessage::NameClaimed => {
                chat.messages.push(ChatMessage {
                    from: String::new(),
                    text: "This name is already taken. Enter your identity secret:".into(),
                    is_system: true,
                });
                chat.input_mode = InputMode::SecretEntry;
                *conn_state = ConnectionState::NameClaimed;
            }
            RelayMessage::Welcome { .. } => {
                bevy::log::info!("received Welcome from relay, saving config");
                save_config(&config.data_dir, &config.config);
            }
            RelayMessage::RejectVersion { expected } => {
                chat.messages.push(ChatMessage {
                    from: String::new(),
                    text: format!("Version mismatch. Expected: {expected}"),
                    is_system: true,
                });
            }
        }
    }
}

fn spawn_new_messages(
    mut commands: Commands,
    mut chat: ResMut<ChatState>,
    container_query: Query<Entity, With<ChatHistoryContainer>>,
    fraktur_font: Option<Res<FrakturFont>>,
) {
    if chat.spawned_count >= chat.messages.len() {
        return;
    }

    let Ok(container) = container_query.single() else {
        return;
    };

    for i in chat.spawned_count..chat.messages.len() {
        let msg = &chat.messages[i];

        let system_color = Color::srgb(0.5, 0.5, 0.3);

        let child = if msg.is_system && msg.from.is_empty() {
            commands
                .spawn((
                    Text::new(format!("* {}", msg.text)),
                    TextFont::from_font_size(MESSAGE_FONT_SIZE),
                    TextColor(system_color),
                ))
                .id()
        } else if msg.is_system {
            // System message with a name — render name in Fraktur
            commands
                .spawn((
                    Text::new("* "),
                    TextFont::from_font_size(MESSAGE_FONT_SIZE),
                    TextColor(system_color),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        TextSpan::new(to_fraktur(&msg.from)),
                        TextFont {
                            font: fraktur_font.as_ref().map(|f| f.0.clone()).unwrap_or_default(),
                            font_size: MESSAGE_FONT_SIZE,
                            ..default()
                        },
                        TextColor(Color::srgb(0.6, 0.8, 1.0)),
                    ));
                    parent.spawn((
                        TextSpan::new(&msg.text),
                        TextFont::from_font_size(MESSAGE_FONT_SIZE),
                        TextColor(system_color),
                    ));
                })
                .id()
        } else {
            commands
                .spawn((
                    Text::new(format!("{} ", to_fraktur(&msg.from))),
                    TextFont {
                        font: fraktur_font.as_ref().map(|f| f.0.clone()).unwrap_or_default(),
                        font_size: MESSAGE_FONT_SIZE,
                        ..default()
                    },
                    TextColor(Color::srgb(0.6, 0.8, 1.0)),
                ))
                .with_child((
                    TextSpan::new(&msg.text),
                    TextFont::from_font_size(MESSAGE_FONT_SIZE),
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                ))
                .id()
        };
        commands.entity(container).add_child(child);
    }
    chat.spawned_count = chat.messages.len();
}

fn update_input_display(
    chat: Res<ChatState>,
    mut query: Query<(&mut Text, &mut TextColor), With<InputText>>,
) {
    if !chat.is_changed() {
        return;
    }

    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };

    let prefix = match chat.input_mode {
        InputMode::Chat => "> ",
        InputMode::RelaySecretEntry => "relay> ",
        InputMode::NameEntry => "name> ",
        InputMode::SecretEntry => "secret> ",
    };

    let input_color = match chat.input_mode {
        InputMode::Chat => Color::srgb(0.3, 1.0, 0.3),
        InputMode::RelaySecretEntry => Color::srgb(1.0, 0.5, 0.3),
        InputMode::NameEntry => Color::srgb(1.0, 1.0, 0.3),
        InputMode::SecretEntry => Color::srgb(1.0, 0.5, 0.3),
    };
    color.0 = input_color;

    if chat.input_buffer.is_empty() {
        **text = format!("{prefix}_");
    } else {
        **text = format!("{prefix}{}_", chat.input_buffer);
    }
}

fn update_status_bar(
    conn_state: Res<ConnectionState>,
    config: Res<ClientConfig>,
    peers: Res<PeerList>,
    mut query: Query<&mut Text, With<StatusBar>>,
) {
    if !conn_state.is_changed() && !peers.is_changed() {
        return;
    }

    let Ok(mut text) = query.single_mut() else {
        return;
    };

    let name_display = if config.config.identity_name.is_empty() {
        "?".to_string()
    } else {
        config.config.identity_name.clone()
    };

    let status = match *conn_state {
        ConnectionState::Loading => "Loading...".into(),
        ConnectionState::FirstLaunch => "Enter your name to begin".into(),
        ConnectionState::NeedsRelaySecret => format!("{name_display} | Enter relay secret"),
        ConnectionState::Connecting => format!("{name_display} | Connecting..."),
        ConnectionState::Connected => {
            let peer_count = peers.0.len();
            let peer_names: String = peers
                .0
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            if peer_count == 0 {
                format!("{name_display} | Connected (no peers)")
            } else {
                format!("{name_display} | {peer_count} peer(s): {peer_names}")
            }
        }
        ConnectionState::NameClaimed => format!("{name_display} | Name claimed - enter secret"),
        ConnectionState::Disconnected => format!("{name_display} | Disconnected"),
    };

    **text = status;
}

// --- Scrollbar systems (carried over from chat_prototype.rs) ---

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

    if drag.dragging && !mouse_button.pressed(MouseButton::Left) {
        drag.dragging = false;
    }

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

    if drag.dragging {
        thumb_color.0 = THUMB_DRAG_COLOR;
    } else {
        match *thumb_interaction {
            Interaction::Hovered => thumb_color.0 = THUMB_HOVER_COLOR,
            Interaction::Pressed => thumb_color.0 = THUMB_DRAG_COLOR,
            Interaction::None => thumb_color.0 = THUMB_COLOR,
        }
    }

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

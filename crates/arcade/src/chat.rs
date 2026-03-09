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
    ClientConfig, ConnectRequest, ConnectionState, NetSocket, PeerList, ReceiveSet, RelayEvent,
    send_chat_message,
};

/// Normalize a name: first letter uppercase, rest lowercase.
fn normalize_name(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut s = first.to_ascii_uppercase().to_string();
            for c in chars {
                s.push(c.to_ascii_lowercase());
            }
            s
        }
    }
}

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
            .add_message::<TextSubmitted>()
            .add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    (read_text_input, handle_text_submit).chain(),
                    (process_incoming_messages, spawn_new_messages)
                        .chain()
                        .after(ReceiveSet),
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
    welcomed: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum InputMode {
    #[default]
    Chat,
    RelaySecretEntry,
    NameEntry,
    SecretEntry,
}

#[derive(Message)]
struct TextSubmitted(String);

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

fn setup_ui(mut commands: Commands, state: Res<ConnectionState>, mut chat: ResMut<ChatState>, mut fonts: ResMut<Assets<Font>>, assets_dir: Res<crate::assets::AssetsDir>) {
    commands.spawn(Camera2d);

    let font_path = assets_dir.0.join("fonts/NotoSansMath-Regular.ttf");
    let font_bytes = std::fs::read(&font_path).unwrap_or_else(|e| {
        panic!("failed to load font from {}: {e}", font_path.display());
    });
    let fraktur_font = fonts.add(Font::try_from_bytes(font_bytes).expect("failed to parse font"));
    commands.insert_resource(FrakturFont(fraktur_font));

    if *state == ConnectionState::FirstLaunch {
        chat.input_mode = InputMode::NameEntry;
        chat.messages.push(ChatMessage {
            from: String::new(),
            text: "Welcome! Choose a name (one word, letters only):".into(),
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

/// Input layer: translates raw keyboard events into buffer edits and TextSubmitted events.
fn read_text_input(
    mut events: MessageReader<KeyboardInput>,
    mut chat: ResMut<ChatState>,
    mut submit: MessageWriter<TextSubmitted>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }

        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let trimmed = chat.input_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    submit.write(TextSubmitted(trimmed));
                    chat.input_buffer.clear();
                }
            }
            KeyCode::Backspace => {
                chat.input_buffer.pop();
            }
            KeyCode::KeyC if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) => {
                let history = chat.messages.iter()
                    .map(|m| {
                        if m.from.is_empty() {
                            m.text.clone()
                        } else {
                            format!("{}: {}", m.from, m.text)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(history);
                }
            }
            KeyCode::KeyV if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        chat.input_buffer.push_str(&text);
                    }
                }
            }
            _ => {
                if let Some(ref text) = event.text {
                    chat.input_buffer.push_str(text);
                }
            }
        }
    }
}

/// Simulation layer: dispatches submitted text based on current input mode.
fn handle_text_submit(
    mut events: MessageReader<TextSubmitted>,
    mut chat: ResMut<ChatState>,
    mut config: ResMut<ClientConfig>,
    mut connect: MessageWriter<ConnectRequest>,
    net: Option<Res<NetSocket>>,
) {
    for TextSubmitted(text) in events.read() {
        match chat.input_mode {
            InputMode::NameEntry => {
                if text.chars().all(|c| c.is_ascii_alphabetic()) && text.len() <= 20 {
                    let name = normalize_name(text);
                    config.config.identity_name = name.clone();
                    config.config.identity_secret = generate_identity_secret();
                    save_config(&config.data_dir, &config.config);

                    chat.messages.push(ChatMessage {
                        from: name,
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
            }
            InputMode::RelaySecretEntry => {
                bevy::log::info!("relay secret entered, connecting...");
                config.config.relay_secret = Some(text.clone());
                chat.messages.push(ChatMessage {
                    from: String::new(),
                    text: "Connecting...".into(),
                    is_system: true,
                });
                chat.input_mode = InputMode::Chat;
                connect.write(ConnectRequest);
            }
            InputMode::SecretEntry => {
                config.config.identity_secret = text.clone();
                save_config(&config.data_dir, &config.config);

                chat.messages.push(ChatMessage {
                    from: String::new(),
                    text: "Secret updated. Reconnecting...".into(),
                    is_system: true,
                });
                chat.input_mode = InputMode::Chat;
                connect.write(ConnectRequest);
            }
            InputMode::Chat => {
                if let Some(ref net) = net {
                    send_chat_message(net, &text);
                }
            }
        }
    }
}

/// Download chat history from S3 and push messages into the chat state.
/// Fetches only this version's history from the per-version S3 layout.
/// Best-effort: if the download or parse fails, we just skip history.
fn fetch_chat_history_from_s3(chat: &mut ChatState) {
    let commit_hash = crate::version::COMMIT_HASH;
    let url = format!(
        "https://arcade.seanshubin.com/admin/versions/{commit_hash}/chat-history.json"
    );
    let body = match ureq::get(&url).call() {
        Ok(mut resp) => match resp.body_mut().read_to_string() {
            Ok(s) => s,
            Err(_) => return,
        },
        Err(_) => return,
    };

    let persisted: Vec<protocol::PersistedHistoryEntry> = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_) => return,
    };

    for entry in protocol::restore_entries(persisted) {
        if entry.payload.is_empty() {
            continue;
        }
        if let Some(ChatPayload::Text(text)) = deserialize::<ChatPayload>(&entry.payload) {
            chat.messages.push(ChatMessage {
                from: entry.from,
                text,
                is_system: false,
            });
        }
    }
}

/// Chat-domain consumer of relay events: display messages and manage input mode.
/// Net-domain concerns (ConnectionState, PeerList, Config) are handled in net.rs.
fn process_incoming_messages(
    mut events: MessageReader<RelayEvent>,
    mut chat: ResMut<ChatState>,
    config: Res<ClientConfig>,
) {
    for RelayEvent(msg) in events.read() {
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
            RelayMessage::PeerJoined { .. } => {}
            RelayMessage::PeerLeft { .. } => {}
            RelayMessage::RejectSecret => {
                chat.messages.push(ChatMessage {
                    from: String::new(),
                    text: "Relay secret rejected. Try again:".into(),
                    is_system: true,
                });
                chat.input_mode = InputMode::RelaySecretEntry;
            }
            RelayMessage::NameClaimed => {
                chat.messages.push(ChatMessage {
                    from: String::new(),
                    text: "This name is already taken. Enter your identity secret:".into(),
                    is_system: true,
                });
                chat.input_mode = InputMode::SecretEntry;
            }
            RelayMessage::ChatHistory { .. } => {
                // Chat history is now downloaded from S3 on Welcome, not sent
                // over UDP. This variant is kept for protocol compatibility.
            }
            RelayMessage::Welcome { .. } => {
                if !chat.welcomed {
                    chat.welcomed = true;

                    let config_path = config.data_dir.join("config.toml");
                    chat.messages.push(ChatMessage {
                        from: String::new(),
                        text: format!("Your config is at: {}", config_path.display()),
                        is_system: true,
                    });

                    // Download chat history from S3 (canonical store).
                    fetch_chat_history_from_s3(&mut chat);
                }
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

    let display_buffer = if chat.input_mode == InputMode::NameEntry {
        normalize_name(&chat.input_buffer)
    } else {
        chat.input_buffer.clone()
    };

    if display_buffer.is_empty() {
        **text = format!("{prefix}_");
    } else {
        **text = format!("{prefix}{display_buffer}_");
    }
}

fn update_status_bar(
    conn_state: Res<ConnectionState>,
    config: Res<ClientConfig>,
    peers: Res<PeerList>,
    version_status: Res<crate::version::VersionStatus>,
    retry_state: Res<crate::version::VersionRetryState>,
    mut query: Query<(&mut Text, &mut TextColor), With<StatusBar>>,
) {
    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };

    if *version_status == crate::version::VersionStatus::Offline {
        let countdown = crate::version::retry_countdown(&retry_state);
        **text = format!("OFFLINE | retry in {countdown:.0}s");
        color.0 = Color::srgb(1.0, 0.4, 0.4);
        return;
    }

    color.0 = Color::srgb(0.6, 0.6, 0.6);

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
            if peers.0.is_empty() {
                format!("{name_display} | Connected")
            } else {
                let peer_names: String = peers
                    .0
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name_display} | {peer_names}")
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use protocol::RelayMessage;

    /// Test orchestrator for chat system logic.
    /// Hides Bevy App setup; exposes domain-focused methods.
    struct ChatTester {
        app: App,
    }

    impl ChatTester {
        fn new() -> Self {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            app.add_message::<RelayEvent>();
            app.add_message::<TextSubmitted>();
            app.add_message::<ConnectRequest>();
            app.init_resource::<ChatState>();

            // Insert a ClientConfig with test data
            let dir = std::env::temp_dir().join(format!("chat_test_{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            app.insert_resource(ClientConfig {
                config: crate::config::Config {
                    identity_name: "Alice".into(),
                    identity_secret: "secret".into(),
                    relay_address: "127.0.0.1:7700".into(),
                    relay_secret: Some("test".into()),
                    new_identity_secret: None,
                },
                data_dir: dir,
            });

            app.add_systems(
                Update,
                (
                    handle_text_submit,
                    process_incoming_messages,
                ),
            );

            ChatTester { app }
        }

        fn with_input_mode(mut self, mode: InputMode) -> Self {
            self.app.world_mut().resource_mut::<ChatState>().input_mode = mode;
            self
        }

        // -- Actions --

        fn receive_relay_message(&mut self, msg: RelayMessage) {
            self.app.world_mut().write_message(RelayEvent(msg));
            self.app.update();
        }

        fn submit_text(&mut self, text: &str) {
            self.app
                .world_mut()
                .write_message(TextSubmitted(text.to_string()));
            self.app.update();
        }

        // -- Queries --

        fn message_count(&self) -> usize {
            self.app.world().resource::<ChatState>().messages.len()
        }

        fn last_message_text(&self) -> String {
            let chat = self.app.world().resource::<ChatState>();
            chat.messages.last().map(|m| m.text.clone()).unwrap_or_default()
        }

        fn last_message_from(&self) -> String {
            let chat = self.app.world().resource::<ChatState>();
            chat.messages.last().map(|m| m.from.clone()).unwrap_or_default()
        }

        fn last_message_is_system(&self) -> bool {
            let chat = self.app.world().resource::<ChatState>();
            chat.messages.last().map(|m| m.is_system).unwrap_or(false)
        }

        fn input_mode(&self) -> &InputMode {
            &self.app.world().resource::<ChatState>().input_mode
        }

        fn identity_name(&self) -> String {
            self.app
                .world()
                .resource::<ClientConfig>()
                .config
                .identity_name
                .clone()
        }

        fn relay_secret(&self) -> Option<String> {
            self.app
                .world()
                .resource::<ClientConfig>()
                .config
                .relay_secret
                .clone()
        }

        fn connect_requests_sent(&self) -> usize {
            let messages = self.app.world().resource::<Messages<ConnectRequest>>();
            messages.iter_current_update_messages().count()
        }
    }

    // -- process_incoming_messages tests --

    #[test]
    fn broadcast_creates_chat_message() {
        // given
        let mut tester = ChatTester::new();

        // when
        let payload = protocol::serialize(&protocol::ChatPayload::Text("hello!".into()));
        tester.receive_relay_message(RelayMessage::Broadcast {
            from: "Bob".into(),
            payload,
        });

        // then
        assert_eq!(tester.message_count(), 1);
        assert_eq!(tester.last_message_from(), "Bob");
        assert_eq!(tester.last_message_text(), "hello!");
        assert!(!tester.last_message_is_system());
    }

    #[test]
    fn empty_broadcast_is_ignored() {
        // given
        let mut tester = ChatTester::new();

        // when
        tester.receive_relay_message(RelayMessage::Broadcast {
            from: "Bob".into(),
            payload: vec![],
        });

        // then
        assert_eq!(tester.message_count(), 0);
    }

    #[test]
    fn peer_joined_produces_no_chat_message() {
        // given
        let mut tester = ChatTester::new();

        // when
        tester.receive_relay_message(RelayMessage::PeerJoined {
            name: "Charlie".into(),
        });

        // then
        assert_eq!(tester.message_count(), 0);
    }

    #[test]
    fn peer_left_produces_no_chat_message() {
        // given
        let mut tester = ChatTester::new();

        // when
        tester.receive_relay_message(RelayMessage::PeerLeft {
            name: "Charlie".into(),
        });

        // then
        assert_eq!(tester.message_count(), 0);
    }

    #[test]
    fn reject_secret_prompts_reentry() {
        // given
        let mut tester = ChatTester::new();

        // when
        tester.receive_relay_message(RelayMessage::RejectSecret);

        // then
        assert!(tester.last_message_text().contains("rejected"));
        assert_eq!(tester.input_mode(), &InputMode::RelaySecretEntry);
    }

    #[test]
    fn name_claimed_prompts_secret_entry() {
        // given
        let mut tester = ChatTester::new();

        // when
        tester.receive_relay_message(RelayMessage::NameClaimed);

        // then
        assert!(tester.last_message_text().contains("already taken"));
        assert_eq!(tester.input_mode(), &InputMode::SecretEntry);
    }

    #[test]
    fn reject_version_shows_expected() {
        // given
        let mut tester = ChatTester::new();

        // when
        tester.receive_relay_message(RelayMessage::RejectVersion {
            expected: "abc123".into(),
        });

        // then
        assert!(tester.last_message_text().contains("abc123"));
    }

    #[test]
    fn welcome_shows_config_path() {
        // given
        let mut tester = ChatTester::new();

        // when
        tester.receive_relay_message(RelayMessage::Welcome { peer_count: 2 });

        // then
        assert_eq!(tester.message_count(), 1);
        assert!(tester.last_message_is_system());
        assert!(tester.last_message_text().contains("config.toml"),
            "expected config path in: {}", tester.last_message_text());
    }

    // -- handle_text_submit tests --

    #[test]
    fn name_entry_valid_name_transitions_to_relay_secret() {
        // given
        let mut tester = ChatTester::new().with_input_mode(InputMode::NameEntry);

        // when
        tester.submit_text("Bob");

        // then
        assert_eq!(tester.input_mode(), &InputMode::RelaySecretEntry);
        assert_eq!(tester.identity_name(), "Bob");
    }

    #[test]
    fn name_entry_normalizes_capitalization() {
        // given
        let mut tester = ChatTester::new().with_input_mode(InputMode::NameEntry);

        // when
        tester.submit_text("bOB");

        // then
        assert_eq!(tester.identity_name(), "Bob");
    }

    #[test]
    fn name_entry_invalid_name_stays_in_name_entry() {
        // given
        let mut tester = ChatTester::new().with_input_mode(InputMode::NameEntry);

        // when
        tester.submit_text("Bob123");

        // then
        assert_eq!(tester.input_mode(), &InputMode::NameEntry);
        assert!(tester.last_message_text().contains("letters"));
    }

    #[test]
    fn name_entry_too_long_stays_in_name_entry() {
        // given
        let mut tester = ChatTester::new().with_input_mode(InputMode::NameEntry);

        // when
        tester.submit_text("Abcdefghijklmnopqrstuvwxyz");

        // then
        assert_eq!(tester.input_mode(), &InputMode::NameEntry);
    }

    #[test]
    fn relay_secret_entry_transitions_to_chat_and_connects() {
        // given
        let mut tester = ChatTester::new().with_input_mode(InputMode::RelaySecretEntry);

        // when
        tester.submit_text("mysecret");

        // then
        assert_eq!(tester.input_mode(), &InputMode::Chat);
        assert_eq!(tester.relay_secret(), Some("mysecret".into()));
        assert_eq!(tester.connect_requests_sent(), 1);
    }

    #[test]
    fn secret_entry_updates_identity_secret_and_connects() {
        // given
        let mut tester = ChatTester::new().with_input_mode(InputMode::SecretEntry);

        // when
        tester.submit_text("new-identity-secret");

        // then
        assert_eq!(tester.input_mode(), &InputMode::Chat);
        assert_eq!(tester.connect_requests_sent(), 1);
    }

    // -- Scrollbar helper tests --

    #[test]
    fn max_scroll_zero_when_content_fits() {
        assert_eq!(max_scroll(100.0, 200.0), 0.0);
    }

    #[test]
    fn max_scroll_positive_when_content_overflows() {
        assert_eq!(max_scroll(500.0, 200.0), 300.0);
    }

    #[test]
    fn thumb_height_fills_track_when_content_fits() {
        assert_eq!(thumb_height(200.0, 100.0, 200.0), 200.0);
    }

    #[test]
    fn thumb_height_proportional_when_content_overflows() {
        let h = thumb_height(200.0, 400.0, 200.0);
        assert_eq!(h, 100.0); // viewport/content * track = 0.5 * 200
    }

    #[test]
    fn thumb_height_respects_minimum() {
        let h = thumb_height(200.0, 10000.0, 200.0);
        assert_eq!(h, THUMB_MIN_HEIGHT);
    }
}

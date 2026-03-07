# Bevy Scrollable UI (0.18)

How to build a scrollable container with mouse wheel and scrollbar in Bevy 0.18's native UI. Discovered through debugging `examples/chat_prototype.rs`.

## Key Findings

### 1. `min_height: Val::Px(0.0)` is required for scroll containers

Bevy's flexbox (via Taffy) defaults to `min-height: auto`, which means flex items grow to fit their content. A container with `overflow: Overflow::scroll_y()` won't scroll unless both it **and its parent** have `min_height: Val::Px(0.0)`. Without this, the container expands to fit all children, so content never overflows.

This is the same gotcha as CSS flexbox — `min-height: 0` on flex items to allow shrinking.

```rust
// Parent row
Node {
    flex_grow: 1.0,
    flex_direction: FlexDirection::Row,
    min_height: Val::Px(0.0),  // Required
    ..default()
}

// Scrollable container
Node {
    flex_grow: 1.0,
    flex_direction: FlexDirection::Column,
    overflow: Overflow::scroll_y(),
    min_height: Val::Px(0.0),  // Required
    ..default()
}
```

### 2. Each scrollable item must be its own entity

A single `Text` node containing all content (e.g., all chat messages concatenated) won't create layout overflow. The text node gets sized to match its container, and the text visually overflows without the layout system knowing — `content_size()` equals `size()`.

Instead, spawn each item as a separate child entity:

```rust
let child = commands
    .spawn((
        Text::new(message),
        TextFont::from_font_size(18.0),
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
    ))
    .id();
commands.entity(container).add_child(child);
```

### 3. UI nodes use `UiGlobalTransform`, not `GlobalTransform`

Bevy 0.18 UI nodes have `UiGlobalTransform` (an `Affine2` wrapper), not `GlobalTransform`. Querying `GlobalTransform` on a UI entity silently returns no results — the query fails and systems early-return with no error or warning.

```rust
// Wrong — query silently fails, system does nothing
Query<(&Interaction, &GlobalTransform, &ComputedNode), With<MyUiNode>>

// Correct
use bevy::ui::UiGlobalTransform;
Query<(&Interaction, &UiGlobalTransform, &ComputedNode), With<MyUiNode>>
```

### 4. Converting screen position to node-local coordinates

Use `ComputedNode::normalize_point()` with `UiGlobalTransform`:

```rust
// Returns Option<Vec2> in [-0.5, 0.5] centered on the node
let normalized = computed_node.normalize_point(*ui_transform, cursor_screen_pos);

// Convert to [0, node_height] from top
let local_y = normalized.map(|n| (n.y + 0.5) * node_size.y);
```

### 5. `ComputedNode::content_size()` vs `size()`

- `size()` — the node's laid-out size (constrained by parent)
- `content_size()` — the content size including overflow

When the container is properly constrained (with `min_height: 0`), `content_size().y` will exceed `size().y` when children overflow. Use their difference for max scroll:

```rust
let viewport_h = computed.size().y;
let content_h = computed.content_size().y;
let max_scroll = (content_h - viewport_h).max(0.0);
```

### 6. `ScrollPosition` is a newtype around `Vec2`

`ScrollPosition(pub Vec2)` with `Deref`/`DerefMut`. Bevy respects it for clipping but does not wire up mouse wheel or any input to it — you must write systems to update `scroll.y` yourself.

### 7. Bevy 0.18 uses `MessageReader`, not `EventReader`

`KeyboardInput`, `MouseWheel`, and other input types are registered as messages:

```rust
fn handle_mouse_wheel(
    mut wheel_events: MessageReader<MouseWheel>,
    // ...
)
```

## Scrollbar Implementation Notes

The scrollbar in `chat_prototype.rs` consists of:
- A **track** node (dark background, full height, `Interaction` component)
- A **thumb** node (lighter color, absolute-positioned child of track, `Interaction` component)

The thumb's `top` and `height` are updated each frame based on scroll position and content ratio. Dragging works by capturing the cursor's track-local Y on press, then mapping cursor delta to scroll delta proportionally.

## Reference

Working example: `examples/chat_prototype.rs`

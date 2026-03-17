//! Maze with keys and gates (wrapping topology).
//!
//! 4×4 cell grid on a torus. 16 px walls, 64 px corridors.
//! 3 concentric regions separated by color-coded gates.
//! Collect key 0 (red) to enter region 2, then key 1 (blue) to reach the goal.
//!
//! Region layout (cells):
//!   1 1 1 1
//!   1 2 2 1
//!   1 2 3 1
//!   1 1 1 1
//!
//! Controls:
//!   Arrow keys / WASD / D-pad / left stick: move
//!   N: new maze (random seed)
//!   R: restart current maze
//!
//! Run with: `cargo run --example maze`

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::{Rng, RngExt, SeedableRng};
use std::collections::{HashSet, VecDeque};

// ---------------------------------------------------------------------------
// Maze layout constants
// ---------------------------------------------------------------------------

const WALL_PX: usize = 16;
const CORR_PX: usize = 64;
const COLS: usize = 4;
const ROWS: usize = 4;
const CELL: usize = WALL_PX + CORR_PX; // 80
const MAZE_W: usize = COLS * CELL; // 320
const MAZE_H: usize = ROWS * CELL; // 320
const HUD_H: usize = 64;
const IMG_W: usize = MAZE_W; // 320
const IMG_H: usize = MAZE_H + HUD_H; // 384
const MOVE_SPEED: f32 = 640.0;
const STICK_DEAD: f32 = 0.3;
const NUM_KEYS: usize = 2;
const ITEM_SIZE: usize = CORR_PX / 2;

// Fixed concentric regions (closed loops on the torus).
const REGION: [[u8; COLS]; ROWS] = [
    [1, 1, 1, 1],
    [1, 2, 2, 1],
    [1, 2, 3, 1],
    [1, 1, 1, 1],
];

// ---------------------------------------------------------------------------
// Key sprite sheet
// ---------------------------------------------------------------------------

const SHEET_COLS: usize = 14;
const SHEET_W: usize = SHEET_COLS * 64;
const KEY_SPRITE_SIZE: usize = 64;
const KEYS_PER_COLOR: usize = 20;

const KEY_COLORS: [[u8; 4]; 9] = [
    [205, 127,  50, 255], // 0: Bronze
    [192, 192, 192, 255], // 1: Silver
    [240, 240, 240, 255], // 2: White
    [ 50,  50,  50, 255], // 3: Black
    [220,  50,  50, 255], // 4: Red
    [255, 215,   0, 255], // 5: Gold
    [ 50, 180,  50, 255], // 6: Green
    [ 50, 100, 220, 255], // 7: Blue
    [150,  50, 200, 255], // 8: Purple
];

// Which color index for each game key.
const KEY_COLOR_IDX: [usize; NUM_KEYS] = [4, 7]; // Red, Blue

fn key_sheet_pos(color: usize, design: usize) -> (usize, usize) {
    let linear = color * KEYS_PER_COLOR + design;
    (linear % SHEET_COLS, linear / SHEET_COLS)
}

// Colors
const COL_BG: [u8; 4] = [16, 16, 16, 255];
const COL_WALL: [u8; 4] = [160, 160, 160, 255];
const COL_PLAYER: [u8; 4] = [0, 220, 0, 255];
const COL_GOAL: [u8; 4] = [255, 255, 0, 255];
const COL_DIM: [u8; 4] = [50, 50, 50, 255];
const COL_WIN: [u8; 4] = [255, 255, 255, 255];

// ---------------------------------------------------------------------------
// Maze generation
// ---------------------------------------------------------------------------

type Edge = ((usize, usize), (usize, usize));

/// Canonical edge representation (smaller coordinate first).
fn edge(a: (usize, usize), b: (usize, usize)) -> Edge {
    if a <= b { (a, b) } else { (b, a) }
}

/// All four neighbors of a cell on the torus.
fn neighbors(cx: usize, cy: usize) -> [(usize, usize); 4] {
    [
        ((cx + 1) % COLS, cy),
        ((cx + COLS - 1) % COLS, cy),
        (cx, (cy + 1) % ROWS),
        (cx, (cy + ROWS - 1) % ROWS),
    ]
}

/// Maze layout generated at runtime.
#[derive(Resource)]
struct MazeLayout {
    seed: u64,
    /// Edges between same-region cells that are open passages.
    open_walls: HashSet<Edge>,
    /// Gates: (col, row, horizontal, key_index).
    gates: [(usize, usize, bool, usize); NUM_KEYS],
    /// Start cell (col, row).
    start: (usize, usize),
    /// Goal cell (col, row).
    goal: (usize, usize),
    /// Key cells: (col, row) for each key index.
    key_cells: [(usize, usize); NUM_KEYS],
    /// Key sprite design index (0-19) for each key.
    key_designs: [usize; NUM_KEYS],
}

/// Collect all cells belonging to a given region.
fn region_cells(r: u8) -> Vec<(usize, usize)> {
    let mut cells = Vec::new();
    for cy in 0..ROWS {
        for cx in 0..COLS {
            if REGION[cy][cx] == r { cells.push((cx, cy)); }
        }
    }
    cells
}

/// Collect all edges between cells of the same region `r`.
fn region_edges(r: u8) -> Vec<Edge> {
    let cells: HashSet<(usize, usize)> = region_cells(r).into_iter().collect();
    let mut edges = HashSet::new();
    for &(cx, cy) in &cells {
        for nb in neighbors(cx, cy) {
            if cells.contains(&nb) {
                edges.insert(edge((cx, cy), nb));
            }
        }
    }
    edges.into_iter().collect()
}

/// Collect all edges between two different regions (boundary edges).
fn boundary_edges(r1: u8, r2: u8) -> Vec<(usize, usize, usize, usize)> {
    let mut edges = Vec::new();
    for cy in 0..ROWS {
        for cx in 0..COLS {
            if REGION[cy][cx] != r1 && REGION[cy][cx] != r2 { continue; }
            for (nx, ny) in neighbors(cx, cy) {
                let a = REGION[cy][cx];
                let b = REGION[ny][nx];
                if (a == r1 && b == r2) || (a == r2 && b == r1) {
                    // Store as (lower_region_cell, higher_region_cell)
                    if a < b {
                        edges.push((cx, cy, nx, ny));
                    }
                }
            }
        }
    }
    // Deduplicate
    edges.sort();
    edges.dedup();
    edges
}

/// Union-Find for Kruskal's algorithm.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), rank: vec![0; n] }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x { self.parent[x] = self.find(self.parent[x]); }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb { return false; }
        if self.rank[ra] < self.rank[rb] { self.parent[ra] = rb; }
        else {
            self.parent[rb] = ra;
            if self.rank[ra] == self.rank[rb] { self.rank[ra] += 1; }
        }
        true
    }
}

/// Generate a random spanning tree for cells using Kruskal's with shuffled edges.
fn random_spanning_tree(
    cells: &[(usize, usize)],
    edges: &[Edge],
    rng: &mut impl Rng,
) -> HashSet<Edge> {
    let cell_idx: std::collections::HashMap<(usize, usize), usize> =
        cells.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let mut uf = UnionFind::new(cells.len());
    let mut shuffled: Vec<Edge> = edges.to_vec();
    // Fisher-Yates shuffle
    for i in (1..shuffled.len()).rev() {
        let j = rng.random_range(0..=i);
        shuffled.swap(i, j);
    }
    let mut tree = HashSet::new();
    for e in &shuffled {
        let a = cell_idx[&e.0];
        let b = cell_idx[&e.1];
        if uf.union(a, b) {
            tree.insert(*e);
        }
    }
    tree
}

/// Is a cell-level edge passable given the layout and current keys?
fn edge_passable(
    cx1: usize, cy1: usize, cx2: usize, cy2: usize,
    layout: &MazeLayout, keys: &[bool; NUM_KEYS],
) -> bool {
    let r1 = REGION[cy1][cx1];
    let r2 = REGION[cy2][cx2];
    if r1 == r2 {
        layout.open_walls.contains(&edge((cx1, cy1), (cx2, cy2)))
    } else {
        // Check if there's a gate here
        for &(gcx, gcy, horiz, ki) in &layout.gates {
            let (a, b) = if horiz {
                ((gcx, gcy), (gcx, (gcy + 1) % ROWS))
            } else {
                ((gcx, gcy), ((gcx + 1) % COLS, gcy))
            };
            if (a == (cx1, cy1) && b == (cx2, cy2)) || (a == (cx2, cy2) && b == (cx1, cy1)) {
                return keys[ki];
            }
        }
        false
    }
}

/// Flood fill from a cell, returning all reachable cells.
fn flood_fill(
    start: (usize, usize), layout: &MazeLayout, keys: &[bool; NUM_KEYS],
) -> HashSet<(usize, usize)> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);
    while let Some((cx, cy)) = queue.pop_front() {
        for (nx, ny) in neighbors(cx, cy) {
            if visited.contains(&(nx, ny)) { continue; }
            if edge_passable(cx, cy, nx, ny, layout, keys) {
                visited.insert((nx, ny));
                queue.push_back((nx, ny));
            }
        }
    }
    visited
}

fn is_solvable(layout: &MazeLayout) -> bool {
    // Phase 1: no keys — must reach key 0
    let reach = flood_fill(layout.start, layout, &[false, false]);
    if !reach.contains(&layout.key_cells[0]) { return false; }
    // Phase 2: key 0 — must reach key 1
    let reach = flood_fill(layout.start, layout, &[true, false]);
    if !reach.contains(&layout.key_cells[1]) { return false; }
    // Phase 3: both keys — must reach goal
    let reach = flood_fill(layout.start, layout, &[true, true]);
    reach.contains(&layout.goal)
}

/// Convert a cell-edge to a gate descriptor (col, row, horizontal).
fn edge_to_gate(cx1: usize, cy1: usize, cx2: usize, cy2: usize) -> (usize, usize, bool) {
    if cy1 == cy2 || (cy1 == 0 && cy2 == ROWS - 1) || (cy1 == ROWS - 1 && cy2 == 0) {
        // Horizontal neighbor — vertical wall (east wall of the left cell)
        if cx2 == (cx1 + 1) % COLS {
            (cx1, cy1, false) // east wall of (cx1, cy1)
        } else {
            (cx2, cy2, false) // east wall of (cx2, cy2)
        }
    } else {
        // Vertical neighbor — horizontal wall (south wall of the upper cell)
        if cy2 == (cy1 + 1) % ROWS {
            (cx1, cy1, true) // south wall of (cx1, cy1)
        } else {
            (cx2, cy2, true) // south wall of (cx2, cy2)
        }
    }
}

fn generate_maze(seed: u64) -> MazeLayout {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

    loop {
        // Build spanning trees for each region
        let mut open_walls = HashSet::new();
        for r in 1..=3u8 {
            let cells = region_cells(r);
            let edges = region_edges(r);
            if edges.is_empty() { continue; }
            let tree = random_spanning_tree(&cells, &edges, &mut rng);
            open_walls.extend(tree);
        }

        // Pick random gates on region boundaries
        let boundary_1_2 = boundary_edges(1, 2);
        let boundary_2_3 = boundary_edges(2, 3);
        let g0_idx = rng.random_range(0..boundary_1_2.len());
        let g1_idx = rng.random_range(0..boundary_2_3.len());
        let (g0cx, g0cy, g0nx, g0ny) = boundary_1_2[g0_idx];
        let (g1cx, g1cy, g1nx, g1ny) = boundary_2_3[g1_idx];
        let (g0c, g0r, g0h) = edge_to_gate(g0cx, g0cy, g0nx, g0ny);
        let (g1c, g1r, g1h) = edge_to_gate(g1cx, g1cy, g1nx, g1ny);
        let gates = [
            (g0c, g0r, g0h, 0),
            (g1c, g1r, g1h, 1),
        ];

        // Pick random placements
        let r1_cells = region_cells(1);
        let r2_cells = region_cells(2);
        let r3_cells = region_cells(3);

        let start_idx = rng.random_range(0..r1_cells.len());
        let start = r1_cells[start_idx];

        // Key 0 in region 1, different from start
        let mut k0 = start;
        while k0 == start {
            k0 = r1_cells[rng.random_range(0..r1_cells.len())];
        }

        // Key 1 in region 2
        let k1 = r2_cells[rng.random_range(0..r2_cells.len())];

        // Goal in region 3
        let goal = r3_cells[rng.random_range(0..r3_cells.len())];

        // Random key sprite designs
        let key_designs = [
            rng.random_range(0..KEYS_PER_COLOR),
            rng.random_range(0..KEYS_PER_COLOR),
        ];

        let layout = MazeLayout {
            seed,
            open_walls,
            gates,
            start,
            goal,
            key_cells: [k0, k1],
            key_designs,
        };

        if is_solvable(&layout) {
            return layout;
        }
        // Retry with same seed but different internal random state (loop continues)
    }
}

// ---------------------------------------------------------------------------
// Collision (reads from MazeLayout)
// ---------------------------------------------------------------------------

fn wall_solid(
    cx1: usize, cy1: usize, cx2: usize, cy2: usize,
    layout: &MazeLayout, keys: &[bool; NUM_KEYS],
) -> bool {
    !edge_passable(cx1, cy1, cx2, cy2, layout, keys)
}

fn build_collision(layout: &MazeLayout, keys: &[bool; NUM_KEYS]) -> Vec<bool> {
    let mut map = vec![false; MAZE_W * MAZE_H];
    for py in 0..MAZE_H {
        for px in 0..MAZE_W {
            let on_v = px % CELL == 0;
            let on_h = py % CELL == 0;
            let solid = if on_v && on_h {
                true
            } else if on_v {
                let cx_r = (px / CELL) % COLS;
                let cx_l = (cx_r + COLS - 1) % COLS;
                wall_solid(cx_l, py / CELL, cx_r, py / CELL, layout, keys)
            } else if on_h {
                let cy_b = (py / CELL) % ROWS;
                let cy_a = (cy_b + ROWS - 1) % ROWS;
                wall_solid(px / CELL, cy_a, px / CELL, cy_b, layout, keys)
            } else {
                false
            };
            if solid {
                let wx = if on_v { WALL_PX } else { 1 };
                let wy = if on_h { WALL_PX } else { 1 };
                for fy in 0..wy {
                    for fx in 0..wx {
                        let x = px + fx;
                        let y = py + fy;
                        if x < MAZE_W && y < MAZE_H {
                            map[y * MAZE_W + x] = true;
                        }
                    }
                }
            }
        }
    }
    map
}

fn can_move(px: i32, py: i32, col: &[bool]) -> bool {
    for dy in 0..CORR_PX as i32 {
        for dx in 0..CORR_PX as i32 {
            let x = (px + dx).rem_euclid(MAZE_W as i32) as usize;
            let y = (py + dy).rem_euclid(MAZE_H as i32) as usize;
            if col[y * MAZE_W + x] { return false; }
        }
    }
    true
}

fn player_cell(px: i32, py: i32) -> (usize, usize) {
    let x = (px + CORR_PX as i32 / 2).rem_euclid(MAZE_W as i32) as usize;
    let y = (py + CORR_PX as i32 / 2).rem_euclid(MAZE_H as i32) as usize;
    (x / CELL, y / CELL)
}

fn cell_pixel(cx: usize, cy: usize) -> (i32, i32) {
    ((cx * CELL + WALL_PX) as i32, (cy * CELL + WALL_PX) as i32)
}

// ---------------------------------------------------------------------------
// XInput FFI
// ---------------------------------------------------------------------------

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    _left_trigger: u8,
    _right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    _thumb_rx: i16,
    _thumb_ry: i16,
}

#[repr(C)]
struct XInputState {
    _packet_number: u32,
    gamepad: XInputGamepad,
}

const DPAD_UP: u16 = 0x0001;
const DPAD_DOWN: u16 = 0x0002;
const DPAD_LEFT: u16 = 0x0004;
const DPAD_RIGHT: u16 = 0x0008;

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

fn load_xinput() -> Option<XInputGetStateFn> {
    use std::ffi::CString;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut std::ffi::c_void;
        fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
    }
    for dll in &[b"xinput1_4.dll\0" as &[u8], b"xinput9_1_0.dll\0"] {
        let module = unsafe { LoadLibraryA(dll.as_ptr()) };
        if module.is_null() { continue; }
        let proc_name = CString::new("XInputGetState").unwrap();
        let proc = unsafe { GetProcAddress(module, proc_name.as_ptr() as *const u8) };
        if !proc.is_null() {
            return Some(unsafe { std::mem::transmute(proc) });
        }
    }
    None
}

fn normalize_thumb(value: i16) -> f32 {
    if value >= 0 { value as f32 / 32767.0 } else { value as f32 / 32768.0 }
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
struct PadState {
    dpad_up: bool,
    dpad_down: bool,
    dpad_left: bool,
    dpad_right: bool,
    left_stick: Vec2,
}

#[derive(Resource)]
struct MazeImage(Handle<Image>);

#[derive(Resource)]
struct KeySheetHandle(Handle<Image>);

#[derive(Resource, Default)]
struct KeySheetPixels(Option<Vec<u8>>);

#[derive(Resource)]
struct GameState {
    px: i32,
    py: i32,
    keys: [bool; NUM_KEYS],
    won: bool,
    move_timer: f32,
    last_dir: (i32, i32),
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let seed: u64 = rand::random();
    let layout = generate_maze(seed);
    let (sx, sy) = cell_pixel(layout.start.0, layout.start.1);
    commands.insert_resource(layout);
    commands.insert_resource(GameState {
        px: sx, py: sy, keys: [false; NUM_KEYS], won: false, move_timer: 0.0, last_dir: (0, 0),
    });

    let pixels = vec![0u8; IMG_W * IMG_H * 4];
    let image = Image::new(
        Extent3d { width: IMG_W as u32, height: IMG_H as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let handle = images.add(image);
    commands.insert_resource(MazeImage(handle.clone()));
    commands.spawn(Sprite { image: handle, ..default() });
    commands.spawn(Camera2d);

    let sheet: Handle<Image> = asset_server.load("external/64-bit/keys/Fantasy Keys-sheet.png");
    commands.insert_resource(KeySheetHandle(sheet));
}

fn cache_key_sheet(
    sheet_handle: Res<KeySheetHandle>,
    images: Res<Assets<Image>>,
    mut cached: ResMut<KeySheetPixels>,
) {
    if cached.0.is_some() { return; }
    if let Some(img) = images.get(&sheet_handle.0) {
        cached.0 = img.data.clone();
    }
}

fn auto_scale(
    windows: Query<&Window>,
    mut proj_q: Query<&mut Projection, With<Camera2d>>,
) {
    let Ok(win) = windows.single() else { return };
    let Ok(mut proj) = proj_q.single_mut() else { return };
    let Projection::Orthographic(ref mut ortho) = *proj else { return };
    let zoom = (win.width() / IMG_W as f32).min(win.height() / IMG_H as f32).floor().max(1.0);
    let new_scale = 1.0 / zoom;
    if (ortho.scale - new_scale).abs() > f32::EPSILON { ortho.scale = new_scale; }
}

fn read_gamepad(
    mut state: ResMut<PadState>,
    mut xinput_fn: Local<Option<Option<XInputGetStateFn>>>,
) {
    let get_state = match *xinput_fn {
        Some(Some(f)) => f,
        Some(None) => return,
        None => {
            let loaded = load_xinput();
            *xinput_fn = Some(loaded);
            match loaded { Some(f) => f, None => return }
        }
    };
    let mut xs = std::mem::MaybeUninit::<XInputState>::uninit();
    let result = unsafe { get_state(0, xs.as_mut_ptr()) };
    if result != 0 { *state = PadState::default(); return; }
    let xs = unsafe { xs.assume_init() };
    let gp = &xs.gamepad;
    let btn = |mask: u16| gp.buttons & mask != 0;
    state.dpad_up = btn(DPAD_UP);
    state.dpad_down = btn(DPAD_DOWN);
    state.dpad_left = btn(DPAD_LEFT);
    state.dpad_right = btn(DPAD_RIGHT);
    state.left_stick = Vec2::new(normalize_thumb(gp.thumb_lx), normalize_thumb(gp.thumb_ly));
}

fn movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    pad: Res<PadState>,
    layout: Res<MazeLayout>,
    mut gs: ResMut<GameState>,
) {
    if gs.won { return; }

    let mut dx: i32 = 0;
    let mut dy: i32 = 0;
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) { dx += 1; }
    if keyboard.pressed(KeyCode::ArrowLeft)  || keyboard.pressed(KeyCode::KeyA) { dx -= 1; }
    if keyboard.pressed(KeyCode::ArrowDown)  || keyboard.pressed(KeyCode::KeyS) { dy += 1; }
    if keyboard.pressed(KeyCode::ArrowUp)    || keyboard.pressed(KeyCode::KeyW) { dy -= 1; }
    if pad.dpad_right { dx += 1; }
    if pad.dpad_left  { dx -= 1; }
    if pad.dpad_down  { dy += 1; }
    if pad.dpad_up    { dy -= 1; }
    if pad.left_stick.x >  STICK_DEAD { dx += 1; }
    if pad.left_stick.x < -STICK_DEAD { dx -= 1; }
    if pad.left_stick.y >  STICK_DEAD { dy -= 1; }
    if pad.left_stick.y < -STICK_DEAD { dy += 1; }
    dx = dx.clamp(-1, 1);
    dy = dy.clamp(-1, 1);

    if dx == 0 && dy == 0 { gs.move_timer = 0.0; gs.last_dir = (0, 0); return; }

    let col = build_collision(&layout, &gs.keys);
    gs.move_timer += time.delta_secs();
    let interval = 1.0 / MOVE_SPEED;
    while gs.move_timer >= interval {
        gs.move_timer -= interval;

        let x_blocked = dx != 0 && !can_move(gs.px + dx, gs.py, &col);
        let y_blocked = dy != 0 && !can_move(gs.px, gs.py + dy, &col);

        let mut eff_x = if x_blocked { 0 } else { dx };
        let mut eff_y = if y_blocked { 0 } else { dy };

        // Corner assist: when one axis is blocked, check if nudging the
        // perpendicular position by up to WALL_PX/2 pixels would unblock it.
        // If so, move toward alignment instead of along the unblocked axis.
        // This prevents skipping over corridor openings when both directions
        // are held (e.g., sliding left along a wall while holding down).
        let nudge = (WALL_PX as i32) / 2;
        if y_blocked && dx != 0 {
            // Want to go dy but blocked — check if nearby x alignment unblocks
            let mut best = None;
            for offset in 1..=nudge {
                for &sign in &[-1i32, 1] {
                    let test_x = gs.px + sign * offset;
                    if can_move(test_x, gs.py + dy, &col) {
                        best = Some(sign);
                        break;
                    }
                }
                if best.is_some() { break; }
            }
            if let Some(nudge_dir) = best {
                eff_x = nudge_dir;
                eff_y = 0; // nudge toward alignment first, then dy unblocks next tick
            }
        }
        if x_blocked && dy != 0 {
            // Want to go dx but blocked — check if nearby y alignment unblocks
            let mut best = None;
            for offset in 1..=nudge {
                for &sign in &[-1i32, 1] {
                    let test_y = gs.py + sign * offset;
                    if can_move(gs.px + dx, test_y, &col) {
                        best = Some(sign);
                        break;
                    }
                }
                if best.is_some() { break; }
            }
            if let Some(nudge_dir) = best {
                eff_y = nudge_dir;
                eff_x = 0;
            }
        }

        // Original corner assist: if one axis is blocked and the other has
        // no input, continue on the unblocked axis using remembered direction.
        if x_blocked && eff_y == 0 && gs.last_dir.1 != 0 {
            eff_y = gs.last_dir.1;
        }
        if y_blocked && eff_x == 0 && gs.last_dir.0 != 0 {
            eff_x = gs.last_dir.0;
        }

        if eff_x != 0 && can_move(gs.px + eff_x, gs.py, &col) {
            gs.px = (gs.px + eff_x).rem_euclid(MAZE_W as i32);
            gs.last_dir = (eff_x, gs.last_dir.1);
        }
        if eff_y != 0 && can_move(gs.px, gs.py + eff_y, &col) {
            gs.py = (gs.py + eff_y).rem_euclid(MAZE_H as i32);
            gs.last_dir = (gs.last_dir.0, eff_y);
        }
    }

    // Pickups
    let (cx, cy) = player_cell(gs.px, gs.py);
    for (ki, &(kcx, kcy)) in layout.key_cells.iter().enumerate() {
        if cx == kcx && cy == kcy && !gs.keys[ki] {
            gs.keys[ki] = true;
        }
    }
    if (cx, cy) == layout.goal {
        gs.won = true;
    }
}

fn restart_or_new(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut layout: ResMut<MazeLayout>,
    mut gs: ResMut<GameState>,
) {
    let new_maze = keyboard.just_pressed(KeyCode::KeyN);
    let restart = keyboard.just_pressed(KeyCode::KeyR);

    if new_maze {
        let seed: u64 = rand::random();
        *layout = generate_maze(seed);
    }

    if new_maze || restart {
        let (sx, sy) = cell_pixel(layout.start.0, layout.start.1);
        *gs = GameState {
            px: sx, py: sy, keys: [false; NUM_KEYS], won: false, move_timer: 0.0, last_dir: (0, 0),
        };
    }
}

fn blit_key_sprite(
    dst: &mut Vec<u8>, dst_w: usize,
    sheet: &[u8],
    color_idx: usize, design: usize,
    dest_x: usize, dest_y: usize,
) {
    let (sc, sr) = key_sheet_pos(color_idx, design);
    let sx0 = sc * KEY_SPRITE_SIZE;
    let sy0 = sr * KEY_SPRITE_SIZE;
    for y in 0..KEY_SPRITE_SIZE {
        for x in 0..KEY_SPRITE_SIZE {
            let si = ((sy0 + y) * SHEET_W + (sx0 + x)) * 4;
            if si + 3 >= sheet.len() { continue; }
            if sheet[si + 3] == 0 { continue; }
            let dx = dest_x + x;
            let dy = dest_y + y;
            if dx >= dst_w || dy * dst_w + dx >= dst.len() / 4 { continue; }
            let di = (dy * dst_w + dx) * 4;
            dst[di..di + 4].copy_from_slice(&sheet[si..si + 4]);
        }
    }
}

fn render(
    gs: Res<GameState>,
    layout: Res<MazeLayout>,
    maze_img: Res<MazeImage>,
    mut images: ResMut<Assets<Image>>,
    sheet_pixels: Res<KeySheetPixels>,
) {
    let Some(image) = images.get_mut(&maze_img.0) else { return };
    let data = image.data.as_mut().unwrap();

    let set = |data: &mut Vec<u8>, x: usize, y: usize, c: [u8; 4]| {
        let i = (y * IMG_W + x) * 4;
        data[i..i + 4].copy_from_slice(&c);
    };

    let fill = |data: &mut Vec<u8>, x0: usize, y0: usize, w: usize, h: usize, c: [u8; 4]| {
        for y in y0..y0 + h { for x in x0..x0 + w { set(data, x, y, c); } }
    };

    // Clear
    for y in 0..IMG_H { for x in 0..IMG_W { set(data, x, y, COL_BG); } }

    // Walls
    let collision = build_collision(&layout, &gs.keys);
    for y in 0..MAZE_H { for x in 0..MAZE_W {
        if collision[y * MAZE_W + x] { set(data, x, y, COL_WALL); }
    }}

    // Gates (colored when locked)
    for &(gcx, gcy, horiz, ki) in &layout.gates {
        if gs.keys[ki] { continue; }
        let col = KEY_COLORS[KEY_COLOR_IDX[ki]];
        if horiz {
            let py0 = (gcy + 1) * CELL % MAZE_H;
            let px0 = gcx * CELL + WALL_PX;
            fill(data, px0, py0, CORR_PX, WALL_PX, col);
        } else {
            let px0 = (gcx + 1) * CELL % MAZE_W;
            let py0 = gcy * CELL + WALL_PX;
            fill(data, px0, py0, WALL_PX, CORR_PX, col);
        }
    }

    // Goal (centered in cell)
    let item_off = (CORR_PX - ITEM_SIZE) / 2;
    {
        let (gx, gy) = layout.goal;
        let x0 = gx * CELL + WALL_PX + item_off;
        let y0 = gy * CELL + WALL_PX + item_off;
        fill(data, x0, y0, ITEM_SIZE, ITEM_SIZE, COL_GOAL);
    }

    // Keys (sprite or fallback)
    for (ki, &(kcx, kcy)) in layout.key_cells.iter().enumerate() {
        if gs.keys[ki] { continue; }
        let dx = kcx * CELL + WALL_PX;
        let dy = kcy * CELL + WALL_PX;
        if let Some(sheet) = sheet_pixels.0.as_ref() {
            blit_key_sprite(data, IMG_W, sheet, KEY_COLOR_IDX[ki], layout.key_designs[ki], dx, dy);
        } else {
            let off = (CORR_PX - ITEM_SIZE) / 2;
            fill(data, dx + off, dy + off, ITEM_SIZE, ITEM_SIZE, KEY_COLORS[KEY_COLOR_IDX[ki]]);
        }
    }

    // Player (CORR_PX × CORR_PX, wrapping)
    let pc = if gs.won { COL_WIN } else { COL_PLAYER };
    for dy in 0..CORR_PX { for dx in 0..CORR_PX {
        let x = (gs.px as usize + dx) % MAZE_W;
        let y = (gs.py as usize + dy) % MAZE_H;
        set(data, x, y, pc);
    }}

    // HUD separator line (2px thick)
    for y in MAZE_H..MAZE_H + 2 { for x in 0..IMG_W { set(data, x, y, COL_WALL); } }

    // HUD: key indicators
    let gap = WALL_PX;
    let total_w = NUM_KEYS * KEY_SPRITE_SIZE + (NUM_KEYS - 1) * gap;
    let start_x = (IMG_W - total_w) / 2;
    let hud_y = MAZE_H + (HUD_H - KEY_SPRITE_SIZE) / 2;
    for i in 0..NUM_KEYS {
        let kx = start_x + i * (KEY_SPRITE_SIZE + gap);
        if gs.keys[i] {
            if let Some(sheet) = sheet_pixels.0.as_ref() {
                blit_key_sprite(data, IMG_W, sheet, KEY_COLOR_IDX[i], layout.key_designs[i], kx, hud_y);
            } else {
                let off = (KEY_SPRITE_SIZE - ITEM_SIZE) / 2;
                fill(data, kx + off, hud_y + off, ITEM_SIZE, ITEM_SIZE, KEY_COLORS[KEY_COLOR_IDX[i]]);
            }
        } else {
            let off = (KEY_SPRITE_SIZE - ITEM_SIZE) / 2;
            fill(data, kx + off, hud_y + off, ITEM_SIZE, ITEM_SIZE, COL_DIM);
        }
    }
}

fn update_title(
    mut windows: Query<&mut Window>,
    gs: Res<GameState>,
    layout: Res<MazeLayout>,
) {
    let Ok(mut win) = windows.single_mut() else { return };
    let status = if gs.won { "WIN! (N=new R=restart)" } else { "maze (N=new R=restart)" };
    let mut k = String::from("keys: [");
    for i in 0..NUM_KEYS {
        if i > 0 { k.push(' '); }
        k.push(if gs.keys[i] { char::from(b'1' + i as u8) } else { '.' });
    }
    k.push(']');
    win.title = format!("{status} | {k} | seed: {}", layout.seed);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .init_resource::<PadState>()
        .init_resource::<KeySheetPixels>()
        .add_systems(Startup, setup)
        .add_systems(Update, (
            read_gamepad,
            restart_or_new,
            movement,
            cache_key_sheet,
            auto_scale,
            render,
            update_title,
        ).chain())
        .run();
}

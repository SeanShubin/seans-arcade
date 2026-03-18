//! Procedural 47-blob autotile texture generator.
//!
//! Generates tileset sheets (512x384, 8x6 grid of 64x64 tiles) matching the
//! same layout as the existing wall/floor assets.
//!
//! Usage:
//!   cargo run --example generate_textures
//!
//! Each "material" is defined by a small set of parameters:
//!   - base color + variation
//!   - noise type (perlin, cellular, speckle, stripe)
//!   - bevel style (width, depth, light direction)
//!
//! The generator composes these to produce visually distinct tilesets.

use image::{Rgb, RgbImage};
use noise::{NoiseFn, Perlin, OpenSimplex};
use std::path::Path;

const TILE_SIZE: u32 = 64;
const COLS: u32 = 8;
const ROWS: u32 = 6;
const SHEET_W: u32 = COLS * TILE_SIZE;
const SHEET_H: u32 = ROWS * TILE_SIZE;

// The 47-blob tile layout: each entry is (pattern, grid_col, grid_row).
// Pattern: [NW, N, NE, W, center, E, SW, S, SE]
// Values: -1 = edge (not same type), 0 = don't care, 1 = same type
const BLOB_RULES: [([i8; 9], u32, u32); 47] = [
    ([0,-1,0,-1,1,-1,0,-1,0], 6, 3),   // isolated
    ([-1,1,-1,1,1,1,-1,1,-1], 7, 0),   // all corners exposed
    ([0,-1,0,-1,1,1,0,1,-1], 0, 0),    // top-left outer
    ([0,-1,0,1,1,-1,-1,1,0], 1, 0),    // top-right outer
    ([-1,1,0,1,1,-1,0,-1,0], 1, 1),    // bottom-right outer
    ([0,1,-1,-1,1,1,0,-1,0], 0, 1),    // bottom-left outer
    ([-1,1,1,1,1,1,1,1,-1], 6, 4),     // NW+SE inner corners
    ([1,1,-1,1,1,1,-1,1,1], 6, 5),     // NE+SW inner corners
    ([0,1,-1,-1,1,1,0,1,1], 2, 4),
    ([0,-1,0,1,1,1,-1,1,1], 4, 4),
    ([-1,1,-1,1,1,1,-1,1,1], 0, 4),
    ([-1,1,0,1,1,-1,1,1,0], 3, 4),
    ([0,-1,0,1,1,1,1,1,-1], 5, 4),
    ([-1,1,-1,1,1,1,1,1,-1], 1, 4),
    ([1,1,-1,1,1,1,0,-1,0], 5, 5),
    ([1,1,0,1,1,-1,-1,1,0], 3, 5),
    ([1,1,-1,1,1,1,-1,1,-1], 1, 5),
    ([0,1,1,-1,1,1,0,1,-1], 2, 5),
    ([-1,1,1,1,1,1,0,-1,0], 4, 5),
    ([-1,1,1,1,1,1,-1,1,-1], 0, 5),
    ([-1,1,-1,1,1,1,0,-1,0], 4, 0),   // top T
    ([0,1,-1,-1,1,1,0,1,-1], 3, 1),   // right T
    ([0,-1,0,1,1,1,-1,1,-1], 3, 0),   // bottom T
    ([-1,1,0,1,1,-1,-1,1,0], 4, 1),   // left T
    ([-1,1,1,1,1,1,-1,1,1], 3, 3),    // NW inner only
    ([1,1,-1,1,1,1,1,1,-1], 2, 3),    // NE inner only
    ([1,1,1,1,1,1,-1,1,-1], 2, 2),    // SW inner only
    ([-1,1,-1,1,1,1,1,1,1], 3, 2),    // SE inner only
    ([0,-1,0,-1,1,0,0,-1,0], 6, 1),   // west cap
    ([0,-1,0,0,1,-1,0,-1,0], 5, 0),   // east cap
    ([0,-1,0,0,1,0,0,-1,0], 2, 1),    // vertical corridor
    ([0,-1,0,-1,1,-1,0,0,0], 5, 1),   // north cap
    ([0,0,0,-1,1,-1,0,-1,0], 6, 0),   // south cap
    ([0,0,0,-1,1,-1,0,0,0], 2, 0),    // horizontal corridor
    ([0,-1,0,-1,1,0,0,0,0], 7, 1),    // NW corner cap
    ([0,-1,0,0,1,-1,0,0,0], 7, 3),    // NE corner cap
    ([0,0,0,0,1,-1,0,-1,0], 7, 4),    // SE corner cap
    ([0,0,0,-1,1,0,0,-1,0], 7, 2),    // SW corner cap
    ([0,-1,0,0,1,0,0,0,0], 4, 3),     // north end
    ([0,0,0,0,1,-1,0,0,0], 5, 2),     // east end
    ([0,0,0,0,1,0,0,-1,0], 5, 3),     // south end
    ([0,0,0,-1,1,0,0,0,0], 4, 2),     // west end
    ([-1,1,0,1,1,0,0,0,0], 1, 3),     // NW diagonal
    ([0,1,-1,0,1,1,0,0,0], 0, 3),     // NE diagonal
    ([0,0,0,0,1,1,0,1,-1], 0, 2),     // SE diagonal
    ([0,0,0,1,1,0,-1,1,0], 1, 2),     // SW diagonal
    ([0,0,0,0,1,0,0,0,0], 6, 2),      // fallback (center only)
];

/// Defines the visual appearance of a material.
#[derive(Clone)]
struct Material {
    name: &'static str,
    base_color: [f64; 3],          // RGB 0.0-1.0
    color_variation: f64,          // how much noise affects color
    noise_scale: f64,              // spatial frequency of noise
    noise_octaves: u32,            // FBM octaves (1 = smooth, 4+ = detailed)
    pattern: PatternType,          // which noise function drives the look
    bevel_width: f64,              // edge bevel in pixels
    bevel_depth: f64,              // how dark/light the bevel gets (0.0-1.0)
    light_angle: f64,              // radians, where light comes from
    speckle_density: f64,          // 0.0-1.0, fraction of pixels that are speckled
    speckle_color: [f64; 3],       // color of speckles
    secondary_color: Option<[f64; 3]>, // for two-tone patterns
}

#[derive(Clone)]
enum PatternType {
    Perlin,             // smooth bumpy surface
    Cellular,           // voronoi-style cells
    Speckle,            // random dots on solid
    Brick,              // brick-like grid pattern
    Stripe { angle: f64 }, // directional lines
    Marble,             // domain-warped perlin
}

fn main() {
    let materials = vec![
        Material {
            name: "concrete",
            base_color: [0.62, 0.62, 0.62],
            color_variation: 0.06,
            noise_scale: 0.08,
            noise_octaves: 3,
            pattern: PatternType::Perlin,
            bevel_width: 11.0,
            bevel_depth: 0.5,
            light_angle: std::f64::consts::FRAC_PI_4 * 3.0,
            speckle_density: 0.0,
            speckle_color: [0.0; 3],
            secondary_color: None,
        },
        Material {
            name: "red_brick",
            base_color: [0.6, 0.25, 0.18],
            color_variation: 0.06,
            noise_scale: 0.1,
            noise_octaves: 2,
            pattern: PatternType::Brick,
            bevel_width: 4.0,
            bevel_depth: 0.4,
            light_angle: std::f64::consts::FRAC_PI_4 * 3.0,
            speckle_density: 0.08,
            speckle_color: [0.85, 0.85, 0.8],
            secondary_color: Some([0.78, 0.75, 0.65]), // mortar color
        },
        Material {
            name: "dark_stone",
            base_color: [0.3, 0.3, 0.32],
            color_variation: 0.1,
            noise_scale: 0.04,
            noise_octaves: 4,
            pattern: PatternType::Cellular,
            bevel_width: 6.0,
            bevel_depth: 0.45,
            light_angle: std::f64::consts::FRAC_PI_4 * 3.0,
            speckle_density: 0.0,
            speckle_color: [0.0; 3],
            secondary_color: Some([0.18, 0.18, 0.2]),
        },
        Material {
            name: "marble",
            base_color: [0.88, 0.86, 0.82],
            color_variation: 0.15,
            noise_scale: 0.03,
            noise_octaves: 4,
            pattern: PatternType::Marble,
            bevel_width: 4.0,
            bevel_depth: 0.3,
            light_angle: std::f64::consts::FRAC_PI_4 * 3.0,
            speckle_density: 0.0,
            speckle_color: [0.0; 3],
            secondary_color: Some([0.4, 0.35, 0.3]),
        },
        Material {
            name: "wood_plank",
            base_color: [0.55, 0.38, 0.22],
            color_variation: 0.12,
            noise_scale: 0.06,
            noise_octaves: 3,
            pattern: PatternType::Stripe { angle: 0.0 },
            bevel_width: 5.0,
            bevel_depth: 0.35,
            light_angle: std::f64::consts::FRAC_PI_4 * 3.0,
            speckle_density: 0.0,
            speckle_color: [0.0; 3],
            secondary_color: Some([0.42, 0.28, 0.15]),
        },
        Material {
            name: "blue_tile",
            base_color: [0.2, 0.35, 0.6],
            color_variation: 0.03,
            noise_scale: 0.15,
            noise_octaves: 1,
            pattern: PatternType::Perlin,
            bevel_width: 4.0,
            bevel_depth: 0.5,
            light_angle: std::f64::consts::FRAC_PI_4 * 3.0,
            speckle_density: 0.0,
            speckle_color: [0.0; 3],
            secondary_color: None,
        },
        Material {
            name: "sandstone",
            base_color: [0.72, 0.62, 0.45],
            color_variation: 0.1,
            noise_scale: 0.07,
            noise_octaves: 3,
            pattern: PatternType::Perlin,
            bevel_width: 5.0,
            bevel_depth: 0.35,
            light_angle: std::f64::consts::FRAC_PI_4 * 3.0,
            speckle_density: 0.04,
            speckle_color: [0.85, 0.78, 0.6],
            secondary_color: None,
        },
        Material {
            name: "metal_plate",
            base_color: [0.5, 0.52, 0.55],
            color_variation: 0.02,
            noise_scale: 0.2,
            noise_octaves: 1,
            pattern: PatternType::Perlin,
            bevel_width: 3.0,
            bevel_depth: 0.55,
            light_angle: std::f64::consts::FRAC_PI_4 * 3.0,
            speckle_density: 0.02,
            speckle_color: [0.7, 0.72, 0.75],
            secondary_color: None,
        },
    ];

    let out_dir = Path::new("assets/generated/wall");
    std::fs::create_dir_all(out_dir).expect("Failed to create output directory");

    for mat in &materials {
        let img = generate_tileset(mat);
        let path = out_dir.join(format!("Wall-{}-64x64.png", mat.name));
        img.save(&path).expect("Failed to save image");
        println!("Generated: {}", path.display());
    }

    println!("\nDone! Generated {} tilesets.", materials.len());
}

fn generate_tileset(mat: &Material) -> RgbImage {
    let mut img = RgbImage::new(SHEET_W, SHEET_H);

    // Fill with magenta (marks unused slots)
    for pixel in img.pixels_mut() {
        *pixel = Rgb([255, 0, 255]);
    }

    let perlin = Perlin::new(42);
    let simplex = OpenSimplex::new(123);

    for &(pattern, col, row) in &BLOB_RULES {
        let tile_x = col * TILE_SIZE;
        let tile_y = row * TILE_SIZE;

        // Determine which edges are exposed from the pattern
        // Pattern: [NW, N, NE, W, center, E, SW, S, SE]
        let edges = Edges {
            n:  pattern[1] == -1,
            s:  pattern[7] == -1,
            w:  pattern[3] == -1,
            e:  pattern[5] == -1,
            nw: pattern[0] == -1 || pattern[1] == -1 || pattern[3] == -1,
            ne: pattern[2] == -1 || pattern[1] == -1 || pattern[5] == -1,
            sw: pattern[6] == -1 || pattern[7] == -1 || pattern[3] == -1,
            se: pattern[8] == -1 || pattern[7] == -1 || pattern[5] == -1,
            // Inner corners: diagonal is exposed but both adjacent cardinals are present
            inner_nw: pattern[0] == -1 && pattern[1] != -1 && pattern[3] != -1,
            inner_ne: pattern[2] == -1 && pattern[1] != -1 && pattern[5] != -1,
            inner_sw: pattern[6] == -1 && pattern[7] != -1 && pattern[3] != -1,
            inner_se: pattern[8] == -1 && pattern[7] != -1 && pattern[5] != -1,
        };

        for py in 0..TILE_SIZE {
            for px in 0..TILE_SIZE {
                let world_x = (tile_x + px) as f64;
                let world_y = (tile_y + py) as f64;
                let local_x = px as f64;
                let local_y = py as f64;

                // 1. Base color from noise pattern
                let base = sample_pattern(mat, &perlin, &simplex, world_x, world_y);

                // 2. Speckle overlay
                let speckled = apply_speckle(mat, &perlin, base, world_x, world_y);

                // 3. Bevel lighting
                let bevel = compute_bevel(&edges, local_x, local_y, mat);
                let final_color = apply_bevel(speckled, bevel, mat);

                img.put_pixel(
                    tile_x + px,
                    tile_y + py,
                    to_rgb(final_color),
                );
            }
        }
    }

    img
}

struct Edges {
    n: bool, s: bool, e: bool, w: bool,
    nw: bool, ne: bool, sw: bool, se: bool,
    inner_nw: bool, inner_ne: bool, inner_sw: bool, inner_se: bool,
}

/// Sample the base material pattern at a world position.
fn sample_pattern(
    mat: &Material,
    perlin: &Perlin,
    simplex: &OpenSimplex,
    x: f64,
    y: f64,
) -> [f64; 3] {
    let noise_val = match &mat.pattern {
        PatternType::Perlin => {
            fbm(perlin, x * mat.noise_scale, y * mat.noise_scale, mat.noise_octaves)
        }
        PatternType::Cellular => {
            cellular_noise(x * mat.noise_scale, y * mat.noise_scale)
        }
        PatternType::Speckle => {
            // Just use base color; speckles applied separately
            0.0
        }
        PatternType::Brick => {
            brick_pattern(x, y, mat, perlin)
        }
        PatternType::Stripe { angle } => {
            let rotated = x * angle.cos() + y * angle.sin();
            let stripe = fbm(perlin, rotated * mat.noise_scale * 3.0, 0.3, mat.noise_octaves);
            // Add cross-grain detail
            let detail = fbm(simplex, x * mat.noise_scale * 0.5, y * mat.noise_scale * 2.0, 2);
            stripe * 0.8 + detail * 0.2
        }
        PatternType::Marble => {
            // Domain warping: feed noise into itself
            let warp_x = fbm(perlin, x * mat.noise_scale, y * mat.noise_scale, 3) * 8.0;
            let warp_y = fbm(simplex, x * mat.noise_scale + 5.3, y * mat.noise_scale + 1.7, 3) * 8.0;
            let v = (x * mat.noise_scale + warp_x).sin() * 0.5 + 0.5;
            let detail = fbm(perlin, x * mat.noise_scale * 2.0 + warp_y, y * mat.noise_scale * 2.0, 2);
            v * 0.7 + detail * 0.3
        }
    };

    let var = noise_val * mat.color_variation;

    if let Some(secondary) = mat.secondary_color {
        // Blend between base and secondary based on pattern
        let t = (noise_val * 0.5 + 0.5).clamp(0.0, 1.0);
        [
            lerp(mat.base_color[0], secondary[0], t) + var,
            lerp(mat.base_color[1], secondary[1], t) + var,
            lerp(mat.base_color[2], secondary[2], t) + var,
        ]
    } else {
        [
            mat.base_color[0] + var,
            mat.base_color[1] + var,
            mat.base_color[2] + var,
        ]
    }
}

/// Brick pattern: returns a noise value that distinguishes brick from mortar.
fn brick_pattern(x: f64, y: f64, mat: &Material, perlin: &Perlin) -> f64 {
    let brick_w = 16.0;
    let brick_h = 8.0;
    let mortar = 1.0;

    let row = (y / brick_h).floor() as i32;
    let offset = if row % 2 == 0 { 0.0 } else { brick_w * 0.5 };
    let bx = ((x + offset) % brick_w) / brick_w;
    let by = (y % brick_h) / brick_h;

    let mortar_x = mortar / brick_w;
    let mortar_y = mortar / brick_h;

    if bx < mortar_x || bx > (1.0 - mortar_x) || by < mortar_y || by > (1.0 - mortar_y) {
        // Mortar
        -1.0
    } else {
        // Brick surface with per-brick variation
        let brick_id = (row as f64) * 100.0 + ((x + offset) / brick_w).floor();
        let base_variation = perlin.get([brick_id * 0.1, 0.0]) * 0.5;
        let surface = fbm(perlin, x * mat.noise_scale, y * mat.noise_scale, mat.noise_octaves);
        base_variation + surface * 0.5
    }
}

/// Apply speckle overlay to the base color.
fn apply_speckle(mat: &Material, perlin: &Perlin, base: [f64; 3], x: f64, y: f64) -> [f64; 3] {
    if mat.speckle_density <= 0.0 {
        return base;
    }

    // Use high-frequency noise as a hash for speckle placement
    let hash = perlin.get([x * 1.731, y * 2.399]);
    if hash > 1.0 - mat.speckle_density * 2.0 {
        mat.speckle_color
    } else {
        base
    }
}

/// Compute bevel factor for a pixel based on edge distances.
/// Returns a value where negative = shadow, positive = highlight.
fn compute_bevel(edges: &Edges, x: f64, y: f64, mat: &Material) -> f64 {
    let w = mat.bevel_width;
    let size = TILE_SIZE as f64;
    let light_x = mat.light_angle.cos();
    let light_y = -mat.light_angle.sin(); // negative because y-down

    let mut bevel = 0.0f64;

    // sqrt falloff: stays strong across most of bevel, drops at inner edge
    if edges.n && y < w {
        let t = (1.0 - y / w).sqrt();
        bevel += t * -light_y;
    }
    if edges.s && y > size - w {
        let t = (1.0 - (size - y) / w).sqrt();
        bevel += t * light_y;
    }
    if edges.w && x < w {
        let t = (1.0 - x / w).sqrt();
        bevel += t * -light_x;
    }
    if edges.e && x > size - w {
        let t = (1.0 - (size - x) / w).sqrt();
        bevel += t * light_x;
    }

    // Inner corners
    let corner_w = w * 1.2;
    if edges.inner_nw && x < corner_w && y < corner_w {
        let dist = (x * x + y * y).sqrt();
        if dist < corner_w {
            let t = (1.0 - dist / corner_w).sqrt();
            bevel += t * (-light_x - light_y) * 0.7;
        }
    }
    if edges.inner_ne && x > size - corner_w && y < corner_w {
        let dx = size - x;
        let dist = (dx * dx + y * y).sqrt();
        if dist < corner_w {
            let t = (1.0 - dist / corner_w).sqrt();
            bevel += t * (light_x - light_y) * 0.7;
        }
    }
    if edges.inner_sw && x < corner_w && y > size - corner_w {
        let dy = size - y;
        let dist = (x * x + dy * dy).sqrt();
        if dist < corner_w {
            let t = (1.0 - dist / corner_w).sqrt();
            bevel += t * (-light_x + light_y) * 0.7;
        }
    }
    if edges.inner_se && x > size - corner_w && y > size - corner_w {
        let dx = size - x;
        let dy = size - y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < corner_w {
            let t = (1.0 - dist / corner_w).sqrt();
            bevel += t * (light_x + light_y) * 0.7;
        }
    }

    bevel.clamp(-1.0, 1.0)
}

/// Apply bevel lighting to a color.
fn apply_bevel(color: [f64; 3], bevel: f64, mat: &Material) -> [f64; 3] {
    let factor = bevel * mat.bevel_depth;
    if factor > 0.0 {
        // Highlight: lighten toward white
        [
            lerp(color[0], 1.0, factor),
            lerp(color[1], 1.0, factor),
            lerp(color[2], 1.0, factor),
        ]
    } else {
        // Shadow: darken toward black
        let f = -factor;
        [
            lerp(color[0], 0.0, f),
            lerp(color[1], 0.0, f),
            lerp(color[2], 0.0, f),
        ]
    }
}

// --- Math primitives ---

fn fbm<N: NoiseFn<f64, 2>>(noise: &N, x: f64, y: f64, octaves: u32) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_amp = 0.0;

    for _ in 0..octaves {
        value += noise.get([x * frequency, y * frequency]) * amplitude;
        max_amp += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    value / max_amp
}

/// Simple cellular/voronoi noise using a grid of random points.
fn cellular_noise(x: f64, y: f64) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let mut min_dist = f64::MAX;
    let mut second_dist = f64::MAX;

    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx = ix + dx;
            let cy = iy + dy;
            // Deterministic pseudo-random point in each cell
            let px = cx as f64 + hash2d(cx, cy, 0) * 0.8 + 0.1;
            let py = cy as f64 + hash2d(cx, cy, 1) * 0.8 + 0.1;
            let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if dist < min_dist {
                second_dist = min_dist;
                min_dist = dist;
            } else if dist < second_dist {
                second_dist = dist;
            }
        }
    }

    // F2 - F1 gives cell boundaries
    (second_dist - min_dist).clamp(0.0, 1.0) * 2.0 - 1.0
}

/// Simple integer hash for deterministic pseudo-random values.
fn hash2d(x: i32, y: i32, seed: i32) -> f64 {
    let mut h = (x.wrapping_mul(374761393))
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(1274126177));
    h = (h ^ (h >> 13)).wrapping_mul(1103515245);
    h = h ^ (h >> 16);
    (h & 0x7FFFFFFF) as f64 / 0x7FFFFFFF as f64
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn to_rgb(color: [f64; 3]) -> Rgb<u8> {
    Rgb([
        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
    ])
}

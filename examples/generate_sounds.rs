//! Generates simple retro sound effects as WAV files for the neon_pong example.
//!
//! Run with: `cargo run --example generate_sounds`
//!
//! Creates:
//! - `assets/sounds/hit.wav`    — short high-pitched ping
//! - `assets/sounds/bounce.wav` — short mid-pitched blip
//! - `assets/sounds/score.wav`  — descending two-tone fanfare

use std::f32::consts::TAU;
use std::fs;
use std::path::Path;

fn main() {
    let sounds_dir = Path::new("assets/sounds");
    fs::create_dir_all(sounds_dir).expect("Failed to create assets/sounds directory");

    let sample_rate = 44100u32;

    // Hit: short high ping (880 Hz, 80ms, with fast decay)
    let hit_samples = generate_tone(sample_rate, 880.0, 0.08, true);
    write_wav(
        &sounds_dir.join("hit.wav"),
        sample_rate,
        &hit_samples,
    );

    // Bounce: short mid blip (440 Hz, 60ms, with fast decay)
    let bounce_samples = generate_tone(sample_rate, 440.0, 0.06, true);
    write_wav(
        &sounds_dir.join("bounce.wav"),
        sample_rate,
        &bounce_samples,
    );

    // Score: two-tone descending (660 Hz then 440 Hz, 100ms each)
    let score_high = generate_tone(sample_rate, 660.0, 0.1, false);
    let score_low = generate_tone(sample_rate, 440.0, 0.15, true);
    let mut score_samples = score_high;
    score_samples.extend_from_slice(&score_low);
    write_wav(
        &sounds_dir.join("score.wav"),
        sample_rate,
        &score_samples,
    );

    println!("Generated sound files in assets/sounds/");
    println!("  hit.wav    — 880 Hz ping");
    println!("  bounce.wav — 440 Hz blip");
    println!("  score.wav  — 660-440 Hz fanfare");
}

fn generate_tone(sample_rate: u32, frequency: f32, duration_secs: f32, decay: bool) -> Vec<i16> {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    let amplitude = 0.5_f32;

    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let wave = (t * frequency * TAU).sin();

            let envelope = if decay {
                let progress = i as f32 / num_samples as f32;
                1.0 - progress
            } else {
                let attack_samples = (sample_rate as f32 * 0.005) as usize;
                if i < attack_samples {
                    i as f32 / attack_samples as f32
                } else {
                    1.0
                }
            };

            let sample = wave * amplitude * envelope;
            (sample * i16::MAX as f32) as i16
        })
        .collect()
}

fn write_wav(path: &Path, sample_rate: u32, samples: &[i16]) {
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = (samples.len() * 2) as u32;
    let file_size = 36 + data_size;

    let mut bytes = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&file_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");

    // fmt chunk
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }

    fs::write(path, bytes).unwrap_or_else(|e| panic!("Failed to write {}: {e}", path.display()));
}

//! Experiment: render "Hello, Sean!" in Mathematical Fraktur.
//!
//! Mathematical Fraktur lives in the Unicode Mathematical Alphanumeric Symbols
//! block (U+1D504–U+1D551), with a few exceptions in Letterlike Symbols.
//! Bevy's built-in font may not have these glyphs, so this example tests
//! whether we need to bundle an external font.
//!
//! Run: `cargo run --example fraktur_hello`

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

/// Convert an ASCII character to its Mathematical Fraktur Unicode equivalent.
fn to_fraktur(c: char) -> char {
    match c {
        // Uppercase — most live at U+1D504, but C, H, I, R, Z are in Letterlike Symbols
        'A' => '𝔄',
        'B' => '𝔅',
        'C' => 'ℭ',
        'D' => '𝔇',
        'E' => '𝔈',
        'F' => '𝔉',
        'G' => '𝔊',
        'H' => 'ℌ',
        'I' => 'ℑ',
        'J' => '𝔍',
        'K' => '𝔎',
        'L' => '𝔏',
        'M' => '𝔐',
        'N' => '𝔑',
        'O' => '𝔒',
        'P' => '𝔓',
        'Q' => '𝔔',
        'R' => 'ℜ',
        'S' => '𝔖',
        'T' => '𝔗',
        'U' => '𝔘',
        'V' => '𝔙',
        'W' => '𝔚',
        'X' => '𝔛',
        'Y' => '𝔜',
        'Z' => 'ℨ',
        // Lowercase — contiguous at U+1D51E
        c @ 'a'..='z' => char::from_u32(0x1D51E + (c as u32 - 'a' as u32)).unwrap(),
        // Non-letters pass through unchanged
        other => other,
    }
}

fn to_fraktur_string(s: &str) -> String {
    s.chars().map(to_fraktur).collect()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    let fraktur_text = to_fraktur_string("Hello, Sean!");

    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_child((
            Text::new(&fraktur_text),
            TextFont::from_font_size(48.0),
            TextColor::WHITE,
        ));
}

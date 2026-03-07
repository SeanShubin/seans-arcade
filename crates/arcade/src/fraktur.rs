//! Mathematical Fraktur name rendering.
//!
//! Maps a-z to U+1D51E-U+1D537, A-Z via lookup table (five irregular uppercase:
//! C=U+212D, H=U+210C, I=U+2111, R=U+211C, Z=U+2128).

const UPPER_FRAKTUR: [char; 26] = [
    '\u{1D504}', // A
    '\u{1D505}', // B
    '\u{212D}',  // C (Letterlike Symbols)
    '\u{1D507}', // D
    '\u{1D508}', // E
    '\u{1D509}', // F
    '\u{1D50A}', // G
    '\u{210C}',  // H (Letterlike Symbols)
    '\u{2111}',  // I (Letterlike Symbols)
    '\u{1D50D}', // J
    '\u{1D50E}', // K
    '\u{1D50F}', // L
    '\u{1D510}', // M
    '\u{1D511}', // N
    '\u{1D512}', // O
    '\u{1D513}', // P
    '\u{1D514}', // Q
    '\u{211C}',  // R (Letterlike Symbols)
    '\u{1D516}', // S
    '\u{1D517}', // T
    '\u{1D518}', // U
    '\u{1D519}', // V
    '\u{1D51A}', // W
    '\u{1D51B}', // X
    '\u{1D51C}', // Y
    '\u{2128}',  // Z (Letterlike Symbols)
];

pub fn to_fraktur(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' => {
                let offset = c as u32 - 'a' as u32;
                char::from_u32(0x1D51E + offset).unwrap_or(c)
            }
            'A'..='Z' => {
                let index = (c as u32 - 'A' as u32) as usize;
                UPPER_FRAKTUR[index]
            }
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase_mapping() {
        assert_eq!(to_fraktur("abc"), "\u{1D51E}\u{1D51F}\u{1D520}");
    }

    #[test]
    fn uppercase_irregular() {
        assert_eq!(to_fraktur("CHIRZ"), "\u{212D}\u{210C}\u{2111}\u{211C}\u{2128}");
    }

    #[test]
    fn mixed_case() {
        let result = to_fraktur("Alice");
        assert!(result.starts_with('\u{1D504}')); // A
    }
}

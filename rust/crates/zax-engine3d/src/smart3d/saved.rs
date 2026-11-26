//! Xnes3D+ Smart 3D: what it learned about a game, kept between sessions (owner request).
//!
//! A learned pattern has a new number in every session, so a piece is remembered by its
//! pictures instead: the bit planes of its tiles. The application keeps the text of
//! [`Saved::to_text`] in a file per game and hands it back when the game starts again; each
//! piece it names is given its learned look the first time it is seen.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use crate::pattern::EnginePattern;

/// The first line of the text, so a file of another kind or version is ignored.
const HEADER: &str = "xnes3d-smart3d 1";

/// What is remembered of a game.
#[derive(Debug, Default, Clone)]
pub(super) struct Saved {
    pub sky: Option<u8>,
    pub floors: HashSet<u64>,
    pub status_bar: HashSet<u64>,
    pub decor: HashSet<u64>,
    /// For each band of the screen: pixels it scrolled, pixels the level scrolled.
    pub bands: HashMap<i32, (u32, u32)>,
}

impl Saved {
    /// True when nothing is remembered.
    pub fn is_empty(&self) -> bool {
        self.sky.is_none()
            && self.floors.is_empty()
            && self.status_bar.is_empty()
            && self.decor.is_empty()
            && self.bands.is_empty()
    }

    /// The text kept in the game's file, one fact per line.
    pub fn to_text(&self) -> String {
        let mut text = String::from(HEADER);
        text.push('\n');
        if let Some(sky) = self.sky {
            let _ = writeln!(text, "sky {sky}");
        }
        let mut write_keys = |name: &str, keys: &HashSet<u64>| {
            let mut keys: Vec<_> = keys.iter().copied().collect();
            keys.sort_unstable();
            for key in keys {
                let _ = writeln!(text, "{name} {key:016x}");
            }
        };
        write_keys("floor", &self.floors);
        write_keys("status", &self.status_bar);
        write_keys("decor", &self.decor);
        let mut bands: Vec<_> = self.bands.iter().collect();
        bands.sort_unstable_by_key(|(band, _)| **band);
        for (band, (moved, level)) in bands {
            let _ = writeln!(text, "band {band} {moved} {level}");
        }
        text
    }

    /// Reads the text of a game's file. A file of another kind gives nothing; a line it cannot
    /// read is skipped.
    pub fn from_text(text: &str) -> Self {
        let mut saved = Self::default();
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(HEADER) {
            return saved;
        }
        for line in lines {
            let mut words = line.split_whitespace();
            let name = words.next().unwrap_or_default();
            let key = |word: Option<&str>| word.and_then(|word| u64::from_str_radix(word, 16).ok());
            match name {
                "sky" => saved.sky = words.next().and_then(|word| word.parse().ok()),
                "floor" => saved.floors.extend(key(words.next())),
                "status" => saved.status_bar.extend(key(words.next())),
                "decor" => saved.decor.extend(key(words.next())),
                "band" => {
                    let numbers: Vec<i64> = words.filter_map(|word| word.parse().ok()).collect();
                    if let [band, moved, level] = numbers[..] {
                        if let (Ok(band), Ok(moved), Ok(level)) = (
                            i32::try_from(band),
                            u32::try_from(moved),
                            u32::try_from(level),
                        ) {
                            saved.bands.insert(band, (moved, level));
                        }
                    }
                }
                _ => {}
            }
        }
        saved
    }
}

/// The key a pattern is remembered by: its tile pictures, the same from one session to the
/// next. FNV-1a, written out here so it never changes with the Rust version.
pub(super) fn pattern_key(pattern: &EnginePattern) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |value: u64| {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for image in pattern.images() {
        let key = image.key();
        eat(key.low);
        eat(key.high);
        eat(u64::from(key.flip_x) | (u64::from(key.flip_y) << 1));
    }
    hash
}

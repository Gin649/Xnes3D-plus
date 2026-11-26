//! Xnes3D+ Smart 3D: the scenery, read from how the game plays (owner request).
//!
//! - Floors: background pieces the characters keep standing on become solid, at full depth.
//! - Decoration: thin background pieces the characters often pass in front of but never stand
//!   on (windows, signs, plants) go a little further back.
//! - Parallax: a band of the screen that scrolls slower than the level (mountains, a skyline)
//!   goes back, the slower the further.
//! - Colour: thin scenery drawn only in dark or blue colours sits a little further back, as
//!   distant things look.
//!
//! What is decided about a piece is kept for the rest of the game, so it does not change from
//! one frame to the next.

use std::collections::{HashMap, HashSet};

use super::characters::Figure;
use crate::frame::{ConversionFrame, FrameTile};
use crate::registry::PatternId;

/// A background piece is a floor once characters stood on it in this many frames.
const FLOOR_FRAMES: u32 = 20;
/// A thin background piece is decoration once characters passed in front of it in this many
/// frames without ever standing on it.
const DECOR_FRAMES: u32 = 120;
/// How much of a character must cover a piece for it to count as passed in front of (pixels).
const DECOR_COVER: i32 = 16;
/// How far back decoration goes (pixels).
const DECOR_PUSH: f32 = 6.0;
/// The depth of thin scenery (Sparse and Medium in `apply`): only this is moved back by the
/// decoration and colour rules; solid ground and walls keep their place.
const THIN_DEPTH: i32 = 6;
/// How far back a band goes when it does not scroll at all with the level moving; a band that
/// scrolls at half the level's speed goes half as far.
const PARALLAX_PUSH: f32 = 32.0;
/// A band scrolling slower than this share of the level's speed is a distant layer...
const PARALLAX_SLOWER: f32 = 0.8;
/// ...but one that barely moves at all is a status bar or a still screen, not a layer.
const PARALLAX_STILL: f32 = 0.05;
/// How many pixels the level must have scrolled before a band's speed is trusted.
const PARALLAX_TRUST: u32 = 60;
/// Past this many pixels the measures are halved, so a band that changes speed later in the
/// game is followed.
const PARALLAX_FORGET: u32 = 600;
/// How far back dark or blue thin scenery goes (pixels).
const COLOR_PUSH: f32 = 4.0;

/// The speed of one band of the screen next to the level's.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Band {
    /// Pixels the band scrolled while the level scrolled `level` pixels.
    pub moved: u32,
    pub level: u32,
}

impl Band {
    /// The band's speed as a share of the level's, once enough is known.
    fn ratio(self) -> Option<f32> {
        #[allow(clippy::cast_precision_loss)] // small counts
        (self.level >= PARALLAX_TRUST).then(|| self.moved as f32 / self.level as f32)
    }
}

/// What is learned about the scenery from frame to frame.
#[derive(Debug, Default)]
pub(super) struct Scenery {
    walked: HashMap<PatternId, u32>,
    pub floors: HashSet<PatternId>,
    passed: HashMap<PatternId, u32>,
    pub decor: HashSet<PatternId>,
    pub bands: HashMap<i32, Band>,
    last_rows: HashMap<i32, i32>,
    last_level_x: Option<i32>,
}

/// The pattern the background tile was recognised as in this frame.
fn tile_pattern(frame: &ConversionFrame, tile: &FrameTile) -> Option<PatternId> {
    frame.shape(tile.shape()?).reference_pattern()
}

/// The band of the screen a background tile is in: its row of tiles.
const fn band_of(y: i32) -> i32 {
    y.div_euclid(8)
}

/// The step from one fine scroll (0 to 7) to the next, in pixels, either way.
const fn step(from: i32, to: i32) -> u32 {
    let forward = (to - from).rem_euclid(8);
    let back = 8 - forward;
    if forward <= back {
        forward.unsigned_abs()
    } else {
        back.unsigned_abs()
    }
}

impl Scenery {
    /// Watches where the characters stand and what they pass in front of. Returns the pieces
    /// that have just been found to be floors, so their 3D can be made solid once. `sky` is the
    /// sky's colour: the sky around a character is never taken for a floor.
    pub(super) fn watch(
        &mut self,
        frame: &ConversionFrame,
        figures: &[Figure],
        sky: Option<u8>,
    ) -> Vec<PatternId> {
        let mut stood: HashSet<PatternId> = HashSet::new();
        let mut covered: HashSet<PatternId> = HashSet::new();
        for figure in figures.iter().filter(|figure| figure.width() >= 6) {
            for tile in frame.background_tiles() {
                if sky.is_some_and(|sky| super::is_mostly_sky(frame, tile, sky)) {
                    continue;
                }
                let Some(pattern) = tile_pattern(frame, tile) else {
                    continue;
                };
                let left = tile.x().max(figure.left);
                let right = (tile.x() + 7).min(figure.right);
                let across = right - left + 1;
                // The tile's top is at the character's feet: it is stood on.
                if across >= 3 && (tile.y() - figure.bottom - 1).abs() <= 2 {
                    stood.insert(pattern);
                    continue;
                }
                let top = tile.y().max(figure.top);
                let bottom = (tile.y() + 7).min(figure.bottom);
                if across > 0 && bottom >= top && across * (bottom - top + 1) >= DECOR_COVER {
                    covered.insert(pattern);
                }
            }
        }
        let mut found = Vec::new();
        for pattern in stood {
            let count = self.walked.entry(pattern).or_insert(0);
            *count += 1;
            if *count >= FLOOR_FRAMES && self.floors.insert(pattern) {
                self.decor.remove(&pattern);
                found.push(pattern);
            }
        }
        for pattern in covered {
            if self.walked.contains_key(&pattern) {
                continue;
            }
            let count = self.passed.entry(pattern).or_insert(0);
            *count += 1;
            if *count >= DECOR_FRAMES {
                self.decor.insert(pattern);
            }
        }
        found
    }

    /// Measures how fast each band of the screen scrolls next to the level, from the fine
    /// horizontal scroll of its tiles. `level_x` is the level's own fine scroll.
    pub(super) fn watch_bands(&mut self, frame: &ConversionFrame, level_x: Option<i32>) {
        // The fine scroll most tiles of each band share.
        let mut counts: HashMap<i32, [u32; 8]> = HashMap::new();
        for tile in frame.background_tiles() {
            let fine = usize::try_from(tile.x().rem_euclid(8)).unwrap_or(0);
            if let Some(slot) = counts.entry(band_of(tile.y())).or_insert([0; 8]).get_mut(fine) {
                *slot += 1;
            }
        }
        let rows: HashMap<i32, i32> = counts
            .into_iter()
            .filter_map(|(band, counts)| {
                let (fine, _) = counts.iter().enumerate().max_by_key(|(_, count)| **count)?;
                Some((band, i32::try_from(fine).unwrap_or(0)))
            })
            .collect();
        if let (Some(before), Some(now)) = (self.last_level_x, level_x) {
            let level = step(before, now);
            if level > 0 {
                for (band, fine) in &rows {
                    let Some(&was) = self.last_rows.get(band) else {
                        continue;
                    };
                    let entry = self.bands.entry(*band).or_default();
                    entry.moved += step(was, *fine);
                    entry.level += level;
                    if entry.level > PARALLAX_FORGET {
                        entry.moved /= 2;
                        entry.level /= 2;
                    }
                }
            }
        }
        self.last_rows = rows;
        self.last_level_x = level_x;
    }

    /// Moves the scenery of this frame back as learned: distant bands, decoration, and thin
    /// pieces in dark or blue colours.
    pub(super) fn push_back(&self, frame: &mut ConversionFrame) {
        let mut pushes = Vec::new();
        for &id in frame.instances() {
            let instance = frame.instance(id);
            if !instance.background() {
                continue;
            }
            let data = frame.shape(instance.shape());
            // A status bar stays where it is (`apply`: one voxel deep, no shadow).
            if instance.depth <= 1 && !instance.cast_shadow {
                continue;
            }
            let mut push = 0.0;
            if let Some(ratio) = self
                .bands
                .get(&band_of(data.pixel_start().y))
                .and_then(|band| band.ratio())
            {
                if (PARALLAX_STILL..PARALLAX_SLOWER).contains(&ratio) {
                    push += PARALLAX_PUSH * (1.0 - ratio);
                }
            }
            if instance.depth <= THIN_DEPTH {
                let pattern = instance.reference().pattern;
                if self.decor.contains(&pattern) {
                    push += DECOR_PUSH;
                }
                if !self.floors.contains(&pattern) && looks_distant(frame, data.tiles()) {
                    push += COLOR_PUSH;
                }
            }
            if push > 0.0 {
                pushes.push((id, push));
            }
        }
        for (id, push) in pushes {
            frame.instance_mut(id).offset.z += push;
        }
    }
}

/// True when the colours the first tile of a piece is drawn with in this frame are all dark or
/// blue (black left out): distant scenery, night skies, shadows.
fn looks_distant(frame: &ConversionFrame, tiles: &[crate::frame::TileId]) -> bool {
    let Some(&first) = tiles.first() else {
        return false;
    };
    let tile = frame.tile(first);
    let mut any = false;
    for value in 1..=3 {
        let Some(color) = super::nes_color(frame, tile, value) else {
            continue;
        };
        let hue = color & 0x0F;
        let brightness = color >> 4;
        // The blacks and the greys of the last columns say nothing about distance.
        if hue >= 0x0D {
            continue;
        }
        let blue = (1..=3).contains(&hue);
        let dark = brightness == 0 && hue != 0;
        if !(blue || dark) {
            return false;
        }
        any = true;
    }
    any
}

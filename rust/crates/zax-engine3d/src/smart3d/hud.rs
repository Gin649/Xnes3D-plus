//! Xnes3D+ Smart 3D: status bars made of sprites (owner request).
//!
//! Many games draw their health, lives or score with sprites rather than background tiles
//! (Darkwing Duck's heart and gas gauge). Such a sprite stays at the same spot near the top or
//! the bottom of the screen while the level scrolls, and the same picture is never seen
//! anywhere else. Once one has done that long enough it is treated like a status bar: thin, no
//! shadow, and not brought forward with the characters.

use std::collections::{HashMap, HashSet};

use crate::frame::ConversionFrame;
use crate::registry::PatternId;

/// How many frames a sprite must stand still while the level scrolls before it counts as part
/// of a status bar: three seconds, so a player who stops for a moment is never taken for one.
const STILL_FRAMES: u32 = 180;

/// What is watched from frame to frame to find the status bar sprites.
#[derive(Debug, Default)]
pub(super) struct HudWatch {
    /// The fine scroll of the level in the last frame, to tell when it scrolls.
    last_playfield: Option<(i32, i32)>,
    /// Where each sprite picture was first seen.
    first_seen: HashMap<PatternId, (i32, i32)>,
    /// The sprite pictures seen in more than one place: characters, enemies, digits. Never a
    /// status bar piece.
    moved: HashSet<PatternId>,
    /// For each picture still a candidate, how many frames it stood still while the level
    /// scrolled.
    still: HashMap<PatternId, u32>,
    /// The sprite pictures found to be status bar pieces.
    pub(super) patterns: HashSet<PatternId>,
}

impl HudWatch {
    /// Looks at this frame's sprites. Returns the pictures that have just been found to be
    /// status bar pieces, so their 3D can be made thin once.
    pub(super) fn watch(
        &mut self,
        frame: &ConversionFrame,
        playfield: Option<(i32, i32)>,
    ) -> Vec<PatternId> {
        let scrolled = playfield.is_some()
            && self.last_playfield.is_some()
            && playfield != self.last_playfield;
        self.last_playfield = playfield;
        let mut found = Vec::new();
        for &id in frame.instances() {
            let instance = frame.instance(id);
            if instance.background() {
                continue;
            }
            let pattern = instance.reference().pattern;
            if self.patterns.contains(&pattern) || self.moved.contains(&pattern) {
                continue;
            }
            let data = frame.shape(instance.shape());
            let start = data.pixel_start();
            let place = (start.x, start.y);
            let first = *self.first_seen.entry(pattern).or_insert(place);
            if first != place {
                self.moved.insert(pattern);
                self.still.remove(&pattern);
                continue;
            }
            let in_band = start.y < super::STATUS_BAR_TOP_BAND
                || data.pixel_end().y >= super::STATUS_BAR_BOTTOM_BAND;
            if !in_band || !scrolled {
                continue;
            }
            let count = self.still.entry(pattern).or_insert(0);
            *count += 1;
            if *count >= STILL_FRAMES {
                self.still.remove(&pattern);
                self.patterns.insert(pattern);
                found.push(pattern);
            }
        }
        found
    }
}

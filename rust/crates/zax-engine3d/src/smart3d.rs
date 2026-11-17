//! Smart 3D (Zaxnes Plus): the settings a pattern learned with no profile gets from its shape,
//! instead of the same ones for every shape (SPEC-04 §4.3: 3 voxels deep, layer L1).
//!
//! It only acts while a game plays with the "3D by default" entry (no profile): a profile's own
//! patterns, and the shapes it learns, keep the defaults the profile expects. What it reads is
//! what the conversion already knows when a shape is learned:
//!
//! - a sprite stands out from the scenery, thicker than the background; one the game draws behind
//!   the background (doorways, bushes) sits further back and thinner;
//! - a background shape is placed by how filled in it is: mostly empty (distant details) thin,
//!   solid (ground, bricks, pipes) thick, the rest in between; a shape covering most of the
//!   screen is a backdrop, at the back;
//! - a status bar (score, lives, health) is kept as a thin flat panel with no shadow. It stays on
//!   the first layer like the rest: moving only the pieces recognised as the bar to the interface
//!   layer pulled them away from the pieces that were not (a digit that just changed), and
//!   scrambled the bar. It is told by the split scrolling NES games use for it: a band at the top or the
//!   bottom of the screen whose tiles do not scroll with the rest. A pattern first learned while
//!   the level stood still becomes a status bar the first frame its band is seen standing still
//!   while the level scrolls (`promote_status_bars`).
//!
//! The numbers are first values to be tuned on real games.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::engine::build_geometry;
use crate::frame::{ConversionFrame, ShapeId};
use crate::instance::PatternInstance;
use crate::settings::ZLayer;
use crate::segmenter::SegmentContext;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turns Smart 3D on for the game about to start (the "3D by default" entry) or off (2D Mode or a
/// profile).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// True while the game plays with Smart 3D.
#[must_use]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// What a shape is taken for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A sprite in front of the background.
    Sprite,
    /// A sprite the game draws behind the background.
    SpriteBehind,
    /// A mostly empty background shape.
    Sparse,
    /// A background shape neither sparse nor solid.
    Medium,
    /// A mostly filled background shape: ground, bricks, walls, pipes.
    Solid,
    /// A background shape covering most of the screen.
    Backdrop,
    /// A status bar: score, lives, health.
    StatusBar,
}

/// Below this share of drawn pixels a background shape is sparse.
const SPARSE_FILL: f32 = 0.25;
/// From this share of drawn pixels a background shape is solid.
const SOLID_FILL: f32 = 0.70;
/// A status bar is at most this many tiles tall...
const STATUS_BAR_MAX_ROWS: i32 = 6;
/// ...at least this many wide...
const STATUS_BAR_MIN_COLUMNS: i32 = 2;
/// ...and starts in the top band of the screen, or ends in the bottom one (pixels).
const STATUS_BAR_TOP_BAND: i32 = 64;
const STATUS_BAR_BOTTOM_BAND: i32 = 176;

/// The 3D settings of each kind.
pub fn apply(kind: Kind, instance: &mut PatternInstance) {
    let (depth, layer) = match kind {
        Kind::Sprite => (6, ZLayer::L1),
        Kind::SpriteBehind => (3, ZLayer::L2),
        // Thin but still in front: small shapes such as the letters of a word are each a shape,
        // and some letters are sparse and some not, so moving the sparse ones back would break
        // words apart. Only large backdrops go to the back layers.
        Kind::Sparse => (2, ZLayer::L1),
        Kind::Medium => (4, ZLayer::L1),
        Kind::Solid => (8, ZLayer::L1),
        Kind::Backdrop => (2, ZLayer::L3),
        Kind::StatusBar => (1, ZLayer::L1),
    };
    instance.depth = depth;
    instance.layer = layer;
    if kind == Kind::StatusBar {
        instance.cast_shadow = false;
        instance.receive_shadow = false;
    }
}

/// What `shape` of `frame` is taken for.
#[must_use]
pub fn classify(frame: &ConversionFrame, shape: ShapeId) -> Kind {
    let data = frame.shape(shape);
    if !data.background() {
        return if data.hidden() { Kind::SpriteBehind } else { Kind::Sprite };
    }
    if is_status_bar(frame, shape, playfield_offset(frame)) {
        return Kind::StatusBar;
    }
    if data.is_big() {
        return Kind::Backdrop;
    }
    let tiles = i32::try_from(data.tile_count()).unwrap_or(i32::MAX).max(1);
    #[allow(clippy::cast_precision_loss)] // at most 35 x 33 tiles of 64 pixels
    let fill = data.pixel_count() as f32 / (tiles * 64) as f32;
    if fill < SPARSE_FILL {
        Kind::Sparse
    } else if fill >= SOLID_FILL {
        Kind::Solid
    } else {
        Kind::Medium
    }
}

/// The fine scroll of the playfield in this frame: the position within its 8 x 8 cell (x mod 8,
/// y mod 8) most background tiles share. `None` with no background.
fn playfield_offset(frame: &ConversionFrame) -> Option<(i32, i32)> {
    let mut counts = [0u32; 64];
    let mut any = false;
    for tile in frame.background_tiles() {
        let fx = tile.x().rem_euclid(8);
        let fy = tile.y().rem_euclid(8);
        counts[usize::try_from(fy * 8 + fx).unwrap_or(0)] += 1;
        any = true;
    }
    if !any {
        return None;
    }
    let (best, _) = counts.iter().enumerate().max_by_key(|(_, count)| **count)?;
    let best = i32::try_from(best).unwrap_or(0);
    Some((best % 8, best / 8))
}

/// True when the background shape is a status bar: a short band at the top or the bottom of the
/// screen whose tiles sit on another fine scroll than the playfield (the split NES games use to
/// keep their status bar still while the level scrolls).
fn is_status_bar(frame: &ConversionFrame, shape: ShapeId, playfield: Option<(i32, i32)>) -> bool {
    let data = frame.shape(shape);
    if !data.background() {
        return false;
    }
    let size = data.tile_size();
    if size.y > STATUS_BAR_MAX_ROWS || size.x < STATUS_BAR_MIN_COLUMNS {
        return false;
    }
    let in_band = data.pixel_start().y < STATUS_BAR_TOP_BAND || data.pixel_end().y >= STATUS_BAR_BOTTOM_BAND;
    if !in_band {
        return false;
    }
    let Some(playfield) = playfield else {
        return false;
    };
    let Some(&first) = data.tiles().first() else {
        return false;
    };
    let tile = frame.tile(first);
    let own = (tile.x().rem_euclid(8), tile.y().rem_euclid(8));
    own != playfield
}

/// The status bars of this frame that were learned as something else, while the level stood
/// still, become status bars now (once: a pattern already made a status bar is left alone).
pub(crate) fn promote_status_bars(frame: &ConversionFrame, context: &mut SegmentContext<'_>) {
    if !enabled() {
        return;
    }
    let playfield = playfield_offset(frame);
    if playfield.is_none() {
        return;
    }
    for &shape in frame.shapes() {
        let data = frame.shape(shape);
        if !data.background() {
            continue;
        }
        let Some(id) = data.reference_pattern() else {
            continue;
        };
        if !is_status_bar(frame, shape, playfield) {
            continue;
        }
        let Some(pattern) = context.patterns.get_mut(id) else {
            continue;
        };
        let Some(instance) = pattern.instances_mut().first_mut() else {
            continue;
        };
        if instance.depth == 1 && !instance.cast_shadow {
            continue;
        }
        apply(Kind::StatusBar, instance);
        build_geometry(
            context.patterns,
            id,
            context.host,
            context.geometry,
            context.settings.render_algorithm,
        );
    }
}

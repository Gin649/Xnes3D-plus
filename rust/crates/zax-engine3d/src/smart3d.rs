//! Smart 3D (Xnes3D+): the settings a pattern learned with no profile gets from its shape,
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
//! - with no profile the engine learns most of the background one tile at a time, so a single
//!   tile is also looked at with its neighbours: a tile with nothing above and below it is a
//!   letter of a line of text and is kept thin;
//! - the sky goes to the back, frame by frame (`push_sky_back`): a pattern is shared by every
//!   palette its image is drawn with, so the same plain tile can be a sky in one place and a
//!   floor or a dialog panel in another, and only the frame knows which;
//! - a status bar (score, lives, health) is kept as a thin flat panel with no shadow. It stays on
//!   the first layer like the rest: moving only the pieces recognised as the bar to the interface
//!   layer pulled them away from the pieces that were not (a digit that just changed), and
//!   scrambled the bar. It is told by the split scrolling NES games use for it: a band at the top or the
//!   bottom of the screen whose tiles do not scroll with the rest. A pattern first learned while
//!   the level stood still becomes a status bar the first frame its band is seen standing still
//!   while the level scrolls (`promote_status_bars`).
//!
//! - the characters (every sprite in front of the background) come a little forward, so they
//!   stand on one plane ahead of the scenery (`raise_sprites`).
//!
//! The numbers are first values to be tuned on real games.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::engine::build_geometry;
use crate::frame::{ConversionFrame, FrameTile, ShapeId};
use crate::vectors::Cell;
use crate::instance::PatternInstance;
use crate::settings::ZLayer;
use crate::segmenter::SegmentContext;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turns Smart 3D on for the game about to start (the "3D by default" entry) or off (2D Mode or a
/// profile).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    // Each game starts with no sky found yet.
    SKY.store(NO_SKY, Ordering::Relaxed);
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
/// Xnes3D+: a background shape at most this many tiles tall, and not solid, is a line of text.
const TEXT_LINE_ROWS: i32 = 1;
/// From this share of drawn pixels a background shape is solid.
const SOLID_FILL: f32 = 0.70;
/// A status bar is at most this many tiles tall...
const STATUS_BAR_MAX_ROWS: i32 = 6;
/// ...at least this many wide...
/// Xnes3D+: one, since with no profile most of a status bar is learned one tile at a time.
const STATUS_BAR_MIN_COLUMNS: i32 = 1;
/// ...and starts in the top band of the screen, or ends in the bottom one (pixels).
const STATUS_BAR_TOP_BAND: i32 = 64;
const STATUS_BAR_BOTTOM_BAND: i32 = 176;

/// The 3D settings of each kind.
pub fn apply(kind: Kind, instance: &mut PatternInstance) {
    let (depth, layer) = match kind {
        // Xnes3D+, second tuning (owner request: stand out more): characters about twice as
        // thick, solid scenery half as thick again, the backdrop one layer further back.
        Kind::Sprite => (12, ZLayer::L1),
        Kind::SpriteBehind => (4, ZLayer::L2),
        // Thin but still in front: small shapes such as the letters of a word are each a shape,
        // and some letters are sparse and some not, so moving the sparse ones back would break
        // words apart. Only large backdrops go to the back layers.
        Kind::Sparse => (1, ZLayer::L1),
        Kind::Medium => (6, ZLayer::L1),
        Kind::Solid => (12, ZLayer::L1),
        Kind::Backdrop => (2, ZLayer::L4),
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
    // Xnes3D+: a single tile, the usual case with no profile, is judged with its neighbours.
    if tiles == 1 {
        if let Some(&first) = data.tiles().first() {
            let tile = frame.tile(first);
            if fill < SOLID_FILL && is_text_tile(frame, tile) {
                return Kind::Sparse;
            }
        }
    }
    // Xnes3D+: a background shape of several tiles in one row that is not solid is most often
    // a line of text (a whole word joins into one shape once learned), so it is kept thin like a
    // single letter. A solid one-tile ledge or platform still gets its full depth.
    if tiles > 1 && data.tile_size().y <= TEXT_LINE_ROWS && fill < SOLID_FILL {
        return Kind::Sparse;
    }
    if fill < SPARSE_FILL {
        Kind::Sparse
    } else if fill >= SOLID_FILL {
        Kind::Solid
    } else {
        Kind::Medium
    }
}

/// Xnes3D+: true when the cells above and below `tile` hold no background tile (only the
/// background colour): the tile is part of a line of text, such as a story or a title screen.
/// Text printed on a plain panel needs nothing here: its tiles are full, as the panel's are,
/// so both get the same depth and the letters sit flush with the panel.
fn is_text_tile(frame: &ConversionFrame, tile: &FrameTile) -> bool {
    let cell = tile.position();
    let clear = |cell: Cell| frame.background_at_cell(cell).is_none();
    clear(Cell::new(cell.x, cell.y - 1)) && clear(Cell::new(cell.x, cell.y + 1))
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

// ---- Xnes3D+: the characters ---------------------------------------------------------------

/// How far the characters come forward, in pixels (the scenery's front is at 0).
const SPRITE_RAISE: f32 = 4.0;

/// Xnes3D+ (owner request): every sprite drawn in front of the background (the main character
/// and the enemies alike) comes a little forward, so they all stand on one plane ahead of the
/// scenery, at their full depth. Sprites the game draws behind the background stay where they
/// are.
fn raise_sprites(frame: &mut ConversionFrame) {
    let raised: Vec<_> = frame
        .instances()
        .iter()
        .copied()
        .filter(|&id| {
            let instance = frame.instance(id);
            !instance.background() && !frame.shape(instance.shape()).hidden()
        })
        .collect();
    for id in raised {
        frame.instance_mut(id).offset.z -= SPRITE_RAISE;
    }
}

// ---- Xnes3D+: the sky ----------------------------------------------------------------------

/// A colour is the sky's when plain tiles of it make at least this share of the background...
const SKY_SHARE: f32 = 0.30;
/// ...and sit, on average, above this screen row (pixels): a large floor lower down is not a sky.
const SKY_LOWEST_MEAN_Y: i64 = 120;
/// How far back the sky goes, in pixels: two layers behind the scenery's front.
const SKY_PUSH: f32 = 24.0;
/// Xnes3D+: once a sky is found, it stays the sky while plain tiles of its colour make at least
/// this share of the background. Without this, the sky went back only while it filled 30% of
/// the screen, so scrolling past tall buildings (Darkwing Duck) brought it forward flush with
/// the scenery, which then looked flat, and near that limit it flipped every few frames.
const SKY_KEEP_SHARE: f32 = 0.05;
/// A remembered sky is still not used when its plain tiles sit, on average, below this row: then
/// that colour is most likely a floor or water here, not the sky.
const SKY_KEEP_LOWEST_MEAN_Y: i64 = 160;
/// The value of `SKY` when no sky has been found yet.
const NO_SKY: u8 = 0xFF;
/// Xnes3D+: the NES colour of the sky found earlier in this game, or `NO_SKY`.
static SKY: AtomicU8 = AtomicU8::new(NO_SKY);

/// True when the tile is plain: all its pixels drawn, in one colour.
fn is_plain(tile: &FrameTile) -> bool {
    tile.image().pixel_count() >= 64 && tile.image().color_count() == 1
}

/// The NES colour a pixel value (1 to 3) of a background tile shows in this frame.
fn nes_color(frame: &ConversionFrame, tile: &FrameTile, value: i32) -> Option<u8> {
    let entry = usize::try_from(tile.palette() * 4 + value).ok()?;
    frame.palette().get(entry).map(|color| color & 0x3F)
}

/// The sky's colour in this frame: the colour of the plain tiles that cover a large share of the
/// background, mostly in its upper half. `None` when no colour does (the sky of most games is
/// the background colour itself, which is not drawn as tiles at all).
fn sky_color(frame: &ConversionFrame) -> Option<u8> {
    // Per NES colour: plain tiles counted and the sum of their rows.
    let mut counts = [(0u32, 0i64); 64];
    let mut total = 0u32;
    for tile in frame.background_tiles() {
        total += 1;
        if !is_plain(tile) {
            continue;
        }
        let value = tile.image().color_at(0, 0);
        if let Some(color) = nes_color(frame, tile, value) {
            if let Some(entry) = counts.get_mut(usize::from(color)) {
                entry.0 += 1;
                entry.1 += i64::from(tile.y());
            }
        }
    }
    #[allow(clippy::cast_precision_loss)] // at most a few thousand tiles
    let share = |count: u32| count as f32 / total.max(1) as f32;
    let mean_row = |count: u32, rows: i64| rows / i64::from(count.max(1));
    // Xnes3D+: the sky found earlier stays the sky while some of it is still on screen, so it
    // does not jump forward and back as the scenery covers more or less of it.
    let remembered = SKY.load(Ordering::Relaxed);
    if let Some(&(count, rows)) = counts.get(usize::from(remembered)) {
        if share(count) >= SKY_KEEP_SHARE && mean_row(count, rows) < SKY_KEEP_LOWEST_MEAN_Y {
            return Some(remembered);
        }
    }
    let (color, &(count, rows)) = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, (count, _))| *count)?;
    if share(count) < SKY_SHARE || mean_row(count, rows) >= SKY_LOWEST_MEAN_Y {
        return None;
    }
    let color = u8::try_from(color).ok()?;
    SKY.store(color, Ordering::Relaxed);
    Some(color)
}

/// True when at least half of the tile's pixels show `sky`: a plain sky tile, or the edge of the
/// sky around a cloud, a rope or a rooftop, which goes back with it instead of standing out.
fn is_mostly_sky(frame: &ConversionFrame, tile: &FrameTile, sky: u8) -> bool {
    let image = tile.image();
    if is_plain(tile) {
        return nes_color(frame, tile, image.color_at(0, 0)) == Some(sky);
    }
    let mut colors = [None; 4];
    for (value, slot) in colors.iter_mut().enumerate().skip(1) {
        *slot = i32::try_from(value)
            .ok()
            .and_then(|value| nes_color(frame, tile, value));
    }
    let mut sky_pixels = 0;
    for y in 0..8 {
        for x in 0..8 {
            let value = usize::try_from(image.color_at(x, y)).unwrap_or(0);
            if value != 0 && colors.get(value).copied().flatten() == Some(sky) {
                sky_pixels += 1;
            }
        }
    }
    sky_pixels >= 32
}

/// Xnes3D+ (owner request): the sky of this frame goes back, so the scenery stands in front of
/// it instead of flat against it. A background shape goes back when at least half of its tiles
/// are mostly sky (a run of plain sky tiles is often one shape).
fn push_sky_back(frame: &mut ConversionFrame) {
    let Some(sky) = sky_color(frame) else {
        return;
    };
    let pushed: Vec<_> = frame
        .instances()
        .iter()
        .copied()
        .filter(|&id| {
            let instance = frame.instance(id);
            if !instance.background() {
                return false;
            }
            let data = frame.shape(instance.shape());
            let sky_tiles = data
                .tiles()
                .iter()
                .filter(|&&tile| is_mostly_sky(frame, frame.tile(tile), sky))
                .count();
            sky_tiles * 2 >= data.tile_count().max(1)
        })
        .collect();
    for id in pushed {
        let instance = frame.instance_mut(id);
        instance.offset.z += SKY_PUSH;
        // Owner request: no shadows on the sky, so it stays a plain colour behind the scenery.
        instance.cast_shadow = false;
        instance.receive_shadow = false;
    }
}

/// Xnes3D+: the changes Smart 3D makes to each frame once it is segmented: the sky goes back
/// and the characters come forward.
pub(crate) fn adjust_frame(frame: &mut ConversionFrame) {
    if !enabled() {
        return;
    }
    push_sky_back(frame);
    raise_sprites(frame);
}

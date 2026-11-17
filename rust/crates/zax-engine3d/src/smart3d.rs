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
//!   tile is also looked at with its neighbours: the plain tile that fills much of the screen is
//!   the sky and goes to the back, and a tile with nothing above and below it is a letter of a
//!   line of text and is kept thin;
//! - a status bar (score, lives, health) is kept as a thin flat panel with no shadow. It stays on
//!   the first layer like the rest: moving only the pieces recognised as the bar to the interface
//!   layer pulled them away from the pieces that were not (a digit that just changed), and
//!   scrambled the bar. It is told by the split scrolling NES games use for it: a band at the top or the
//!   bottom of the screen whose tiles do not scroll with the rest. A pattern first learned while
//!   the level stood still becomes a status bar the first frame its band is seen standing still
//!   while the level scrolls (`promote_status_bars`).
//!
//! - the main character, the sprite that keeps moving the way the D-pad is pressed, comes a
//!   little forward so it stands out from the scenery (`raise_main_character`).
//!
//! The numbers are first values to be tuned on real games.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

use crate::engine::build_geometry;
use crate::frame::{ConversionFrame, FrameTile, ShapeId};
use crate::vectors::Cell;
use crate::instance::PatternInstance;
use crate::settings::ZLayer;
use crate::segmenter::SegmentContext;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Xnes3D+: the buttons of player 1 in the frame being played, in the NES order (bit 0 A, 1 B,
/// 2 Select, 3 Start, 4 Up, 5 Down, 6 Left, 7 Right). The host sets it every frame.
static PAD: AtomicU8 = AtomicU8::new(0);

/// Xnes3D+: what is known so far about which sprite is the main character.
static TRACKER: Mutex<Tracker> = Mutex::new(Tracker::new());

/// Turns Smart 3D on for the game about to start (the "3D by default" entry) or off (2D Mode or a
/// profile).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    // A new game starts with no main character known.
    if let Ok(mut tracker) = TRACKER.lock() {
        *tracker = Tracker::new();
    }
}

/// Xnes3D+: the buttons player 1 holds in the frame about to run (port 1 of the input word).
pub fn set_pad(buttons: u8) {
    PAD.store(buttons, Ordering::Relaxed);
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
    /// Xnes3D+: a plain tile of one colour that fills much of the screen, such as a sky.
    Sky,
    /// A status bar: score, lives, health.
    StatusBar,
}

/// Below this share of drawn pixels a background shape is sparse.
const SPARSE_FILL: f32 = 0.25;
/// Xnes3D+: a plain tile is the sky when at least this share of the background tiles shows it.
const SKY_SHARE: f32 = 0.20;
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
        // Xnes3D+ (owner request): the sky sits behind the scenery, so the picture has depth
        // instead of the scenery standing flat against it.
        Kind::Sky => (2, ZLayer::L4),
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
            if is_sky_tile(frame, tile) {
                return Kind::Sky;
            }
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

/// Xnes3D+: true when `tile` is plain (all its pixels drawn, in one colour) and the same image
/// covers a large share of the background, as a sky does.
fn is_sky_tile(frame: &ConversionFrame, tile: &FrameTile) -> bool {
    let image = tile.image();
    if image.pixel_count() < 64 || image.color_count() != 1 {
        return false;
    }
    let mut total = 0u32;
    let mut same = 0u32;
    for other in frame.background_tiles() {
        total += 1;
        if other.image().id() == image.id() {
            same += 1;
        }
    }
    #[allow(clippy::cast_precision_loss)] // at most a few thousand tiles
    let share = same as f32 / total.max(1) as f32;
    share >= SKY_SHARE
}

/// Xnes3D+: true when the cells above and below `tile` hold no background tile (only the
/// background colour): the tile is part of a line of text, such as a story or a title screen.
fn is_text_tile(frame: &ConversionFrame, tile: &FrameTile) -> bool {
    let cell = tile.position();
    let above = Cell::new(cell.x, cell.y - 1);
    let below = Cell::new(cell.x, cell.y + 1);
    frame.background_at_cell(above).is_none() && frame.background_at_cell(below).is_none()
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

// ---- Xnes3D+: the main character -----------------------------------------------------------

/// How far the main character comes forward, in pixels (the scenery's front is at 0).
const MAIN_CHARACTER_RAISE: f32 = 4.0;
/// A sprite is followed from one frame to the next when its centre moved at most this far.
const FOLLOW_DISTANCE: f32 = 14.0;
/// Frames a followed sprite may be missing (flicker, a blink after a hit) before it is dropped.
const MISSING_FRAMES: u8 = 8;
/// Score a sprite needs before it is taken for the main character: about half a second of
/// moving the way the D-pad is pressed.
const MAIN_SCORE: f32 = 30.0;
/// Highest score, so a sprite that stops being the character loses the place in a few seconds.
const MAX_SCORE: f32 = 120.0;
/// Another sprite takes the place only once it scores this much more than the current one.
const SWITCH_MARGIN: f32 = 15.0;
/// Score lost by a sprite each frame it moves sideways while no direction is pressed.
const IDLE_PENALTY: f32 = 0.5;

const PAD_LEFT: u8 = 1 << 6;
const PAD_RIGHT: u8 = 1 << 7;

/// One sprite followed across frames.
#[derive(Debug, Clone, Copy)]
struct Followed {
    x: f32,
    y: f32,
    score: f32,
    missing: u8,
    seen: bool,
}

/// The sprites followed so far, and which one is the main character.
#[derive(Debug)]
struct Tracker {
    followed: Vec<Followed>,
    main: Option<usize>,
    last_fine_x: Option<i32>,
}

impl Tracker {
    const fn new() -> Self {
        Self {
            followed: Vec::new(),
            main: None,
            last_fine_x: None,
        }
    }
}

/// The sprite shapes of the frame drawn in front of the background, with their boxes in
/// pixels (left, top, right, bottom).
fn front_sprites(frame: &ConversionFrame) -> Vec<(ShapeId, [f32; 4])> {
    frame
        .shapes()
        .iter()
        .filter_map(|&shape| {
            let data = frame.shape(shape);
            if data.background() || data.hidden() {
                return None;
            }
            let (start, end) = (data.pixel_start(), data.pixel_end());
            #[allow(clippy::cast_precision_loss)] // screen pixels
            Some((
                shape,
                [start.x as f32, start.y as f32, end.x as f32, end.y as f32],
            ))
        })
        .collect()
}

/// How far the background moved sideways on the screen since the last frame, from the fine
/// scroll of the playfield: a game scrolling right moves its background left. NES games scroll
/// a few pixels a frame at most, so the move is taken between -4 and 3.
fn background_shift(tracker: &mut Tracker, frame: &ConversionFrame) -> i32 {
    let fine_x = playfield_offset(frame).map(|(x, _)| x);
    let shift = match (tracker.last_fine_x, fine_x) {
        (Some(last), Some(now)) => {
            let moved = (now - last).rem_euclid(8);
            if moved >= 4 {
                moved - 8
            } else {
                moved
            }
        }
        _ => 0,
    };
    tracker.last_fine_x = fine_x;
    shift
}

/// Xnes3D+: finds the main character, the sprite that keeps moving the way the D-pad is
/// pressed, and brings it a little forward for this frame, with the sprites touching it (a
/// character is often drawn in several pieces). The movement is measured against the
/// background, so a character the game keeps in the middle of a scrolling screen still counts.
pub(crate) fn raise_main_character(frame: &mut ConversionFrame) {
    if !enabled() {
        return;
    }
    let Ok(mut tracker) = TRACKER.lock() else {
        return;
    };
    let pad = PAD.load(Ordering::Relaxed);
    let direction: f32 = match (pad & PAD_LEFT != 0, pad & PAD_RIGHT != 0) {
        (true, false) => -1.0,
        (false, true) => 1.0,
        _ => 0.0,
    };
    #[allow(clippy::cast_precision_loss)] // a few pixels
    let shift = background_shift(&mut tracker, frame) as f32;
    let sprites = front_sprites(frame);

    // Follow each sprite from the last frame: the nearest followed one not taken yet.
    for followed in &mut tracker.followed {
        followed.seen = false;
    }
    let mut owners: Vec<Option<usize>> = Vec::with_capacity(sprites.len());
    for (_, area) in &sprites {
        let x = (area[0] + area[2]) / 2.0;
        let y = (area[1] + area[3]) / 2.0;
        let nearest = tracker
            .followed
            .iter()
            .enumerate()
            .filter(|(_, followed)| !followed.seen)
            .map(|(index, followed)| (index, (followed.x - x).hypot(followed.y - y)))
            .filter(|(_, distance)| *distance <= FOLLOW_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index);
        let index = if let Some(index) = nearest {
            let followed = &mut tracker.followed[index];
            // How the sprite moved in the level: on screen, minus what the scrolling did.
            let moved = (x - followed.x) - shift;
            if direction != 0.0 {
                if moved * direction > 0.0 {
                    followed.score = (followed.score + 1.0).min(MAX_SCORE);
                } else if moved * direction < 0.0 {
                    followed.score = (followed.score - 1.0).max(0.0);
                }
            } else if moved.abs() >= 1.0 {
                // Moving sideways with no direction pressed: most likely an enemy walking on
                // its own (the character mostly stops when the D-pad is let go).
                followed.score = (followed.score - IDLE_PENALTY).max(0.0);
            }
            followed.x = x;
            followed.y = y;
            followed.missing = 0;
            followed.seen = true;
            index
        } else {
            tracker.followed.push(Followed {
                x,
                y,
                score: 0.0,
                missing: 0,
                seen: true,
            });
            tracker.followed.len() - 1
        };
        owners.push(Some(index));
    }

    // Sprites gone for too long are dropped, and the indices moved to match.
    let main_before = tracker.main;
    let mut kept: Vec<Option<usize>> = Vec::with_capacity(tracker.followed.len());
    let mut next = 0;
    for followed in &mut tracker.followed {
        if !followed.seen {
            followed.missing = followed.missing.saturating_add(1);
        }
        if followed.missing > MISSING_FRAMES {
            kept.push(None);
        } else {
            kept.push(Some(next));
            next += 1;
        }
    }
    tracker.followed.retain(|followed| followed.missing <= MISSING_FRAMES);
    let renumber = |index: Option<usize>| index.and_then(|index| kept.get(index).copied().flatten());
    let owners: Vec<Option<usize>> = owners.into_iter().map(renumber).collect();
    let mut main = renumber(main_before);

    // The best scorer becomes the main character, once it scores enough and clearly beats the
    // one there now.
    let best = tracker
        .followed
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.score.total_cmp(&b.1.score))
        .map(|(index, followed)| (index, followed.score));
    if let Some((index, score)) = best {
        let current = main.and_then(|main| tracker.followed.get(main)).map_or(0.0, |f| f.score);
        if score >= MAIN_SCORE && (main.is_none() || score > current + SWITCH_MARGIN) {
            main = Some(index);
        }
    }
    if main.and_then(|main| tracker.followed.get(main)).is_some_and(|f| f.score < MAIN_SCORE / 2.0) {
        main = None;
    }
    tracker.main = main;
    let Some(main) = main else {
        return;
    };

    // The main character's box this frame, grown by one pixel so touching pieces join it.
    let Some(area) = sprites
        .iter()
        .zip(&owners)
        .filter(|(_, owner)| **owner == Some(main))
        .map(|((_, area), _)| *area)
        .next()
    else {
        return;
    };
    let grown = [area[0] - 1.0, area[1] - 1.0, area[2] + 1.0, area[3] + 1.0];
    let raised: Vec<ShapeId> = sprites
        .iter()
        .filter(|(_, other)| {
            other[0] <= grown[2] && other[2] >= grown[0] && other[1] <= grown[3] && other[3] >= grown[1]
        })
        .map(|(shape, _)| *shape)
        .collect();
    drop(tracker);

    let instances: Vec<_> = frame
        .instances()
        .iter()
        .copied()
        .filter(|&id| raised.contains(&frame.instance(id).shape()))
        .collect();
    for id in instances {
        frame.instance_mut(id).offset.z -= MAIN_CHARACTER_RAISE;
    }
}

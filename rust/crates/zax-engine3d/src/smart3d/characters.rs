//! Xnes3D+ Smart 3D: the characters as whole figures (owner request).
//!
//! The engine joins touching sprites into one shape only when they share a palette, so a
//! character drawn in two or three colours is several shapes. Here every sprite drawn in front
//! of the background is joined with the ones it touches, whatever their palette: one figure per
//! character, with the box it covers. The floors (`scenery.rs`) are found under these figures'
//! feet.
//!
//! A small sprite on its own, a bullet or a spark, is drawn thinner than a character
//! (`is_effect`, used when the sprite is first learned).

use std::collections::HashSet;

use crate::frame::{ConversionFrame, ShapeId};
use crate::registry::PatternId;

/// A sprite shape with at most this many drawn pixels, touching no other sprite, is an effect.
const EFFECT_PIXELS: i32 = 24;

/// One character on screen: the box its front sprites cover, in screen pixels (both ends
/// included).
#[derive(Debug, Clone, Copy)]
pub(super) struct Figure {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Figure {
    /// True when the two boxes overlap or are at most one pixel apart.
    const fn touches(&self, other: &Self) -> bool {
        self.left <= other.right + 1
            && other.left <= self.right + 1
            && self.top <= other.bottom + 1
            && other.top <= self.bottom + 1
    }

    /// The box covering both.
    fn join(&mut self, other: &Self) {
        self.left = self.left.min(other.left);
        self.top = self.top.min(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }

    /// The width in pixels.
    pub const fn width(&self) -> i32 {
        self.right - self.left + 1
    }
}

/// The characters of this frame: the sprites drawn in front of the background, status bar
/// sprites left out, joined into one figure per group that touches.
pub(super) fn figures(frame: &ConversionFrame, status_bar: &HashSet<PatternId>) -> Vec<Figure> {
    let mut figures: Vec<Figure> = Vec::new();
    for &id in frame.instances() {
        let instance = frame.instance(id);
        if instance.background() || status_bar.contains(&instance.reference().pattern) {
            continue;
        }
        let data = frame.shape(instance.shape());
        if data.hidden() {
            continue;
        }
        let start = data.pixel_start();
        let end = data.pixel_end();
        let mut figure = Figure {
            left: start.x,
            top: start.y,
            right: end.x,
            bottom: end.y,
        };
        // Join every figure this one touches; a join can make it touch another, so repeat.
        loop {
            let Some(index) = figures.iter().position(|other| other.touches(&figure)) else {
                break;
            };
            let other = figures.swap_remove(index);
            figure.join(&other);
        }
        figures.push(figure);
    }
    figures
}

/// True when the sprite shape is small and touches no other sprite: a bullet, a spark, a
/// sparkle, not a piece of a character.
pub(super) fn is_effect(frame: &ConversionFrame, shape: ShapeId) -> bool {
    let data = frame.shape(shape);
    if data.background() || data.pixel_count() > EFFECT_PIXELS {
        return false;
    }
    let start = data.pixel_start();
    let end = data.pixel_end();
    let own = Figure {
        left: start.x,
        top: start.y,
        right: end.x,
        bottom: end.y,
    };
    !frame.sprites().iter().any(|tile| {
        if !tile.present() || tile.shape() == Some(shape) {
            return false;
        }
        let other = Figure {
            left: tile.x(),
            top: tile.y(),
            right: tile.x() + 7,
            bottom: tile.y() + 7,
        };
        own.touches(&other)
    })
}

//! Whether a frontend (a launcher, ES-DE) started this run with a game: the command line's
//! `--rom`, or a game sent while Zaxnes ran (Android's `launch-request.txt`). Quitting the game
//! from the handheld menu then closes Zaxnes, back to the frontend, as other emulators do,
//! rather than showing Zaxnes' own ROM list.

use std::sync::atomic::{AtomicBool, Ordering};

static FROM_FRONTEND: AtomicBool = AtomicBool::new(false);

/// Marks this run as started by a frontend.
pub fn set_from_frontend() {
    FROM_FRONTEND.store(true, Ordering::Relaxed);
}

/// True when a frontend started the game.
#[must_use]
pub fn from_frontend() -> bool {
    FROM_FRONTEND.load(Ordering::Relaxed)
}

//! Xnes3D+: what Smart 3D learned about each game, kept between sessions (owner request).
//!
//! A game with no 3D profile plays in Smart 3D, which learns as it goes: the sky, the floors,
//! the status bar, the distant layers. That is kept in a small text file per ROM, read back
//! when the game starts so it looks right from the first frames, written when the game stops
//! and once a minute while it plays (Android can close the app without the game being
//! stopped). "Reset Smart 3D" in the game's options deletes the file.

use std::fs;
use std::path::Path;

use super::{EmulationHost, GameView};
use crate::emulation::game_catalog::GameCatalog;
use crate::emulation::notification_center::{NotificationSeverity, DEFAULT_SECONDS};
use crate::game_session::GameSession;
use crate::ui::keys;
use crate::ui::strings;

/// How often, in emulated frames, what was learned is written while a game plays: a minute.
pub(super) const SMART3D_SAVE_FRAMES: i64 = 3600;

impl<C: GameCatalog, V: GameView> EmulationHost<C, V> {
    /// Gives the session just made what Smart 3D learned about its game in earlier sessions.
    pub(super) fn load_smart3d_memory(&mut self) {
        if !zax_engine3d::smart3d::enabled() {
            return;
        }
        let Some(rom) = self.game_options_rom.clone() else {
            return;
        };
        let Ok(text) = fs::read_to_string(self.paths.smart3d_file(&rom)) else {
            return;
        };
        if let Some(session) = self.session.as_mut() {
            session.engine_mut().smart3d_import(&text);
            self.log("Smart 3D: what was learned about this game is back.");
        }
    }

    /// Writes what Smart 3D learned about the game in play to its file.
    pub(super) fn keep_smart3d_memory(&self) {
        if let Some(session) = self.session.as_ref() {
            self.keep_smart3d_memory_of(session);
        }
    }

    /// The same for a session already taken out of the host, as stopping a game does.
    pub(super) fn keep_smart3d_memory_of(&self, session: &GameSession) {
        if !zax_engine3d::smart3d::enabled() || !self.paths.is_writable {
            return;
        }
        let Some(rom) = self.game_options_rom.as_deref() else {
            return;
        };
        let text = session.engine().smart3d_export();
        if text.is_empty() {
            return;
        }
        let file = self.paths.smart3d_file(rom);
        if let Some(folder) = file.parent() {
            if fs::create_dir_all(folder).is_err() {
                return;
            }
        }
        if fs::write(&file, text).is_err() {
            self.log("Smart 3D: what was learned could not be written.");
        }
    }

    /// Xnes3D+ (owner request): makes Smart 3D forget the game of `rom`: its file is deleted,
    /// and when that game is the one in play, what this session learned too. The pieces
    /// already given a learned look keep it until the game starts again.
    pub fn forget_smart3d(&mut self, rom: &Path) {
        let key = super::game_options::game_options_key(&rom.to_string_lossy());
        let _ = fs::remove_file(self.paths.smart3d_file(&key));
        if self.game_options_rom.as_deref() == Some(key.as_str()) {
            if let Some(session) = self.session.as_mut() {
                session.engine_mut().smart3d_forget();
            }
        }
        self.notifications.show(
            &strings::get(keys::HANDHELD_SMART3D_FORGOTTEN),
            DEFAULT_SECONDS,
            NotificationSeverity::Normal,
        );
    }
}

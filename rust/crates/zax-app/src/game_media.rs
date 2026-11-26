//! Xnes3D+: the box art and the game information the game list and the Game info screen show.
//!
//! The Android side fills them in (see `GameMedia.java`): from the Xnes3D+ index of the
//! LaunchBox Games Database (NES and Famicom games only), or from what a frontend such as ES-DE
//! already downloaded. Each game has a folder named after its ROM file, without the extension,
//! under `media/` in the user data:
//!
//! - `box.png`: the front of the box;
//! - `shot.png`: a picture of the game;
//! - `cart.png`: the cartridge;
//! - `info.txt`: `key=value` lines (title, year, developer, publisher, players, genre,
//!   overview). The overview may hold `\n` for its line breaks.
//!
//! The pictures are PNG: Android converts any other format before it writes them, so this side
//! only reads PNG.

use std::path::{Path, PathBuf};

use zax_platform::ui_image::UiPicture;

/// The folder of the user data that holds the media of every game.
pub const FOLDER: &str = "media";

/// Which picture of a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picture {
    /// The front of the box.
    Box,
    /// A picture of the game being played.
    Shot,
    /// The cartridge.
    Cart,
}

impl Picture {
    const fn file(self) -> &'static str {
        match self {
            Self::Box => "box.png",
            Self::Shot => "shot.png",
            Self::Cart => "cart.png",
        }
    }
}

/// What the information file of a game says. Every field may be empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameInfo {
    pub title: String,
    pub year: String,
    pub developer: String,
    pub publisher: String,
    pub players: String,
    pub genre: String,
    pub overview: String,
}

impl GameInfo {
    /// The short line under the cover: year, publisher, players and genre, those it has.
    #[must_use]
    pub fn summary(&self) -> String {
        let players = if self.players.is_empty() {
            String::new()
        } else if self.players == "1" {
            "1 player".to_owned()
        } else {
            format!("{} players", self.players)
        };
        [
            self.year.as_str(),
            self.publisher.as_str(),
            players.as_str(),
            self.genre.as_str(),
        ]
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" · ")
    }
}

/// The name of the media folder of the game in `rom`: its file name without the extension.
#[must_use]
pub fn key(rom: &Path) -> Option<String> {
    rom.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
}

/// The media folder of the game in `rom`, under the user data folder `root`.
#[must_use]
pub fn folder(root: &Path, rom: &Path) -> Option<PathBuf> {
    key(rom).map(|key| root.join(FOLDER).join(key))
}

/// Picture `which` of the game in `rom`, if it has one.
#[must_use]
pub fn picture(root: &Path, rom: &Path, which: Picture) -> Option<UiPicture> {
    let file = folder(root, rom)?.join(which.file());
    let image = zax_render::assets::png_image::decode(&std::fs::read(file).ok()?).ok()?;
    let picture = UiPicture {
        width: u32::try_from(image.width).ok()?,
        height: u32::try_from(image.height).ok()?,
        rgba: image.pixels,
    };
    picture.is_whole().then_some(picture)
}

/// The information of the game in `rom`, if it has some.
#[must_use]
pub fn info(root: &Path, rom: &Path) -> Option<GameInfo> {
    let text = std::fs::read_to_string(folder(root, rom)?.join("info.txt")).ok()?;
    Some(parse_info(&text))
}

/// The information `text` holds.
#[must_use]
pub fn parse_info(text: &str) -> GameInfo {
    let mut info = GameInfo::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().replace("\\n", "\n");
        match key.trim() {
            "title" => info.title = value,
            "year" => info.year = value,
            "developer" => info.developer = value,
            "publisher" => info.publisher = value,
            "players" => info.players = value,
            "genre" => info.genre = value,
            "overview" => info.overview = value,
            _ => {}
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_information_file_is_read_line_by_line() {
        let info = parse_info(
            "title=Super Mario Bros. 3\nyear=1990\npublisher=Nintendo\nplayers=2\n\
             genre=Platform\noverview=First line\\nSecond line\nunknown=kept out\n",
        );
        assert_eq!(info.title, "Super Mario Bros. 3");
        assert_eq!(info.overview, "First line\nSecond line");
        assert_eq!(info.summary(), "1990 · Nintendo · 2 players · Platform");
    }

    #[test]
    fn the_key_is_the_file_name_without_its_extension() {
        assert_eq!(
            key(Path::new("/roms/Contra (USA).zip")).as_deref(),
            Some("Contra (USA)")
        );
    }
}

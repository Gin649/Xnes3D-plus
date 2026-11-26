//! Xnes3D+: what the emulator's own menus ask of the Android side, for the things only
//! Android can do (its folder and file pickers, downloads, the storage permission).
//!
//! A menu row sends a message through SDL (`SDL_AndroidSendMessage`), which reaches
//! `ZaxnesActivity.onUnhandledMessage` on the Java side. Java opens its picker, then answers
//! through two places in the user data folder, which the application checks about twice a second
//! (`Running::take_android_answers`):
//!
//! - [`ROMS_FOLDER_FILE`]: the path of the ROM folder the player picked;
//! - [`PROFILE_INBOX`]: a folder of new `.3dn` profiles, imported into the library and then
//!   deleted.
//!
//! Off Android nothing is sent and the rows that need it are not shown.

/// What a menu row asks of Android. The numbers must match `ZaxnesActivity.onUnhandledMessage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidAction {
    /// Pick the folder the game list shows (and get the permission to read it).
    ChooseRomFolder = 1,
    /// Pick a 3dSen bundle file and convert its profiles.
    Import3dSen = 2,
    /// Download the community profiles.
    CommunityProfiles = 3,
    /// Pick a folder of `.3dn` profiles that is copied from at each start.
    ProfilesFolder = 4,
    /// Copy from that folder now.
    CopyProfiles = 5,
    /// Pick the folder the battery saves and save states go to (used from the next start).
    ChooseSavesFolder = 6,
    /// Look for a newer Xnes3D+ release and offer to install it.
    CheckUpdates = 7,
    /// Fetch the box art and information the request file asks for ([`write_media_request`]).
    MediaRequest = 8,
    /// The row that asks for the box art and information of every game from the LaunchBox
    /// index. Rust writes the request, then sends [`Self::MediaRequest`]; Java never gets 9.
    DownloadMedia = 9,
    /// The same from what ES-DE downloaded. Java never gets 10 either.
    EsDeMedia = 10,
}

/// The file Rust writes what box art and information it wants into, for `GameMedia.java`.
pub const MEDIA_REQUEST_FILE: &str = "media-request.txt";
/// The file Java writes once it wrote new box art or information.
pub const MEDIA_DONE_FILE: &str = "media-done.txt";

/// Writes the request for the media of `roms` from `source` ("launchbox", "esde", or "pick"
/// for another LaunchBox entry of one game) into the user data folder `root`. False when it
/// could not be written.
#[must_use]
pub fn write_media_request(root: &std::path::Path, source: &str, roms: &[&std::path::Path]) -> bool {
    let mut text = format!("source={source}\n");
    for rom in roms {
        text.push_str(&format!("rom={}\n", rom.display()));
    }
    let part = root.join(format!("{MEDIA_REQUEST_FILE}.part"));
    std::fs::write(&part, text).is_ok() && std::fs::rename(&part, root.join(MEDIA_REQUEST_FILE)).is_ok()
}

/// The Xnes3D+ version this build is (`.github/fork/VERSION`), shown beside Check for updates.
#[must_use]
pub fn app_version() -> &'static str {
    include_str!("../../../../.github/fork/VERSION").trim()
}

/// The file Java writes the picked ROM folder to. Must match ZaxnesActivity.ROMS_FOLDER_FILE.
pub const ROMS_FOLDER_FILE: &str = "roms-folder.txt";
/// The file Java writes the picked saves folder to. Must match ProfilesActivity.SAVES_FOLDER_FILE.
pub const SAVES_FOLDER_FILE: &str = "saves-folder.txt";

/// The saves folder chosen: the one in use since the start (`--saves`), or one picked since,
/// which is used from the next start. Shown on its row of the options.
static SAVES_FOLDER: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Remembers the saves folder chosen, for its row of the options.
pub fn set_saves_folder(folder: &str) {
    if let Ok(mut chosen) = SAVES_FOLDER.lock() {
        *chosen = Some(folder.to_owned());
    }
}

/// The saves folder chosen, if any.
#[must_use]
pub fn saves_folder() -> Option<String> {
    SAVES_FOLDER.lock().ok().and_then(|chosen| chosen.clone())
}

/// The folder Java puts new profiles in. Must match ProfileFolder.INBOX.
pub const PROFILE_INBOX: &str = "profile-inbox";

/// Whether the rows that need Android are offered. `ZAXNES_PLUS_ANDROID_ROWS=1` shows them on
/// a desktop too, for the screenshots of the interface (they do nothing there).
#[must_use]
pub fn available() -> bool {
    cfg!(target_os = "android")
        || std::env::var("ZAXNES_PLUS_ANDROID_ROWS").is_ok_and(|value| value == "1")
}

/// Asks Android for `action`. False when it could not be sent (off Android, or SDL refused it).
#[must_use]
pub fn ask(action: AndroidAction) -> bool {
    zax_platform::android_message::send(action as u32)
}

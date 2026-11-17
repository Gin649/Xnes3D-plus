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
}

/// The file Java writes the picked ROM folder to. Must match ZaxnesActivity.ROMS_FOLDER_FILE.
pub const ROMS_FOLDER_FILE: &str = "roms-folder.txt";
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

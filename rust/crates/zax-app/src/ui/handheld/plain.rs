//! Xnes3D+: plain names for the options and a short explanation of each, shown under the list
//! for the selected row. Xnes3D+ is in English only, so these are plain English strings rather
//! than translation keys. A row or page that is not listed here keeps its Zaxnes name and has
//! no explanation.

use zax_settings::catalogue::SettingId;

use super::options::{OptionRow, OptionsPage};

/// The plain name of `row`, if Xnes3D+ gives it one.
#[must_use]
pub fn label(row: OptionRow) -> Option<&'static str> {
    Some(match row {
        OptionRow::Preset => "Performance",
        OptionRow::WidescreenOn => "Widescreen",
        OptionRow::RenderScale => "3D sharpness",
        OptionRow::Palette => "Colours",
        OptionRow::Overscan => "Picture edges",
        OptionRow::Region => "TV region",
        OptionRow::ShowFps => "Show frame rate",
        OptionRow::FrameCap => "Frame rate limit",
        OptionRow::CatchUp => "Catch up when slow",
        OptionRow::Signal => "TV signal",
        OptionRow::Choice(SettingId::UiOrientation) => "Screen orientation",
        OptionRow::Setting(id) => return setting_label(id),
        OptionRow::Page(page) => return page_title(page),
        _ => return None,
    })
}

/// The plain name of a setting row, if Xnes3D+ gives it one.
fn setting_label(id: SettingId) -> Option<&'static str> {
    Some(match id {
        SettingId::VideoVSync => "Sync to the screen",
        SettingId::VideoAntiAlias => "Smooth 3D edges",
        SettingId::VideoTvModel => "TV look",
        SettingId::RenderVisualStyle => "3D style",
        SettingId::RenderStyleReflections => "Reflections",
        SettingId::LightIntensity => "Light",
        SettingId::CameraFov => "Field of view",
        SettingId::AudioConsoleProfile => "Console sound",
        SettingId::EmuRunAhead => "Lower input lag",
        SettingId::EmuSmoothScrollAhead => "Smooth scrolling",
        SettingId::EmuSmoothScroll3d => "Smooth scrolling in 3D",
        SettingId::EmuExactRate => "Exact console speed",
        SettingId::EmuRealtimeInput => "Fast button reading",
        _ => return None,
    })
}

/// The plain title of `page`, if Xnes3D+ gives it one.
#[must_use]
pub fn page_title(page: OptionsPage) -> Option<&'static str> {
    Some(match page {
        OptionsPage::Library => super::options::LIBRARY_TITLE,
        OptionsPage::AllSettings => "Advanced settings",
        OptionsPage::SimpleDisplay => "Screen",
        OptionsPage::SimpleNesPicture => "Picture",
        OptionsPage::SimpleRendering3d => "3D view",
        OptionsPage::SimpleAudio => "Sound",
        OptionsPage::SimpleEmulation => "Emulation",
        OptionsPage::Optimizations => "Speed tweaks",
        OptionsPage::Advanced => "Expert settings",
        OptionsPage::Widescreen => "Widescreen and hacks",
        OptionsPage::Shaders => "Screen filters (shaders)",
        _ => return None,
    })
}

/// A short explanation of `row`, shown under the list while it is selected.
#[must_use]
pub fn help(row: OptionRow) -> Option<&'static str> {
    Some(match row {
        OptionRow::Preset => {
            "Fastest keeps every game smooth on a handheld. The faithful choices match the real \
             console more closely but need more power."
        }
        OptionRow::Volume => "How loud the games are.",
        OptionRow::WidescreenOn => {
            "Shows more of the level on the sides in games that have a widescreen patch."
        }
        OptionRow::RenderScale => {
            "How sharp the 3D picture is drawn. Higher looks better and uses more power."
        }
        OptionRow::Shadows => "Shadows under the 3D blocks and characters.",
        OptionRow::Palette => "The set of colours the NES picture uses.",
        OptionRow::Overscan => "How much of the picture's edges is cut, as on an old TV.",
        OptionRow::Region => {
            "Auto picks the right speed for each game: NTSC (60 Hz, America and Japan) or PAL \
             (50 Hz, Europe)."
        }
        OptionRow::ShowFps => "Shows how many frames a second the game runs at.",
        OptionRow::Signal => {
            "Off shows clean pixels. Composite, S-Video and RF copy the cables of the real \
             console: S-Video is the sharpest of them, RF the blurriest."
        }
        OptionRow::Choice(SettingId::UiOrientation) => {
            "Landscape suits handhelds. Portrait puts the game on top and the touch pad under it."
        }
        OptionRow::Controls => "Which button does what.",
        OptionRow::Achievements => "Your RetroAchievements account and its settings.",
        OptionRow::Reset => "Puts every setting on this page back to its default.",
        OptionRow::Android(crate::android_bridge::AndroidAction::DownloadMedia) => {
            "Downloads the box art, pictures and information of every game in your list from \
             the LaunchBox Games Database. Press R2 on a game to see them."
        }
        OptionRow::Android(crate::android_bridge::AndroidAction::EsDeMedia) => {
            "Copies the box art and information ES-DE already downloaded for your NES and \
             Famicom games. Nothing is downloaded."
        }
        OptionRow::Setting(id) => return setting_help(id),
        OptionRow::Page(page) => return page_help(page),
        _ => return None,
    })
}

fn setting_help(id: SettingId) -> Option<&'static str> {
    Some(match id {
        SettingId::VideoVSync => {
            "Waits for the screen before showing each frame, so the picture never tears."
        }
        SettingId::VideoAntiAlias => "Smooths the jagged edges of the 3D blocks.",
        SettingId::VideoTvModel => "Makes the picture look like a modern screen or an old TV.",
        SettingId::RenderVisualStyle => "The look of the 3D blocks.",
        SettingId::RenderStyleReflections => "Shiny reflections on the 3D floor. Uses more power.",
        SettingId::LightIntensity => "How bright the light on the 3D scene is.",
        SettingId::CameraFov => "How wide the 3D camera sees. Wider shows more of the scene.",
        SettingId::AudioConsoleProfile => {
            "None plays the clean sound. The others copy the sound of a measured real console."
        }
        SettingId::EmuRunAhead => {
            "Makes the game answer your buttons sooner by running frames ahead. Uses more power."
        }
        SettingId::EmuSmoothScrollAhead => {
            "On screens faster than 60 Hz, slides the picture between frames so scrolling looks \
             smoother. Needs Lower input lag."
        }
        SettingId::EmuSmoothScroll3d => "The same smooth scrolling for the 3D scene.",
        SettingId::EmuExactRate => {
            "Runs at the real console's exact speed (60.1 frames a second) rather than an even 60."
        }
        SettingId::EmuRealtimeInput => {
            "Reads the buttons about every millisecond instead of once a frame, for less lag."
        }
        _ => return None,
    })
}

fn page_help(page: OptionsPage) -> Option<&'static str> {
    Some(match page {
        OptionsPage::Library => "Your ROM and saves folders, and where the 3D profiles come from.",
        OptionsPage::Touch => "The on-screen pad, its size and its vibration.",
        OptionsPage::AllSettings => {
            "Every setting, for when you want to fine-tune. The pages above cover most needs."
        }
        OptionsPage::SimpleDisplay => "Frame rate, screen sync, smooth edges and screen filters.",
        OptionsPage::SimpleNesPicture => "The TV signal, colours and picture edges.",
        OptionsPage::SimpleRendering3d => "The 3D style, light, shadows and camera.",
        OptionsPage::SimpleAudio => "The volume and the console's sound.",
        OptionsPage::SimpleEmulation => "Input lag, smooth scrolling and console speed.",
        OptionsPage::Optimizations => "Switches that trade accuracy for speed.",
        OptionsPage::Advanced => "Detailed settings for the picture, 3D, emulation and sound.",
        OptionsPage::Widescreen => "The defaults for every game with a widescreen patch.",
        OptionsPage::Shaders => "Filters drawn over the picture, such as CRT scanlines.",
        _ => return None,
    })
}

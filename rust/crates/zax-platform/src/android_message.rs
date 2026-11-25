//! Xnes3D+: a message from the native side to the Android activity
//! (`SDL_AndroidSendMessage`, which reaches `SDLActivity.onUnhandledMessage` on the Java side).

/// The Xnes3D+ messages start here. SDL's user messages start at 0x8000 and Zaxnes uses the
/// first ones (the screen orientation is 0x8001), so Xnes3D+ keeps well above them. Must match
/// XNES_MESSAGES in ZaxnesActivity.java.
pub const FIRST_USER_MESSAGE: u32 = 0x8100;

/// Sends the user message `FIRST_USER_MESSAGE + number` to the activity. False off Android, or
/// when SDL refused it.
#[must_use]
pub fn send(number: u32) -> bool {
    #[cfg(target_os = "android")]
    {
        extern "C" {
            fn SDL_AndroidSendMessage(command: u32, param: i32) -> i32;
        }
        // SAFETY: SDL is initialised (the menus asking for this are drawn through it), and the
        // call only posts a message to the activity's handler thread.
        let result = unsafe { SDL_AndroidSendMessage(FIRST_USER_MESSAGE + number, 0) };
        result == 0
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = number;
        false
    }
}

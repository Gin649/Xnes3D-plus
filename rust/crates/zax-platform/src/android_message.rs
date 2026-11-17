//! Zaxnes Plus: a message from the native side to the Android activity
//! (`SDL_AndroidSendMessage`, which reaches `SDLActivity.onUnhandledMessage` on the Java side).

/// SDL's user messages start here (lower numbers are SDL's own commands).
pub const FIRST_USER_MESSAGE: u32 = 0x8000;

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

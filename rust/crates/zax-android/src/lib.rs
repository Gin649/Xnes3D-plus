//! zax-android: the Android entry of Zaxnes. SDL's Java activity (`org.libsdl.app.SDLActivity`)
//! loads `libSDL2.so`, then this library, and calls its `SDL_main` on a thread of its own.
//!
//! The application runs as on a handheld (`--handheld`, the console interface driven by a
//! gamepad), its user data in the app's own folder on the shared storage
//! (`Android/data/<package>/files`): the app writes there without any permission, and the ROMs,
//! profiles, battery saves, save states and movies are copied there from a computer.

#[cfg(target_os = "android")]
mod entry {
    use std::ffi::{c_char, c_int, CStr};

    extern "C" {
        /// SDL's path of the app's folder on the shared storage (`getExternalFilesDir`), or null.
        fn SDL_AndroidGetExternalStoragePath() -> *const c_char;
    }

    /// The app's folder on the shared storage, if Android gives one.
    fn external_storage() -> Option<String> {
        // SAFETY: SDL returns null or a NUL-terminated string it keeps for the process' life.
        let path = unsafe { SDL_AndroidGetExternalStoragePath() };
        if path.is_null() {
            return None;
        }
        // SAFETY: checked non-null above; SDL's string is NUL-terminated.
        let path = unsafe { CStr::from_ptr(path) };
        Some(path.to_string_lossy().into_owned())
    }

    /// The arguments the activity passes on (`ZaxnesActivity.getArguments`): `--rom <file>` when
    /// a frontend started it with a game. SDL puts a program name first, which is skipped.
    fn passed_arguments(argc: c_int, argv: *mut *mut c_char) -> Vec<String> {
        if argv.is_null() {
            return Vec::new();
        }
        let count = usize::try_from(argc).unwrap_or(0);
        (1..count)
            .filter_map(|index| {
                // SAFETY: SDL hands `argc` entries of `argv`, each null or NUL-terminated.
                let item = unsafe { *argv.add(index) };
                if item.is_null() {
                    return None;
                }
                // SAFETY: checked non-null above; SDL's strings are NUL-terminated.
                Some(unsafe { CStr::from_ptr(item) }.to_string_lossy().into_owned())
            })
            .collect()
    }

    /// Called by SDL once its activity is up.
    #[no_mangle]
    pub extern "C" fn SDL_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
        let mut arguments = vec!["--handheld".to_owned()];
        if let Some(root) = external_storage() {
            arguments.push("--userdata".to_owned());
            arguments.push(root);
        }
        arguments.extend(passed_arguments(argc, argv));
        // A panic must not unwind into SDL's C code.
        std::panic::catch_unwind(|| zax_app::program::run(&arguments)).unwrap_or(1)
    }
}

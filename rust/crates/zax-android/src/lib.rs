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

    /// The folder, in the user data, where the activity puts a 3dSen profile bundle to convert
    /// (`ProfileFolder.java`).
    const IMPORT_FOLDER: &str = "import-3dsen";

    /// Converts the 3dSen profile bundles waiting in [`IMPORT_FOLDER`] into `.3dn` profiles, with
    /// the same importer as File > Import from 3dSen, then removes them.
    fn import_waiting_bundles(root: &str) {
        let folder = std::path::Path::new(root).join(IMPORT_FOLDER);
        let Ok(entries) = std::fs::read_dir(&folder) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "bundle") {
                continue;
            }
            let arguments = vec![
                "--import-3dsen".to_owned(),
                path.to_string_lossy().into_owned(),
                "--userdata".to_owned(),
                root.to_owned(),
            ];
            let _ = std::panic::catch_unwind(|| zax_app::program::run(&arguments));
            let _ = std::fs::remove_file(&path);
        }
    }

    /// Called by SDL once its activity is up.
    #[no_mangle]
    pub extern "C" fn SDL_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
        let mut arguments = vec!["--handheld".to_owned()];
        if let Some(root) = external_storage() {
            import_waiting_bundles(&root);
            arguments.push("--userdata".to_owned());
            arguments.push(root);
        }
        arguments.extend(passed_arguments(argc, argv));
        // A panic must not unwind into SDL's C code.
        std::panic::catch_unwind(|| zax_app::program::run(&arguments)).unwrap_or(1)
    }
}

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

    // ---- Xnes3D+ profiles: converting a 3dSen bundle picked on the device -------------------

    use std::ffi::{c_void, CString};

    /// `JNIEnv*`: a pointer to the pointer to JNI's function table.
    type JniEnv = *mut *const *const c_void;
    type JString = *mut c_void;

    // Positions in JNI's function table (the JNI specification, fixed since Java 1.2).
    const NEW_STRING_UTF: usize = 167;
    const GET_STRING_UTF_CHARS: usize = 169;
    const RELEASE_STRING_UTF_CHARS: usize = 170;

    /// The function at `index` of the table of `env`.
    ///
    /// # Safety
    /// `env` is the `JNIEnv*` JNI handed to the native method, on its thread.
    unsafe fn jni_function(env: JniEnv, index: usize) -> *const c_void {
        // SAFETY: the caller's env points at JNI's table, which has these entries.
        unsafe { *(*env).add(index) }
    }

    /// A Java string as Rust text.
    ///
    /// # Safety
    /// As [`jni_function`]; `text` is a live `java.lang.String` reference or null.
    unsafe fn read_string(env: JniEnv, text: JString) -> Option<String> {
        if text.is_null() {
            return None;
        }
        type Get = unsafe extern "C" fn(JniEnv, JString, *mut u8) -> *const c_char;
        type Release = unsafe extern "C" fn(JniEnv, JString, *const c_char);
        // SAFETY: the table entries at these positions have these signatures.
        let get: Get = unsafe { std::mem::transmute(jni_function(env, GET_STRING_UTF_CHARS)) };
        let release: Release =
            unsafe { std::mem::transmute(jni_function(env, RELEASE_STRING_UTF_CHARS)) };
        // SAFETY: JNI returns null or modified UTF-8, NUL-terminated, until released.
        let chars = unsafe { get(env, text, std::ptr::null_mut()) };
        if chars.is_null() {
            return None;
        }
        let value = unsafe { CStr::from_ptr(chars) }.to_string_lossy().into_owned();
        unsafe { release(env, text, chars) };
        Some(value)
    }

    /// Rust text as a new Java string (null on failure).
    ///
    /// # Safety
    /// As [`jni_function`].
    unsafe fn new_string(env: JniEnv, text: &str) -> JString {
        type New = unsafe extern "C" fn(JniEnv, *const c_char) -> JString;
        let Ok(text) = CString::new(text.replace('\0', " ")) else {
            return std::ptr::null_mut();
        };
        // SAFETY: the table entry at this position has this signature.
        let new: New = unsafe { std::mem::transmute(jni_function(env, NEW_STRING_UTF)) };
        unsafe { new(env, text.as_ptr()) }
    }

    /// `ProfilesActivity.nativeImport3dSen(bundle, profiles)`: converts the 3dSen profile bundle
    /// at `bundle` into `.3dn` profiles in the folder `profiles`, with the importer of File >
    /// Import from 3dSen. Returns its report: a first line "ok" or "failed", then what it printed.
    ///
    /// # Safety
    /// Called by the Java VM with a valid env and string references.
    #[no_mangle]
    pub unsafe extern "C" fn Java_io_github_nixxou_zaxnes_ProfilesActivity_nativeImport3dSen(
        env: JniEnv,
        _this: *mut c_void,
        bundle: JString,
        profiles: JString,
    ) -> JString {
        // SAFETY: the VM passes a valid env and string references (or null).
        let (bundle, profiles) = unsafe { (read_string(env, bundle), read_string(env, profiles)) };
        let report = match (bundle, profiles) {
            (Some(bundle), Some(profiles)) => {
                let result = std::panic::catch_unwind(|| {
                    let mut output = Vec::new();
                    let mut error = Vec::new();
                    let _ = std::fs::create_dir_all(&profiles);
                    let code = zax_app::three_d_sen_command::run_into(
                        std::path::Path::new(&bundle),
                        std::path::Path::new(&profiles),
                        &mut output,
                        &mut error,
                    )
                    .unwrap_or(1);
                    let status = if code == 0 { "ok" } else { "failed" };
                    format!(
                        "{status}\n{}{}",
                        String::from_utf8_lossy(&output),
                        String::from_utf8_lossy(&error)
                    )
                });
                result.unwrap_or_else(|_| "failed\nThe converter stopped unexpectedly.".to_owned())
            }
            _ => "failed\nNo bundle was given.".to_owned(),
        };
        // SAFETY: as above.
        unsafe { new_string(env, &report) }
    }
}

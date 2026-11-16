package io.github.nixxou.zaxnes;

import android.content.Intent;
import android.view.KeyEvent;

import org.libsdl.app.SDLActivity;

import java.io.File;
import java.io.FileOutputStream;

/**
 * Zaxnes on Android: SDL's activity, which loads the libraries below and runs {@code SDL_main} of
 * the last one ({@code libmain.so}, the crate zax-android).
 */
public class ZaxnesActivity extends SDLActivity {
    @Override
    protected String[] getLibraries() {
        // SDL (its hidapi, which SDL's Java controller code calls, is built into it), then the
        // app's library, whose SDL_main SDL runs.
        return new String[] { "SDL2", "main" };
    }

    /**
     * A frontend (ES-DE, a launcher) can start Zaxnes with a game, as an "open this file" request
     * whose data is the ROM: it then goes straight into that game ({@code --rom}). Started from
     * the home screen, Zaxnes opens on its ROM list as before.
     */
    @Override
    protected String[] getArguments() {
        new File(getExternalFilesDir(null), LAUNCH_REQUEST).delete(); // one left from a past run
        String rom = LaunchedRom.from(this, getIntent());
        return rom != null ? new String[] { "--rom", rom } : new String[0];
    }

    /**
     * A game sent while Zaxnes is already running (the activity is single-instance, so the
     * request comes here rather than starting it again). The running app is told through a small
     * file, which it checks about twice a second, and opens the game as from the ROM list.
     */
    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        String rom = LaunchedRom.from(this, intent);
        if (rom == null) return;
        File root = getExternalFilesDir(null);
        if (root == null) return;
        File part = new File(root, LAUNCH_REQUEST + ".part");
        try (FileOutputStream out = new FileOutputStream(part)) {
            out.write(rom.getBytes("UTF-8"));
        } catch (Exception e) {
            return;
        }
        part.renameTo(new File(root, LAUNCH_REQUEST)); // whole, so it is never read half-written
    }

    /**
     * Quitting closes the activity once Zaxnes' main has returned; the process goes too, so the
     * next game a frontend sends starts in a fresh one (the native side can't start twice in a
     * process). Battery saves are written when the game stops, before this.
     */
    @Override
    protected void onDestroy() {
        boolean leaving = isFinishing();
        super.onDestroy();
        if (leaving) System.exit(0);
    }

    /** Copies the profiles from the folder picked in Zaxnes Plus profiles (see ProfilesActivity). */
    @Override
    protected void onCreate(android.os.Bundle savedInstanceState) {
        ProfileFolder.importAll(this);
        super.onCreate(savedInstanceState);
    }

    /** Must match LAUNCH_REQUEST_FILE in zax-app's application.rs. */
    private static final String LAUNCH_REQUEST = "launch-request.txt";

    /**
     * Back, whatever device sends it, reaches the application as the keyboard's Back (the
     * in-game menu). A handheld's controller (Retroid Pocket) sends it as a gamepad key, which
     * SDL would otherwise take for the pad's Select.
     */
    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        if (event.getKeyCode() == KeyEvent.KEYCODE_BACK) {
            if (event.getAction() == KeyEvent.ACTION_DOWN && event.getRepeatCount() == 0) {
                SDLActivity.onNativeKeyDown(KeyEvent.KEYCODE_BACK);
            } else if (event.getAction() == KeyEvent.ACTION_UP) {
                SDLActivity.onNativeKeyUp(KeyEvent.KEYCODE_BACK);
            }
            return true;
        }
        return super.dispatchKeyEvent(event);
    }
}

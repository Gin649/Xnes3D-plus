package io.github.nixxou.zaxnes;

import android.app.Activity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * The ROM a frontend sent with its "open this file" request, as a file the emulator can read.
 *
 * <p>Frontends hand over a content address (from Android's file picker or their own file
 * provider) with permission to read it. The emulator core opens plain files, so the game is
 * copied into the app's cache, keeping its file name: battery saves and save states are named
 * after the ROM's file name, so they stay the same game's whichever way it was started. A plain
 * path (a {@code file://} address or a {@code ROM} text extra) is used as it is when readable.
 */
final class LaunchedRom {
    private LaunchedRom() {}

    static String from(Activity activity, Intent intent) {
        if (intent == null) return null;
        Uri uri = intent.getData();
        String path = null;
        if (uri == null) {
            String extra = intent.getStringExtra("ROM");
            if (extra == null || extra.isEmpty()) return null;
            if (extra.startsWith("content://")) uri = Uri.parse(extra);
            else path = extra;
        }
        if (uri != null && "file".equals(uri.getScheme())) path = uri.getPath();
        if (path != null) return new File(path).canRead() ? path : null;
        if (uri == null || !"content".equals(uri.getScheme())) return null;

        File folder = new File(activity.getCacheDir(), "launch");
        folder.mkdirs();
        File[] old = folder.listFiles();
        if (old != null) for (File f : old) f.delete(); // only the latest game is kept
        File out = new File(folder, fileName(activity, uri));
        try (InputStream in = activity.getContentResolver().openInputStream(uri);
             OutputStream o = new FileOutputStream(out)) {
            if (in == null) return null;
            byte[] buffer = new byte[64 * 1024];
            int n;
            while ((n = in.read(buffer)) > 0) o.write(buffer, 0, n);
        } catch (Exception e) {
            out.delete();
            return null;
        }
        return out.getAbsolutePath();
    }

    /** The ROM's own file name ("Super Mario Bros. (USA).nes"), so saves keep matching it. */
    private static String fileName(Activity activity, Uri uri) {
        String name = null;
        try (Cursor c = activity.getContentResolver().query(uri,
                new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null)) {
            if (c != null && c.moveToFirst()) name = c.getString(0);
        } catch (Exception ignored) {
        }
        if (name == null || name.isEmpty()) {
            name = uri.getLastPathSegment();
            if (name == null) name = "game.nes";
            int cut = Math.max(name.lastIndexOf('/'), name.lastIndexOf(':'));
            if (cut >= 0) name = name.substring(cut + 1);
        }
        name = name.replace('/', '_').replace('\\', '_');
        return name.isEmpty() ? "game.nes" : name;
    }
}

package io.github.nixxou.zaxnes;

import android.content.ContentResolver;
import android.content.Context;
import android.content.SharedPreferences;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * A folder of 3D profiles the player picks once (Zaxnes Plus profiles, ProfilesActivity), say on
 * the SD card. Android hides the app's own folder from file managers, so profiles are put in that
 * folder instead and copied into Zaxnes' profiles each time it starts: new ones, and ones that
 * changed (another size or a newer date). Nothing is ever deleted from either side.
 */
final class ProfileFolder {
    private static final String PREFS = "zaxnes_plus";
    private static final String KEY = "profiles_tree";
    private static final String EXTENSION = ".3dn";
    private static final int MAX_DEPTH = 3; // subfolders, a few levels down
    /** 3dSen's packed profiles (see ProfilesActivity's Import 3dSen profiles). */
    private static final String BUNDLE_EXTENSION = ".bundle";

    private ProfileFolder() {}

    static Uri get(Context c) {
        String s = prefs(c).getString(KEY, null);
        return s == null ? null : Uri.parse(s);
    }

    static void set(Context c, Uri tree) {
        prefs(c).edit().putString(KEY, tree == null ? null : tree.toString()).apply();
    }

    private static SharedPreferences prefs(Context c) {
        return c.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    /** Zaxnes' own profiles folder (the user data root, as SDL gives it to the native side). */
    static File target(Context c) {
        File root = c.getExternalFilesDir(null);
        return root == null ? null : new File(root, "profiles");
    }

    /** Copies new and changed profiles from the picked folder; the number copied. */
    static int importAll(Context c) {
        Uri tree = get(c);
        File to = target(c);
        if (tree == null || to == null) return 0;
        to.mkdirs();
        try {
            String root = DocumentsContract.getTreeDocumentId(tree);
            return walk(c, c.getContentResolver(), tree, root, to, 0);
        } catch (Exception e) {
            return 0; // the folder was removed, or access was taken back
        }
    }

    private static int walk(Context c, ContentResolver r, Uri tree, String docId, File to, int depth) {
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, docId);
        String[] cols = {
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_SIZE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        };
        int copied = 0;
        try (Cursor cur = r.query(children, cols, null, null, null)) {
            if (cur == null) return 0;
            while (cur.moveToNext()) {
                String id = cur.getString(0);
                String name = cur.getString(1);
                String mime = cur.getString(2);
                long size = cur.isNull(3) ? -1 : cur.getLong(3);
                long modified = cur.isNull(4) ? 0 : cur.getLong(4);
                if (DocumentsContract.Document.MIME_TYPE_DIR.equals(mime)) {
                    if (depth < MAX_DEPTH) copied += walk(c, r, tree, id, to, depth + 1);
                    continue;
                }
                if (name == null || !name.toLowerCase(java.util.Locale.ROOT).endsWith(EXTENSION)) continue;
                File out = new File(to, safe(name));
                if (out.isFile() && out.length() == size && out.lastModified() >= modified) continue;
                if (copy(r, DocumentsContract.buildDocumentUriUsingTree(tree, id), out)) {
                    if (modified > 0) out.setLastModified(modified);
                    copied++;
                }
            }
        } catch (Exception ignored) {
        }
        return copied;
    }

    /** A 3dSen bundle (3dSen ships its profiles packed in a Unity .bundle file). */
    static boolean isBundle(String name) {
        return name != null && name.toLowerCase(java.util.Locale.ROOT).endsWith(BUNDLE_EXTENSION);
    }

    /** Copies one profile that was opened or shared with Zaxnes Plus. */
    static boolean importOne(Context c, Uri uri, String name) {
        File to = target(c);
        if (to == null || name == null || !name.toLowerCase(java.util.Locale.ROOT).endsWith(EXTENSION)) return false;
        to.mkdirs();
        return copy(c.getContentResolver(), uri, new File(to, safe(name)));
    }

    static boolean copy(ContentResolver r, Uri from, File out) {
        File part = new File(out.getPath() + ".part");
        try (InputStream in = r.openInputStream(from); OutputStream o = new FileOutputStream(part)) {
            if (in == null) return false;
            byte[] buffer = new byte[64 * 1024];
            int n;
            while ((n = in.read(buffer)) > 0) o.write(buffer, 0, n);
        } catch (Exception e) {
            part.delete();
            return false;
        }
        out.delete();
        return part.renameTo(out);
    }

    static String safe(String name) {
        return name.replace('/', '_').replace('\\', '_');
    }
}

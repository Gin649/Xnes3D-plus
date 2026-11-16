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
    /**
     * 3dSen's own profiles come packed in one Unity bundle of its installation
     * ({@code <name>_Data/StreamingAssets/aa/StandaloneWindows64/defaultprofiles_assets_all_*.bundle}).
     * Put in the folder, it goes to {@link #IMPORT_FOLDER}, where the native side converts it into
     * .3dn profiles at the next start (zax-android's import_waiting_bundles); it is only taken again
     * when it changes.
     */
    private static final String BUNDLE_PREFIX = "defaultprofiles_assets_all_";
    private static final String BUNDLE_EXTENSION = ".bundle";
    static final String IMPORT_FOLDER = "import-3dsen";
    private static final String KEY_BUNDLE = "bundle_taken";

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
                if (isBundle(name)) {
                    String seen = name + "|" + size + "|" + modified;
                    if (seen.equals(prefs(c).getString(KEY_BUNDLE, null))) continue;
                    if (takeBundle(c, r, DocumentsContract.buildDocumentUriUsingTree(tree, id), name)) {
                        prefs(c).edit().putString(KEY_BUNDLE, seen).apply();
                        copied++;
                    }
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

    static boolean isBundle(String name) {
        if (name == null) return false;
        String n = name.toLowerCase(java.util.Locale.ROOT);
        return n.startsWith(BUNDLE_PREFIX) && n.endsWith(BUNDLE_EXTENSION);
    }

    /** True when a 3dSen bundle is waiting to be converted at the next start. */
    static boolean bundleWaiting(Context c) {
        File root = c.getExternalFilesDir(null);
        File[] waiting = root == null ? null : new File(root, IMPORT_FOLDER).listFiles();
        return waiting != null && waiting.length > 0;
    }

    /** Puts a 3dSen bundle where the native side converts it at the next start. */
    static boolean takeBundle(Context c, ContentResolver r, Uri from, String name) {
        File root = c.getExternalFilesDir(null);
        if (root == null) return false;
        File folder = new File(root, IMPORT_FOLDER);
        folder.mkdirs();
        return copy(r, from, new File(folder, safe(name)));
    }

    /** Copies one profile, or a 3dSen bundle, that was opened or shared with Zaxnes Plus. */
    static boolean importOne(Context c, Uri uri, String name) {
        if (isBundle(name)) return takeBundle(c, c.getContentResolver(), uri, name);
        File to = target(c);
        if (to == null || name == null || !name.toLowerCase(java.util.Locale.ROOT).endsWith(EXTENSION)) return false;
        to.mkdirs();
        return copy(c.getContentResolver(), uri, new File(to, safe(name)));
    }

    private static boolean copy(ContentResolver r, Uri from, File out) {
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

    private static String safe(String name) {
        return name.replace('/', '_').replace('\\', '_');
    }
}

package io.github.nixxou.zaxnes;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.DialogInterface;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.widget.Toast;

import java.io.File;
import java.util.ArrayList;
import java.util.List;

/**
 * The "Zaxnes profiles" screen (its own icon): imports 3dSen's profiles from the bundle file of a
 * 3dSen installation, picked anywhere on the device; gets the community profiles listed in
 * community/profiles.json (CommunityProfiles); picks the folder .3dn profiles are copied from
 * (ProfileFolder); and takes .3dn and .bundle files opened or shared from a file manager.
 *
 * <p>The Java here is compiled without lambdas (no LambdaMetafactory in android.jar), so the
 * listeners are written out in full.
 */
public class ProfilesActivity extends Activity {
    private static final int PICK_FOLDER = 1;
    private static final int PICK_BUNDLE = 2;

    /**
     * zax-android: converts the 3dSen bundle file at {@code bundle} into .3dn profiles in the
     * folder {@code profiles}, as File > Import from 3dSen does. Returns "ok" or "failed" on the
     * first line, then the importer's report.
     */
    private native String nativeImport3dSen(String bundle, String profiles);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        if (handleSharedFiles(getIntent())) return;
        showMenu();
    }

    private void showMenu() {
        final Uri tree = ProfileFolder.get(this);
        String[] items = {
            "Import 3dSen profiles…",
            "Get community profiles",
            tree == null ? "Choose a profiles folder…" : "Profiles folder: " + folderName(tree) + " (change)",
            "Copy from the profiles folder now",
            "Close",
        };
        new AlertDialog.Builder(this)
            .setTitle("Zaxnes 3D profiles")
            .setItems(items, new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int which) {
                    switch (which) {
                        case 0: pickBundle(); break;
                        case 1: getCommunityProfiles(); break;
                        case 2: pickFolder(); break;
                        case 3:
                            if (tree == null) {
                                Toast.makeText(ProfilesActivity.this, "Choose a profiles folder first.", Toast.LENGTH_LONG).show();
                                showMenu();
                            } else {
                                toast(ProfileFolder.importAll(ProfilesActivity.this));
                                finish();
                            }
                            break;
                        default: finish();
                    }
                }
            })
            .setOnCancelListener(new DialogInterface.OnCancelListener() {
                @Override
                public void onCancel(DialogInterface d) {
                    finish();
                }
            })
            .show();
    }

    // ---- 3dSen bundle -----------------------------------------------------------------------

    private void pickBundle() {
        new AlertDialog.Builder(this)
            .setTitle("Import 3dSen profiles")
            .setMessage("Choose the bundle file from your 3dSen installation. On your PC, in Steam: "
                + "3dSen › Manage › Browse local files, then the folder ending in _Data › "
                + "StreamingAssets › aa › StandaloneWindows64 › "
                + "defaultprofiles_assets_all_….bundle. Copy it to this device first.")
            .setPositiveButton("Choose file", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    i.addCategory(Intent.CATEGORY_OPENABLE);
                    i.setType("*/*");
                    try {
                        startActivityForResult(i, PICK_BUNDLE);
                    } catch (Exception e) {
                        Toast.makeText(ProfilesActivity.this, "No file picker on this device.", Toast.LENGTH_LONG).show();
                        finish();
                    }
                }
            })
            .setNegativeButton("Back", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    showMenu();
                }
            })
            .setOnCancelListener(new DialogInterface.OnCancelListener() {
                @Override
                public void onCancel(DialogInterface d) {
                    finish();
                }
            })
            .show();
    }

    /** Copies the bundle into the cache, converts it on a background thread, then reports. */
    private void convertBundle(final Uri uri, String name) {
        if (!ProfileFolder.isBundle(name)) {
            showResult("That isn't a 3dSen bundle", "Choose the file whose name starts with "
                + "defaultprofiles_assets_all_ and ends in .bundle.");
            return;
        }
        final File copy = new File(new File(getCacheDir(), "import-3dsen"), ProfileFolder.safe(name));
        final File profiles = ProfileFolder.target(this);
        if (profiles == null) {
            showResult("Import failed", "Zaxnes' storage isn't available right now.");
            return;
        }
        final AlertDialog working = new AlertDialog.Builder(this)
            .setTitle("Converting 3dSen profiles…")
            .setMessage("This can take a moment.")
            .setCancelable(false)
            .show();
        new Thread(new Runnable() {
            @Override
            public void run() {
                String report;
                try {
                    copy.getParentFile().mkdirs();
                    if (!ProfileFolder.copy(getContentResolver(), uri, copy)) {
                        report = "failed\nThe bundle file couldn't be read.";
                    } else {
                        System.loadLibrary("c++_shared");
                        System.loadLibrary("SDL2");
                        System.loadLibrary("main");
                        report = nativeImport3dSen(copy.getAbsolutePath(), profiles.getAbsolutePath());
                    }
                } catch (Throwable t) {
                    report = "failed\n" + t.getMessage();
                } finally {
                    copy.delete();
                }
                final String done = report == null ? "failed\nNo answer from the converter." : report;
                runOnUiThread(new Runnable() {
                    @Override
                    public void run() {
                        working.dismiss();
                        boolean ok = done.startsWith("ok");
                        showResult(ok ? "3dSen profiles imported" : "Import finished with problems", readable(done));
                    }
                });
            }
        }).start();
    }

    /** The importer's report without its first line and the folder paths. */
    private static String readable(String report) {
        StringBuilder out = new StringBuilder();
        String[] lines = report.split("\n");
        int shown = 0;
        for (int i = 1; i < lines.length; i++) {
            String line = lines[i].trim();
            if (line.isEmpty() || line.startsWith("3dSen:")) continue;
            if (shown == 12) {
                out.append("…");
                break;
            }
            out.append(line).append('\n');
            shown++;
        }
        String text = out.toString().trim();
        return (text.isEmpty() ? "Done." : text)
            + "\n\nThe profiles are used the next time a game starts in Zaxnes.";
    }

    private void showResult(String title, String message) {
        new AlertDialog.Builder(this)
            .setTitle(title)
            .setMessage(message)
            .setPositiveButton("OK", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    finish();
                }
            })
            .setOnCancelListener(new DialogInterface.OnCancelListener() {
                @Override
                public void onCancel(DialogInterface d) {
                    finish();
                }
            })
            .show();
    }

    // ---- community profiles -----------------------------------------------------------------

    private void getCommunityProfiles() {
        final AlertDialog working = new AlertDialog.Builder(this)
            .setTitle("Getting community profiles…")
            .setMessage("Each one is downloaded from its creator's own link.")
            .setCancelable(false)
            .show();
        new Thread(new Runnable() {
            @Override
            public void run() {
                final CommunityProfiles.Result r = CommunityProfiles.fetchAll(ProfilesActivity.this);
                runOnUiThread(new Runnable() {
                    @Override
                    public void run() {
                        working.dismiss();
                        if (r.error != null) {
                            showResult("Couldn't get community profiles", r.error);
                            return;
                        }
                        StringBuilder text = new StringBuilder();
                        if (r.listed == 0) {
                            text.append("The list has no profiles yet.");
                        } else {
                            if (r.addedCount > 0) text.append("New or updated, thanks to their makers:\n").append(r.added).append('\n');
                            if (r.upToDate > 0) text.append(r.upToDate).append(r.upToDate == 1 ? " profile was" : " profiles were").append(" already up to date.\n");
                            if (r.failed.length() > 0) text.append("\nCouldn't be downloaded (try again later):\n").append(r.failed);
                            text.append("\nThey are used the next time a game starts in Zaxnes.");
                        }
                        showResult(r.addedCount > 0 ? r.addedCount + " community profiles added" : "Community profiles", text.toString().trim());
                    }
                });
            }
        }).start();
    }

    // ---- profiles folder --------------------------------------------------------------------

    private void pickFolder() {
        Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        try {
            startActivityForResult(i, PICK_FOLDER);
        } catch (Exception e) {
            Toast.makeText(this, "No folder picker on this device.", Toast.LENGTH_LONG).show();
            finish();
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        Uri picked = resultCode == RESULT_OK && data != null ? data.getData() : null;
        if (picked == null) {
            finish();
            return;
        }
        if (requestCode == PICK_BUNDLE) {
            convertBundle(picked, name(picked));
        } else if (requestCode == PICK_FOLDER) {
            try {
                getContentResolver().takePersistableUriPermission(picked, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            } catch (Exception ignored) {
            }
            ProfileFolder.set(this, picked);
            toast(ProfileFolder.importAll(this));
            finish();
        } else {
            finish();
        }
    }

    // ---- files opened or shared from a file manager -----------------------------------------

    /** .3dn files are copied in; a .bundle is converted. True when there was something. */
    private boolean handleSharedFiles(Intent intent) {
        if (intent == null) return false;
        List<Uri> uris = new ArrayList<>();
        String action = intent.getAction();
        if (Intent.ACTION_VIEW.equals(action) && intent.getData() != null) {
            uris.add(intent.getData());
        } else if (Intent.ACTION_SEND.equals(action)) {
            Uri u = intent.getParcelableExtra(Intent.EXTRA_STREAM);
            if (u != null) uris.add(u);
        } else if (Intent.ACTION_SEND_MULTIPLE.equals(action)) {
            ArrayList<Uri> list = intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM);
            if (list != null) uris.addAll(list);
        }
        if (uris.isEmpty()) return false;
        for (Uri u : uris) {
            String n = name(u);
            if (ProfileFolder.isBundle(n)) {
                convertBundle(u, n);
                return true;
            }
        }
        int copied = 0;
        for (Uri u : uris) if (ProfileFolder.importOne(this, u, name(u))) copied++;
        toast(copied);
        finish();
        return true;
    }

    private String name(Uri uri) {
        try (Cursor c = getContentResolver().query(uri, new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null)) {
            if (c != null && c.moveToFirst()) return c.getString(0);
        } catch (Exception ignored) {
        }
        String last = uri.getLastPathSegment();
        if (last == null) return null;
        int cut = Math.max(last.lastIndexOf('/'), last.lastIndexOf(':'));
        return cut >= 0 ? last.substring(cut + 1) : last;
    }

    private static String folderName(Uri tree) {
        String id = tree.getLastPathSegment();
        if (id == null) return tree.toString();
        int colon = id.indexOf(':');
        String volume = colon > 0 ? id.substring(0, colon) : "";
        String path = colon >= 0 ? id.substring(colon + 1) : id;
        String where = "primary".equals(volume) ? "Internal storage" : "SD card";
        return path.isEmpty() ? where : where + " / " + path;
    }

    private void toast(int copied) {
        String text = copied == 0 ? "No new profiles to copy."
            : copied == 1 ? "1 profile copied into Zaxnes." : copied + " profiles copied into Zaxnes.";
        Toast.makeText(this, text, Toast.LENGTH_LONG).show();
    }
}

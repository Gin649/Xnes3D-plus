package io.github.nixxou.zaxnes;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.content.DialogInterface;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import android.provider.Settings;
import android.widget.Toast;

import java.io.File;
import java.io.FileOutputStream;
import java.util.ArrayList;
import java.util.List;

/**
 * The Android half of the rows of Options > Games and 3D profiles. Xnes3D+ has one icon: its
 * own menus send a message (zax-app's android_bridge.rs), ZaxnesActivity opens this screen with
 * the action asked, and this screen runs the picker or the download, then closes. It also takes
 * .3dn and .bundle files opened or shared from a file manager.
 *
 * <p>What it finds goes back to the running app through the user data folder: the ROM folder as
 * a path in roms-folder.txt, profiles into profile-inbox, which the app imports from on its own.
 *
 * <p>The Java here is compiled without lambdas (no LambdaMetafactory in android.jar), so the
 * listeners are written out in full.
 */
public class ProfilesActivity extends Activity {
    /** The extra ZaxnesActivity passes: the number of the action (AndroidAction in Rust). */
    static final String EXTRA_ACTION = "action";
    static final int ACTION_ROM_FOLDER = 1;
    static final int ACTION_IMPORT_3DSEN = 2;
    static final int ACTION_COMMUNITY = 3;
    static final int ACTION_PROFILES_FOLDER = 4;
    static final int ACTION_COPY_PROFILES = 5;
    static final int ACTION_SAVES_FOLDER = 6;
    /** Xnes3D+: Options > Check for updates. */
    static final int ACTION_UPDATES = 7;
    /** Xnes3D+: the box art and information media-request.txt asks for (see GameMedia). */
    static final int ACTION_MEDIA = 8;
    static final int LAST_ACTION = ACTION_MEDIA;

    /** Where the saves folder is kept; ZaxnesActivity passes it to the native side as --saves. */
    static final String PREFS = "zaxnes_plus";
    static final String KEY_SAVES = "saves_folder";

    /** Must match ROMS_FOLDER_FILE in zax-app's android_bridge.rs. */
    static final String ROMS_FOLDER_FILE = "roms-folder.txt";
    /** Must match SAVES_FOLDER_FILE in zax-app's android_bridge.rs. */
    static final String SAVES_FOLDER_FILE = "saves-folder.txt";

    private static final int PICK_FOLDER = 1;
    private static final int PICK_BUNDLE = 2;
    private static final int PICK_ROM_FOLDER = 3;
    private static final int ASK_STORAGE = 4;
    private static final int PICK_SAVES_FOLDER = 5;

    /** Which folder the storage permission is being asked for: the ROMs' or the saves'. */
    private boolean forSaves;

    /** Waiting for the player to come back from the "All files access" settings page. */
    private boolean waitingForAllFiles;

    /** Xnes3D+: the update waiting for the "Install unknown apps" switch, if any. */
    private AppUpdate.Release waitingToInstall;

    /**
     * zax-android: converts the 3dSen bundle file at {@code bundle} into .3dn profiles in the
     * folder {@code profiles}, as File > Import from 3dSen does. Returns "ok" or "failed" on the
     * first line, then the importer's report.
     */
    private native String nativeImport3dSen(String bundle, String profiles);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // Xnes3D+: Android's installer reporting on an update.
        if (AppUpdate.STATUS_ACTION.equals(getIntent().getAction())) {
            installStatus(getIntent());
            return;
        }
        if (savedInstanceState != null) return; // a picker is already open
        if (handleSharedFiles(getIntent())) return;
        switch (getIntent().getIntExtra(EXTRA_ACTION, 0)) {
            case ACTION_ROM_FOLDER: chooseRomFolder(); break;
            case ACTION_IMPORT_3DSEN: pickBundle(); break;
            case ACTION_COMMUNITY: getCommunityProfiles(); break;
            case ACTION_PROFILES_FOLDER: pickFolder(); break;
            case ACTION_COPY_PROFILES: copyNow(); break;
            case ACTION_SAVES_FOLDER: forSaves = true; chooseRomFolder(); break;
            case ACTION_UPDATES: checkForUpdates(); break;
            case ACTION_MEDIA: getMedia(); break;
            default: finish();
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (waitingToInstall != null) {
            AppUpdate.Release release = waitingToInstall;
            waitingToInstall = null;
            if (canInstall()) {
                downloadAndInstall(release);
            } else {
                showResult("Update not installed",
                    "Xnes3D+ needs \"Install unknown apps\" to install its own updates. "
                        + "You can try again from Options > Games and 3D profiles.");
            }
        }
        if (waitingForAllFiles) {
            waitingForAllFiles = false;
            if (canReadStorage()) {
                openRomFolderPicker();
            } else {
                showResult("No access to your files",
                    "Without \"All files access\", Xnes3D+ can't read your ROM folder. "
                        + "You can try again from Options > Games and 3D profiles.");
            }
        }
    }

    private AlertDialog.Builder dialog() {
        return new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert);
    }

    private File inbox() {
        File to = ProfileFolder.inbox(this);
        if (to != null) to.mkdirs();
        return to;
    }

    // ---- ROM folder -------------------------------------------------------------------------

    /** Whether the app may read any folder of the shared storage. */
    private boolean canReadStorage() {
        if (Build.VERSION.SDK_INT >= 30) return Environment.isExternalStorageManager();
        return checkSelfPermission(Manifest.permission.READ_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED;
    }

    private void chooseRomFolder() {
        if (canReadStorage()) {
            openRomFolderPicker();
            return;
        }
        if (Build.VERSION.SDK_INT < 30) {
            requestPermissions(new String[] { Manifest.permission.READ_EXTERNAL_STORAGE }, ASK_STORAGE);
            return;
        }
        dialog()
            .setTitle("Allow access to your games")
            .setMessage("To list the games in your own ROM folder, without copying them anywhere, "
                + "Xnes3D+ needs \"All files access\". It only reads the folder you choose next.\n\n"
                + "Turn it on for Xnes3D+ on the next screen, then come back.")
            .setPositiveButton("Continue", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    waitingForAllFiles = true;
                    try {
                        startActivity(new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                            Uri.parse("package:" + getPackageName())));
                    } catch (Exception e) {
                        try {
                            startActivity(new Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION));
                        } catch (Exception again) {
                            waitingForAllFiles = false;
                            showResult("Can't open the setting", "Allow \"All files access\" for Xnes3D+ "
                                + "in Android's settings (Apps > Xnes3D+), then try again.");
                        }
                    }
                }
            })
            .setNegativeButton("Cancel", finishOnClick())
            .setOnCancelListener(finishOnCancel())
            .show();
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(requestCode, permissions, results);
        if (requestCode != ASK_STORAGE) return;
        if (canReadStorage()) {
            openRomFolderPicker();
        } else {
            showResult("No access to your files", "Without it, Xnes3D+ can't read your ROM folder.");
        }
    }

    private void openRomFolderPicker() {
        Toast.makeText(this, forSaves
            ? "Choose the folder for your saves and save states."
            : "Choose the folder your NES games are in.", Toast.LENGTH_LONG).show();
        Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        try {
            startActivityForResult(i, forSaves ? PICK_SAVES_FOLDER : PICK_ROM_FOLDER);
        } catch (Exception e) {
            showResult("No folder picker", "This device has no folder picker.");
        }
    }

    /**
     * The real path of a folder picked in Android's folder picker: "primary:ROMs/nes" is
     * /storage/emulated/0/ROMs/nes, "1234-5678:nes" is /storage/1234-5678/nes (an SD card).
     */
    static String pathOf(Uri tree) {
        String id;
        try {
            id = DocumentsContract.getTreeDocumentId(tree);
        } catch (Exception e) {
            return null;
        }
        if (id == null) return null;
        if (id.startsWith("raw:")) return id.substring(4);
        int colon = id.indexOf(':');
        if (colon < 0) return null;
        String volume = id.substring(0, colon);
        String path = id.substring(colon + 1);
        String root = "primary".equalsIgnoreCase(volume)
            ? Environment.getExternalStorageDirectory().getPath()
            : "/storage/" + volume;
        return path.isEmpty() ? root : root + "/" + path;
    }

    /** Tells the running app its new ROM folder (it checks for the file twice a second). */
    private boolean sendRomFolder(String path) {
        return sendAnswer(ROMS_FOLDER_FILE, path);
    }

    /** Writes {@code text} to {@code name} in the user data folder, whole, for the running app. */
    private boolean sendAnswer(String name, String text) {
        File root = getExternalFilesDir(null);
        if (root == null) return false;
        File part = new File(root, name + ".part");
        try (FileOutputStream out = new FileOutputStream(part)) {
            out.write(text.getBytes("UTF-8"));
        } catch (Exception e) {
            return false;
        }
        return part.renameTo(new File(root, name));
    }

    // ---- 3dSen bundle -----------------------------------------------------------------------

    private void pickBundle() {
        dialog()
            .setTitle("Import 3dSen profiles")
            .setMessage("Choose the bundle file from your 3dSen installation. On your PC, in Steam: "
                + "3dSen > Manage > Browse local files, then the folder ending in _Data > "
                + "StreamingAssets > aa > StandaloneWindows64 > "
                + "defaultprofiles_assets_all_....bundle. Copy it to this device first.")
            .setPositiveButton("Choose file", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    i.addCategory(Intent.CATEGORY_OPENABLE);
                    i.setType("*/*");
                    try {
                        startActivityForResult(i, PICK_BUNDLE);
                    } catch (Exception e) {
                        showResult("No file picker", "This device has no file picker.");
                    }
                }
            })
            .setNegativeButton("Cancel", finishOnClick())
            .setOnCancelListener(finishOnCancel())
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
        final File profiles = inbox();
        if (profiles == null) {
            showResult("Import failed", "Xnes3D+'s storage isn't available right now.");
            return;
        }
        final AlertDialog working = dialog()
            .setTitle("Converting 3dSen profiles...")
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
                out.append("...");
                break;
            }
            out.append(line).append('\n');
            shown++;
        }
        String text = out.toString().trim();
        return (text.isEmpty() ? "Done." : text) + "\n\nYour game list shows them in a moment.";
    }

    // ---- community profiles -----------------------------------------------------------------

    private void getCommunityProfiles() {
        final File profiles = inbox();
        if (profiles == null) {
            showResult("Couldn't get community profiles", "Xnes3D+'s storage isn't available right now.");
            return;
        }
        final AlertDialog working = dialog()
            .setTitle("Getting community profiles...")
            .setMessage("Each one is downloaded from its creator's own link.")
            .setCancelable(false)
            .show();
        new Thread(new Runnable() {
            @Override
            public void run() {
                final CommunityProfiles.Result r = CommunityProfiles.fetchAll(ProfilesActivity.this, profiles);
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
                        }
                        showResult(r.addedCount > 0 ? r.addedCount + " community profiles added" : "Community profiles", text.toString().trim());
                    }
                });
            }
        }).start();
    }

    // ---- box art and information (Xnes3D+) -------------------------------------------------

    private void getMedia() {
        final GameMedia.Request request = GameMedia.take(this);
        if (request == null || request.roms.isEmpty()) {
            showResult("No games", "There are no games in your list to get box art for.");
            return;
        }
        if ("pick".equals(request.source)) {
            pickEntry(request.roms.get(0));
            return;
        }
        final boolean esde = "esde".equals(request.source);
        final AlertDialog working = dialog()
            .setTitle(esde ? "Copying from ES-DE..." : "Getting box art and game info...")
            .setMessage("Starting...")
            .setCancelable(false)
            .show();
        new Thread(new Runnable() {
            @Override
            public void run() {
                GameMedia.Progress progress = new GameMedia.Progress() {
                    @Override
                    public void at(final int done, final int total, final String game) {
                        runOnUiThread(new Runnable() {
                            @Override
                            public void run() {
                                working.setMessage((done + 1) + " of " + total + "\n" + game);
                            }
                        });
                    }
                };
                final GameMedia.Result r = esde
                    ? GameMedia.fromEsDe(ProfilesActivity.this, request, progress)
                    : GameMedia.fromLaunchBox(ProfilesActivity.this, request, progress);
                GameMedia.done(ProfilesActivity.this);
                runOnUiThread(new Runnable() {
                    @Override
                    public void run() {
                        working.dismiss();
                        if (r.error != null) {
                            showResult("Box art not updated", r.error);
                            return;
                        }
                        StringBuilder text = new StringBuilder();
                        text.append(r.found).append(r.found == 1 ? " game" : " games").append(" found, ")
                            .append(r.pictures).append(r.pictures == 1 ? " picture" : " pictures").append(" saved.");
                        if (r.missing > 0) {
                            text.append("\n\nNot found (press R2 on a game, then Y, to pick it by hand):\n").append(r.notFound);
                        }
                        showResult("Box art and game info", text.toString().trim());
                    }
                });
            }
        }).start();
    }

    /** Lets the player pick the LaunchBox entry of one game by searching the index. */
    private void pickEntry(final String rom) {
        final AlertDialog loading = dialog()
            .setTitle("Loading the LaunchBox list...")
            .setMessage("One moment.")
            .setCancelable(false)
            .show();
        new Thread(new Runnable() {
            @Override
            public void run() {
                List<GameMedia.Game> index = null;
                try {
                    index = GameMedia.index(ProfilesActivity.this);
                } catch (Exception ignored) {
                    // said below
                }
                final List<GameMedia.Game> found = index;
                runOnUiThread(new Runnable() {
                    @Override
                    public void run() {
                        loading.dismiss();
                        if (found == null) {
                            showResult("No LaunchBox list", "The list couldn't be downloaded. Check the connection and try again.");
                            return;
                        }
                        showSearch(rom, found);
                    }
                });
            }
        }).start();
    }

    private void showSearch(final String rom, final List<GameMedia.Game> index) {
        final android.widget.LinearLayout layout = new android.widget.LinearLayout(this);
        layout.setOrientation(android.widget.LinearLayout.VERTICAL);
        final android.widget.EditText query = new android.widget.EditText(this);
        String start = GameMedia.key(rom).replaceAll("\\([^)]*\\)", "").replaceAll("\\[[^\\]]*\\]", "").trim();
        query.setText(start);
        query.setSingleLine(true);
        layout.addView(query);
        final android.widget.ListView list = new android.widget.ListView(this);
        final List<GameMedia.Game> shown = new ArrayList<>(GameMedia.search(index, start, 40));
        final android.widget.ArrayAdapter<GameMedia.Game> adapter =
            new android.widget.ArrayAdapter<>(this, android.R.layout.simple_list_item_1, shown);
        list.setAdapter(adapter);
        layout.addView(list, new android.widget.LinearLayout.LayoutParams(
            android.widget.LinearLayout.LayoutParams.MATCH_PARENT, 600));
        final AlertDialog picker = dialog()
            .setTitle("Which game is " + GameMedia.key(rom) + "?")
            .setView(layout)
            .setNegativeButton("Cancel", finishOnClick())
            .setOnCancelListener(finishOnCancel())
            .show();
        query.addTextChangedListener(new android.text.TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int a, int b, int c) {}

            @Override
            public void onTextChanged(CharSequence s, int a, int b, int c) {}

            @Override
            public void afterTextChanged(android.text.Editable s) {
                shown.clear();
                shown.addAll(GameMedia.search(index, s.toString(), 40));
                adapter.notifyDataSetChanged();
            }
        });
        list.setOnItemClickListener(new android.widget.AdapterView.OnItemClickListener() {
            @Override
            public void onItemClick(android.widget.AdapterView<?> parent, android.view.View view, int position, long id) {
                final GameMedia.Game game = shown.get(position);
                picker.dismiss();
                final AlertDialog working = dialog()
                    .setTitle("Getting " + game.name + "...")
                    .setMessage("Its box art, pictures and information.")
                    .setCancelable(false)
                    .show();
                new Thread(new Runnable() {
                    @Override
                    public void run() {
                        GameMedia.write(ProfilesActivity.this, rom, game);
                        GameMedia.done(ProfilesActivity.this);
                        runOnUiThread(new Runnable() {
                            @Override
                            public void run() {
                                working.dismiss();
                                finish();
                            }
                        });
                    }
                }).start();
            }
        });
    }

    // ---- updates (Xnes3D+) ------------------------------------------------------------------

    private void checkForUpdates() {
        final String installed = AppUpdate.installed(this);
        final AlertDialog working = dialog()
            .setTitle("Checking for updates...")
            .setMessage("Asking GitHub for the newest Xnes3D+.")
            .setCancelable(false)
            .show();
        new Thread(new Runnable() {
            @Override
            public void run() {
                AppUpdate.Release found = null;
                String error = null;
                try {
                    found = AppUpdate.latest();
                } catch (Exception e) {
                    error = e.getMessage() == null ? "Check your internet connection." : e.getMessage();
                }
                final AppUpdate.Release release = found;
                final String problem = error;
                runOnUiThread(new Runnable() {
                    @Override
                    public void run() {
                        working.dismiss();
                        if (release == null) {
                            showResult("Couldn't check for updates", problem);
                        } else if (!AppUpdate.newer(release.version, installed)) {
                            showResult("Xnes3D+ is up to date", "You have the newest version, v" + installed + ".");
                        } else {
                            offer(release, installed);
                        }
                    }
                });
            }
        }).start();
    }

    private void offer(final AppUpdate.Release release, String installed) {
        String notes = release.notes == null ? "" : release.notes.trim();
        if (notes.length() > 600) notes = notes.substring(0, 600) + "...";
        dialog()
            .setTitle("Xnes3D+ v" + release.version + " is available")
            .setMessage("You have v" + installed + "." + (notes.isEmpty() ? "" : "\n\n" + notes))
            .setPositiveButton("Install", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    startInstall(release);
                }
            })
            .setNegativeButton("Not now", finishOnClick())
            .setOnCancelListener(finishOnCancel())
            .show();
    }

    /** Whether Android lets Xnes3D+ hand an APK to its installer ("Install unknown apps"). */
    private boolean canInstall() {
        return Build.VERSION.SDK_INT < 26 || getPackageManager().canRequestPackageInstalls();
    }

    private void startInstall(final AppUpdate.Release release) {
        if (canInstall()) {
            downloadAndInstall(release);
            return;
        }
        dialog()
            .setTitle("One switch first")
            .setMessage("To install its own updates, Xnes3D+ needs \"Install unknown apps\". "
                + "Turn it on for Xnes3D+ on the next screen, then come back.")
            .setPositiveButton("Open settings", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    waitingToInstall = release;
                    try {
                        startActivity(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                            Uri.parse("package:" + getPackageName())));
                    } catch (Exception e) {
                        waitingToInstall = null;
                        showResult("Update not installed", "Android's settings page couldn't be opened.");
                    }
                }
            })
            .setNegativeButton("Not now", finishOnClick())
            .setOnCancelListener(finishOnCancel())
            .show();
    }

    private void downloadAndInstall(final AppUpdate.Release release) {
        final File apk = new File(new File(getCacheDir(), "update"), "Xnes3D-plus.apk");
        final AlertDialog working = dialog()
            .setTitle("Downloading Xnes3D+ v" + release.version)
            .setMessage("0%")
            .setCancelable(false)
            .show();
        new Thread(new Runnable() {
            @Override
            public void run() {
                String error = null;
                try {
                    AppUpdate.download(release, apk, new AppUpdate.Progress() {
                        @Override
                        public void at(final int percent) {
                            runOnUiThread(new Runnable() {
                                @Override
                                public void run() {
                                    working.setMessage(percent + "%");
                                }
                            });
                        }
                    });
                    AppUpdate.install(ProfilesActivity.this, apk);
                } catch (Exception e) {
                    error = e.getMessage() == null ? "Check your internet connection." : e.getMessage();
                }
                final String problem = error;
                runOnUiThread(new Runnable() {
                    @Override
                    public void run() {
                        working.dismiss();
                        if (problem != null) {
                            showResult("Update not installed", problem);
                        }
                        // Otherwise Android's installer takes over; its answer comes back here.
                    }
                });
            }
        }).start();
    }

    /** Android's installer reporting on an update: asks the player to confirm, or tells why not. */
    private void installStatus(Intent intent) {
        int status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            Intent confirm = (Intent) intent.getParcelableExtra(Intent.EXTRA_INTENT);
            if (confirm != null) {
                try {
                    startActivity(confirm);
                } catch (Exception e) {
                    showResult("Update not installed", "Android's installer couldn't be opened.");
                    return;
                }
            }
            finish();
        } else if (status == PackageInstaller.STATUS_SUCCESS) {
            finish();
        } else if (status == PackageInstaller.STATUS_FAILURE_ABORTED) {
            finish(); // the player said no
        } else {
            String message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
            showResult("Update not installed", message == null ? "Android refused the update." : message);
        }
    }

    // ---- profiles folder --------------------------------------------------------------------

    private void pickFolder() {
        Uri tree = ProfileFolder.get(this);
        String now = tree == null ? "No folder chosen yet." : "Now: " + folderName(tree) + ".";
        dialog()
            .setTitle("Profiles folder")
            .setMessage("Choose a folder of .3dn profiles, say on your SD card. Xnes3D+ copies new "
                + "and changed profiles from it each time it starts.\n\n" + now)
            .setPositiveButton("Choose folder", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
                    i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
                    try {
                        startActivityForResult(i, PICK_FOLDER);
                    } catch (Exception e) {
                        showResult("No folder picker", "This device has no folder picker.");
                    }
                }
            })
            .setNegativeButton("Cancel", finishOnClick())
            .setOnCancelListener(finishOnCancel())
            .show();
    }

    private void copyNow() {
        if (ProfileFolder.get(this) == null) {
            pickFolder();
            return;
        }
        toast(ProfileFolder.importAll(this, inbox()));
        finish();
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
            toast(ProfileFolder.importAll(this, inbox()));
            finish();
        } else if (requestCode == PICK_SAVES_FOLDER) {
            String path = pathOf(picked);
            File folder = path == null ? null : new File(path);
            if (folder == null || !folder.isDirectory() || !folder.canWrite()) {
                showResult("That folder can't be used", "Choose a folder Xnes3D+ can write to, on the internal storage or an SD card.");
            } else {
                getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY_SAVES, path).apply();
                sendAnswer(SAVES_FOLDER_FILE, path); // its row in the options shows it at once
                showResult("Saves folder set", "Your saves and save states go to " + folderName(picked)
                    + " (in its saves and states folders) from the next time Xnes3D+ starts. "
                    + "The ones you have now are copied there then.");
            }
        } else if (requestCode == PICK_ROM_FOLDER) {
            String path = pathOf(picked);
            if (path == null || !new File(path).isDirectory()) {
                showResult("That folder can't be used", "Choose a folder on the internal storage or an SD card.");
            } else if (!sendRomFolder(path)) {
                showResult("ROM folder not saved", "Xnes3D+'s storage isn't available right now.");
            } else {
                Toast.makeText(this, "ROM folder: " + folderName(picked), Toast.LENGTH_LONG).show();
                finish();
            }
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
        File to = inbox();
        int copied = 0;
        if (to != null) for (Uri u : uris) if (ProfileFolder.importOne(this, u, name(u), to)) copied++;
        toast(copied);
        finish();
        return true;
    }

    // ---- small helpers ----------------------------------------------------------------------

    private void showResult(String title, String message) {
        dialog()
            .setTitle(title)
            .setMessage(message)
            .setPositiveButton("OK", finishOnClick())
            .setOnCancelListener(finishOnCancel())
            .show();
    }

    private DialogInterface.OnClickListener finishOnClick() {
        return new DialogInterface.OnClickListener() {
            @Override
            public void onClick(DialogInterface d, int w) {
                finish();
            }
        };
    }

    private DialogInterface.OnCancelListener finishOnCancel() {
        return new DialogInterface.OnCancelListener() {
            @Override
            public void onCancel(DialogInterface d) {
                finish();
            }
        };
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
            : copied == 1 ? "1 profile added to Xnes3D+." : copied + " profiles added to Xnes3D+.";
        Toast.makeText(this, text, Toast.LENGTH_LONG).show();
    }

    /**
     * Opens this screen for {@code action} (one of the ACTION_ numbers), on the same screen as
     * {@code from}: on a two-screen device (AYN Thor) Android could otherwise put it, and the dim
     * layer of its dialogs, on the other screen.
     */
    static void open(Activity from, int action) {
        Intent i = new Intent(from, ProfilesActivity.class);
        i.putExtra(EXTRA_ACTION, action);
        if (Build.VERSION.SDK_INT >= 26) {
            try {
                android.app.ActivityOptions options = android.app.ActivityOptions.makeBasic();
                options.setLaunchDisplayId(from.getWindowManager().getDefaultDisplay().getDisplayId());
                from.startActivity(i, options.toBundle());
                return;
            } catch (Exception ignored) {
                // a display that refuses it: open it as before
            }
        }
        from.startActivity(i);
    }

    /** Leaves no dim layer behind: the dialogs go with the screen, whichever way it closes. */
    @Override
    protected void onDestroy() {
        super.onDestroy();
        getWindow().clearFlags(android.view.WindowManager.LayoutParams.FLAG_DIM_BEHIND);
    }
}

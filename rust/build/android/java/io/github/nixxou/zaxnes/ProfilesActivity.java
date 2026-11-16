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

import java.util.ArrayList;
import java.util.List;

/**
 * The "Zaxnes profiles" icon: picks the folder 3D profiles are copied from (ProfileFolder) and
 * copies them now. It also takes .3dn files opened or shared from a file manager.
 */
public class ProfilesActivity extends Activity {
    private static final int PICK_FOLDER = 1;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        if (importFiles(getIntent())) return;
        showMenu();
    }

    private void showMenu() {
        Uri tree = ProfileFolder.get(this);
        String where = tree == null ? "No folder picked yet." : "Folder: " + folderName(tree);
        // written out in full: the APK's Java is compiled without lambdas (no LambdaMetafactory in android.jar)
        new AlertDialog.Builder(this)
            .setTitle("3D profiles")
            .setMessage(where + "\n\nNew and changed .3dn profiles in this folder (and its subfolders) are copied into Zaxnes each time it starts. Put 3dSen's defaultprofiles_assets_all_....bundle file here too, and Zaxnes converts all its profiles at the next start.")
            .setPositiveButton(tree == null ? "Choose folder" : "Change folder", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    pickFolder();
                }
            })
            .setNeutralButton("Copy now", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface d, int w) {
                    toast(ProfileFolder.importAll(ProfilesActivity.this));
                    finish();
                }
            })
            .setNegativeButton("Close", new DialogInterface.OnClickListener() {
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
        if (requestCode != PICK_FOLDER) return;
        if (resultCode == RESULT_OK && data != null && data.getData() != null) {
            Uri tree = data.getData();
            try {
                getContentResolver().takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            } catch (Exception ignored) {
            }
            ProfileFolder.set(this, tree);
            toast(ProfileFolder.importAll(this));
        }
        finish();
    }

    /** .3dn files opened or shared with Zaxnes: copied in, then this closes. */
    private boolean importFiles(Intent intent) {
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
            : copied == 1 ? "1 file copied into Zaxnes." : copied + " files copied into Zaxnes.";
        if (ProfileFolder.bundleWaiting(this)) text += " Your 3dSen profiles are converted the next time Zaxnes starts.";
        Toast.makeText(this, text, Toast.LENGTH_LONG).show();
    }
}

package io.github.nixxou.zaxnes;

import android.content.Context;
import android.content.SharedPreferences;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLDecoder;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Get community profiles: downloads the community 3D profiles listed in Zaxnes Plus'
 * community/profiles.json, each from its creator's own link, into Zaxnes' profiles. A profile is
 * fetched again only when its link or version changes. Runs on a background thread.
 */
final class CommunityProfiles {
    static final String LIST_URL =
        "https://raw.githubusercontent.com/Gin649/zaxnes-plus/main/community/profiles.json";
    private static final String PREFS = "zaxnes_plus_community";

    private CommunityProfiles() {}

    /** What a run did, for the dialog. */
    static final class Result {
        final StringBuilder added = new StringBuilder();
        final StringBuilder failed = new StringBuilder();
        int addedCount;
        int upToDate;
        int listed;
        String error;
    }

    static Result fetchAll(Context c) {
        return fetchAll(c, ProfileFolder.target(c));
    }

    /** The same, saving the profiles in {@code profiles}. */
    static Result fetchAll(Context c, File profiles) {
        Result result = new Result();
        if (profiles == null) {
            result.error = "Zaxnes' storage isn't available right now.";
            return result;
        }
        profiles.mkdirs();
        JSONArray list;
        try {
            list = new JSONObject(new String(download(LIST_URL), "UTF-8")).getJSONArray("profiles");
        } catch (Exception e) {
            result.error = "The list of community profiles couldn't be downloaded. Check the connection and try again.";
            return result;
        }
        SharedPreferences done = c.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
        result.listed = list.length();
        for (int i = 0; i < list.length(); i++) {
            JSONObject p = list.optJSONObject(i);
            if (p == null) continue;
            String game = p.optString("game", "A profile");
            String creator = p.optString("creator", "");
            String url = p.optString("url", "");
            if (url.isEmpty()) continue;
            String stamp = url + "|" + p.optString("version", "");
            String key = "got:" + game;
            if (stamp.equals(done.getString(key, null))) {
                result.upToDate++;
                continue;
            }
            String credit = creator.isEmpty() ? game : game + " (by " + creator + ")";
            try {
                int saved = save(download(directLink(url)), p.optString("file", ""), url, game, profiles);
                if (saved == 0) throw new Exception("no .3dn profile in the file");
                done.edit().putString(key, stamp).apply();
                result.addedCount++;
                result.added.append("• ").append(credit).append('\n');
            } catch (Exception e) {
                result.failed.append("• ").append(credit).append('\n');
            }
        }
        return result;
    }

    /** Writes a .3dn, or the .3dn files of a .zip; the number written. */
    private static int save(byte[] data, String fileName, String url, String game, File profiles) throws Exception {
        if (data.length > 4 && data[0] == 'P' && data[1] == 'K') {
            int count = 0;
            try (ZipInputStream zip = new ZipInputStream(new java.io.ByteArrayInputStream(data))) {
                ZipEntry entry;
                while ((entry = zip.getNextEntry()) != null) {
                    String name = new File(entry.getName()).getName();
                    if (entry.isDirectory() || !name.toLowerCase(java.util.Locale.ROOT).endsWith(".3dn")) continue;
                    write(zip, new File(profiles, ProfileFolder.safe(name)));
                    count++;
                }
            }
            return count;
        }
        String name = fileName;
        if (name.isEmpty()) name = nameFromUrl(url);
        if (!name.toLowerCase(java.util.Locale.ROOT).endsWith(".3dn")) name = game + ".3dn";
        try (InputStream in = new java.io.ByteArrayInputStream(data)) {
            write(in, new File(profiles, ProfileFolder.safe(name)));
        }
        return 1;
    }

    private static void write(InputStream in, File out) throws Exception {
        File part = new File(out.getPath() + ".part");
        try (OutputStream o = new FileOutputStream(part)) {
            byte[] buffer = new byte[64 * 1024];
            int n;
            while ((n = in.read(buffer)) > 0) o.write(buffer, 0, n);
        }
        out.delete();
        if (!part.renameTo(out)) throw new Exception("couldn't save " + out.getName());
    }

    /** Dropbox and Google Drive share links, turned into links to the file itself. */
    static String directLink(String url) {
        if (url.contains("dropbox.com/")) {
            String u = url.replace("?dl=0", "?dl=1").replace("&dl=0", "&dl=1");
            if (!u.contains("dl=1")) u += (u.contains("?") ? "&" : "?") + "dl=1";
            return u;
        }
        Matcher drive = Pattern.compile("drive\\.google\\.com/file/d/([^/?]+)").matcher(url);
        if (drive.find()) return "https://drive.google.com/uc?export=download&id=" + drive.group(1);
        return url;
    }

    private static String nameFromUrl(String url) {
        try {
            String path = new URL(url).getPath();
            String name = URLDecoder.decode(path.substring(path.lastIndexOf('/') + 1), "UTF-8");
            return name;
        } catch (Exception e) {
            return "";
        }
    }

    /** The bytes at url, following redirects (also from http to https), up to 32 MB. */
    private static byte[] download(String url) throws Exception {
        for (int hop = 0; hop < 6; hop++) {
            HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
            c.setConnectTimeout(15000);
            c.setReadTimeout(30000);
            c.setInstanceFollowRedirects(false);
            c.setRequestProperty("User-Agent", "ZaxnesPlus");
            int code = c.getResponseCode();
            if (code >= 300 && code < 400) {
                String next = c.getHeaderField("Location");
                c.disconnect();
                if (next == null) throw new Exception("redirect with no address");
                url = new URL(new URL(url), next).toString();
                continue;
            }
            if (code != 200) {
                c.disconnect();
                throw new Exception("HTTP " + code);
            }
            try (InputStream in = c.getInputStream()) {
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                byte[] buffer = new byte[64 * 1024];
                int n;
                while ((n = in.read(buffer)) > 0) {
                    out.write(buffer, 0, n);
                    if (out.size() > 32 * 1024 * 1024) throw new Exception("too big");
                }
                return out.toByteArray();
            } finally {
                c.disconnect();
            }
        }
        throw new Exception("too many redirects");
    }
}

package io.github.nixxou.zaxnes;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.net.Uri;
import android.os.Environment;
import android.util.Xml;

import org.json.JSONArray;
import org.json.JSONObject;
import org.xmlpull.v1.XmlPullParser;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Xnes3D+: the box art, pictures and information of the games of the list, which the game list
 * and its Game info screen show (zax-app's game_media.rs reads them).
 *
 * <p>The Rust side writes what it wants into media-request.txt ("source=launchbox", "esde" or
 * "pick", then a "rom=" line per game) and asks for it. This side fills, for each game, the
 * folder media/[ROM file name without its extension]/ of the user data with box.png, shot.png,
 * cart.png and info.txt, then writes media-done.txt so the running app reads them again.
 *
 * <p>The sources: the Xnes3D+ index of the LaunchBox Games Database (community/launchbox-nes.json,
 * NES and Famicom games only, rebuilt every week by a workflow of the repository), with the
 * pictures from LaunchBox's image server; or what ES-DE already downloaded. Every picture is
 * converted to a PNG no larger than it needs to be, since the Rust side only reads PNG.
 *
 * <p>Written without lambdas, like the rest of the fork's Java.
 */
final class GameMedia {
    static final String INDEX_URL =
        "https://raw.githubusercontent.com/Gin649/Xnes3D-plus/main/community/launchbox-nes.json";
    /**
     * Where the index is while the box art work is not merged into main yet (test builds of
     * the port-0.6.1 branch). Tried only when the main one can't be had.
     */
    static final String INDEX_URL_BRANCH =
        "https://raw.githubusercontent.com/Gin649/Xnes3D-plus/port-0.6.1/community/launchbox-nes.json";
    /** Must match MEDIA_REQUEST_FILE and MEDIA_DONE_FILE in zax-app's android_bridge.rs. */
    static final String REQUEST_FILE = "media-request.txt";
    static final String DONE_FILE = "media-done.txt";
    /** Must match FOLDER in zax-app's game_media.rs. */
    static final String FOLDER = "media";
    /** The index is downloaded again when the copy kept is older than this. */
    private static final long INDEX_MAX_AGE_MS = 7L * 24 * 60 * 60 * 1000;

    private GameMedia() {}

    /** What the Rust side asked for. */
    static final class Request {
        String source = "";
        final List<String> roms = new ArrayList<>();
    }

    /** What a run did, for the dialog. */
    static final class Result {
        int found;
        int missing;
        int pictures;
        String error;
        final StringBuilder notFound = new StringBuilder();
    }

    /** Told how far a run is, from its thread. */
    interface Progress {
        void at(int done, int total, String game);
    }

    /** A game of the index. */
    static final class Game {
        final JSONObject json;
        final String name;
        final List<String> keys = new ArrayList<>();

        Game(JSONObject json) {
            this.json = json;
            this.name = json.optString("n", "");
            keys.add(normalise(name));
            JSONArray alternates = json.optJSONArray("a");
            if (alternates != null) {
                for (int i = 0; i < alternates.length(); i++) keys.add(normalise(alternates.optString(i, "")));
            }
        }

        @Override
        public String toString() {
            String year = json.optString("y", "");
            return year.isEmpty() ? name : name + " (" + year + ")";
        }
    }

    // ---- the request ------------------------------------------------------------------------

    /** The request waiting in the user data folder, taken (its file removed), or null. */
    static Request take(Context c) {
        File root = c.getExternalFilesDir(null);
        if (root == null) return null;
        File file = new File(root, REQUEST_FILE);
        String text;
        try {
            text = new String(readAll(new FileInputStream(file)), "UTF-8");
        } catch (Exception e) {
            return null;
        }
        file.delete();
        Request request = new Request();
        for (String line : text.split("\n")) {
            if (line.startsWith("source=")) request.source = line.substring(7).trim();
            else if (line.startsWith("rom=") && line.length() > 4) request.roms.add(line.substring(4));
        }
        return request;
    }

    /** Tells the running app to read the media again (it checks about twice a second). */
    static void done(Context c) {
        File root = c.getExternalFilesDir(null);
        if (root == null) return;
        try (FileOutputStream out = new FileOutputStream(new File(root, DONE_FILE))) {
            out.write('1');
        } catch (Exception ignored) {
        }
    }

    /** The name of a ROM's media folder: its file name without the extension. */
    static String key(String rom) {
        String name = new File(rom).getName();
        int dot = name.lastIndexOf('.');
        return dot > 0 ? name.substring(0, dot) : name;
    }

    private static File folderOf(Context c, String rom) {
        File root = c.getExternalFilesDir(null);
        if (root == null) return null;
        return new File(new File(root, FOLDER), key(rom));
    }

    // ---- matching names ---------------------------------------------------------------------

    /**
     * A name reduced to what matters for finding a game: lower case, no "(USA)" or "[!]" tags,
     * "The" at either end dropped, "&" as "and", letters and digits only.
     */
    static String normalise(String name) {
        String s = name.toLowerCase(Locale.ROOT);
        s = s.replaceAll("\\([^)]*\\)", " ").replaceAll("\\[[^\\]]*\\]", " ");
        s = s.replace("&", " and ");
        s = s.replaceAll(",\\s*the\\b", " ");
        s = s.replaceAll("^\\s*the\\s+", " ");
        s = s.replaceAll("[^a-z0-9]", "");
        return s;
    }

    /** The game of the index whose name or another name matches the ROM's, or null. */
    static Game match(Map<String, Game> byKey, String rom) {
        String key = normalise(key(rom));
        if (key.isEmpty()) return null;
        Game exact = byKey.get(key);
        if (exact != null) return exact;
        // "Zelda II - The Adventure of Link" for "Zelda II": a name that starts the other.
        Game best = null;
        int bestLength = Integer.MAX_VALUE;
        for (Map.Entry<String, Game> e : byKey.entrySet()) {
            String other = e.getKey();
            if (other.length() < 4 || key.length() < 4) continue;
            if ((other.startsWith(key) || key.startsWith(other)) && Math.abs(other.length() - key.length()) < bestLength) {
                best = e.getValue();
                bestLength = Math.abs(other.length() - key.length());
            }
        }
        return bestLength <= 24 ? best : null;
    }

    /** Up to {@code limit} games of the index for the text {@code query}, the best first. */
    static List<Game> search(List<Game> index, String query, int limit) {
        String key = normalise(query);
        List<Game> starts = new ArrayList<>();
        List<Game> contains = new ArrayList<>();
        for (Game g : index) {
            boolean start = false;
            boolean inside = false;
            for (String k : g.keys) {
                if (k.startsWith(key)) start = true;
                else if (k.contains(key)) inside = true;
            }
            if (start) starts.add(g);
            else if (inside) contains.add(g);
            if (starts.size() >= limit) break;
        }
        starts.addAll(contains);
        return starts.size() > limit ? new ArrayList<>(starts.subList(0, limit)) : starts;
    }

    // ---- the LaunchBox index ----------------------------------------------------------------

    /** The index: the copy kept, downloaded again once a week. */
    static List<Game> index(Context c) throws Exception {
        File kept = new File(c.getFilesDir(), "launchbox-nes.json");
        byte[] data = null;
        boolean old = !kept.isFile() || System.currentTimeMillis() - kept.lastModified() > INDEX_MAX_AGE_MS;
        if (old) {
            try {
                try {
                    data = download(INDEX_URL, 64 * 1024 * 1024);
                } catch (Exception onMain) {
                    data = download(INDEX_URL_BRANCH, 64 * 1024 * 1024);
                }
                File part = new File(kept.getPath() + ".part");
                try (FileOutputStream out = new FileOutputStream(part)) {
                    out.write(data);
                }
                part.renameTo(kept);
            } catch (Exception e) {
                if (!kept.isFile()) throw e;
            }
        }
        if (data == null) data = readAll(new FileInputStream(kept));
        JSONObject root = new JSONObject(new String(data, "UTF-8"));
        JSONArray games = root.getJSONArray("games");
        List<Game> list = new ArrayList<>(games.length());
        for (int i = 0; i < games.length(); i++) {
            JSONObject g = games.optJSONObject(i);
            if (g != null) list.add(new Game(g));
        }
        imagesBase = root.optString("images", "https://images.launchbox-app.com/");
        return list;
    }

    private static String imagesBase = "https://images.launchbox-app.com/";

    /** Every name and other name of the index, to its game. */
    static Map<String, Game> byKey(List<Game> index) {
        Map<String, Game> map = new HashMap<>();
        for (Game g : index) {
            for (String k : g.keys) {
                if (!k.isEmpty() && !map.containsKey(k)) map.put(k, g);
            }
        }
        return map;
    }

    /** Fills the media of every ROM of {@code request} from the LaunchBox index. */
    static Result fromLaunchBox(Context c, Request request, Progress progress) {
        Result result = new Result();
        List<Game> index;
        try {
            index = index(c);
        } catch (Exception e) {
            result.error = "The LaunchBox list couldn't be downloaded. Check the connection and try again.";
            return result;
        }
        Map<String, Game> byKey = byKey(index);
        int done = 0;
        for (String rom : request.roms) {
            progress.at(done++, request.roms.size(), key(rom));
            Game game = match(byKey, rom);
            if (game == null) {
                result.missing++;
                if (result.notFound.length() < 600) result.notFound.append(key(rom)).append('\n');
                continue;
            }
            result.found++;
            result.pictures += write(c, rom, game);
        }
        return result;
    }

    /** Writes the media of {@code game} for {@code rom}; returns how many pictures it saved. */
    static int write(Context c, String rom, Game game) {
        File folder = folderOf(c, rom);
        if (folder == null) return 0;
        folder.mkdirs();
        JSONObject g = game.json;
        StringBuilder info = new StringBuilder();
        line(info, "title", g.optString("n", ""));
        line(info, "year", g.optString("y", ""));
        line(info, "developer", g.optString("d", ""));
        line(info, "publisher", g.optString("p", ""));
        line(info, "players", g.optString("pl", ""));
        line(info, "genre", g.optString("g", ""));
        line(info, "overview", g.optString("o", ""));
        writeText(new File(folder, "info.txt"), info.toString());
        int saved = 0;
        String[][] pictures = { { "box", "box.png", "640" }, { "shot", "shot.png", "640" }, { "cart", "cart.png", "400" } };
        for (String[] p : pictures) {
            String file = g.optString(p[0], "");
            File out = new File(folder, p[1]);
            if (file.isEmpty()) {
                out.delete();
                continue;
            }
            try {
                byte[] data = download(imagesBase + Uri.encode(file, "/"), 16 * 1024 * 1024);
                if (savePng(data, out, Integer.parseInt(p[2]))) saved++;
            } catch (Exception ignored) {
                // a picture that can't be had: the screen says so
            }
        }
        return saved;
    }

    // ---- ES-DE ------------------------------------------------------------------------------

    /** The systems of ES-DE whose media and game lists are read. */
    private static final String[] ES_DE_SYSTEMS = { "nes", "famicom", "fds" };

    /** Fills the media of every ROM of {@code request} from what ES-DE downloaded. */
    static Result fromEsDe(Context c, Request request, Progress progress) {
        Result result = new Result();
        File esde = new File(Environment.getExternalStorageDirectory(), "ES-DE");
        if (!esde.isDirectory()) {
            result.error = "ES-DE's folder wasn't found (Internal storage / ES-DE). "
                + "Xnes3D+ needs \"All files access\" to read it, which choosing the ROM folder asks for.";
            return result;
        }
        // The information of the game lists, by ROM file name without its extension.
        Map<String, Map<String, String>> infos = new HashMap<>();
        for (String system : ES_DE_SYSTEMS) {
            File list = new File(new File(new File(esde, "gamelists"), system), "gamelist.xml");
            if (list.isFile()) readGameList(list, infos);
        }
        int done = 0;
        for (String rom : request.roms) {
            String key = key(rom);
            progress.at(done++, request.roms.size(), key);
            File folder = folderOf(c, rom);
            if (folder == null) continue;
            int saved = 0;
            String[][] kinds = { { "covers", "box.png", "640" }, { "screenshots", "shot.png", "640" }, { "physicalmedia", "cart.png", "400" } };
            for (String[] kind : kinds) {
                File found = null;
                for (String system : ES_DE_SYSTEMS) {
                    File dir = new File(new File(new File(esde, "downloaded_media"), system), kind[0]);
                    for (String ext : new String[] { ".png", ".jpg", ".jpeg", ".webp" }) {
                        File f = new File(dir, key + ext);
                        if (f.isFile()) {
                            found = f;
                            break;
                        }
                    }
                    if (found != null) break;
                }
                if (found == null) continue;
                folder.mkdirs();
                try {
                    if (savePng(readAll(new FileInputStream(found)), new File(folder, kind[1]), Integer.parseInt(kind[2]))) saved++;
                } catch (Exception ignored) {
                    // a picture that can't be read: the screen says so
                }
            }
            Map<String, String> info = infos.get(key);
            if (info != null) {
                folder.mkdirs();
                StringBuilder text = new StringBuilder();
                line(text, "title", value(info, "name"));
                String date = value(info, "releasedate");
                line(text, "year", date.length() >= 4 ? date.substring(0, 4) : "");
                line(text, "developer", value(info, "developer"));
                line(text, "publisher", value(info, "publisher"));
                line(text, "players", value(info, "players"));
                line(text, "genre", value(info, "genre"));
                line(text, "overview", value(info, "desc"));
                writeText(new File(folder, "info.txt"), text.toString());
            }
            if (saved > 0 || info != null) {
                result.found++;
                result.pictures += saved;
            } else {
                result.missing++;
                if (result.notFound.length() < 600) result.notFound.append(key).append('\n');
            }
        }
        return result;
    }

    private static String value(Map<String, String> map, String key) {
        String v = map.get(key);
        return v == null ? "" : v;
    }

    /** Reads the games of an ES-DE gamelist.xml into {@code infos}, by ROM name. */
    private static void readGameList(File file, Map<String, Map<String, String>> infos) {
        try (InputStream in = new FileInputStream(file)) {
            XmlPullParser parser = Xml.newPullParser();
            parser.setInput(in, null);
            Map<String, String> game = null;
            String tag = null;
            for (int event = parser.getEventType(); event != XmlPullParser.END_DOCUMENT; event = parser.next()) {
                if (event == XmlPullParser.START_TAG) {
                    if ("game".equals(parser.getName())) game = new HashMap<>();
                    else tag = parser.getName();
                } else if (event == XmlPullParser.TEXT && game != null && tag != null) {
                    String old = game.get(tag);
                    game.put(tag, old == null ? parser.getText() : old + parser.getText());
                } else if (event == XmlPullParser.END_TAG) {
                    if ("game".equals(parser.getName()) && game != null) {
                        String path = value(game, "path").trim();
                        if (!path.isEmpty()) infos.put(key(path), game);
                        game = null;
                    }
                    tag = null;
                }
            }
        } catch (Exception ignored) {
            // a list that can't be read gives no information
        }
    }

    // ---- small helpers ----------------------------------------------------------------------

    private static void line(StringBuilder out, String key, String value) {
        String v = value.trim().replace("\r", "").replace("\n", "\\n");
        if (!v.isEmpty()) out.append(key).append('=').append(v).append('\n');
    }

    private static void writeText(File file, String text) {
        try (FileOutputStream out = new FileOutputStream(file)) {
            out.write(text.getBytes("UTF-8"));
        } catch (Exception ignored) {
        }
    }

    /**
     * Saves the picture in {@code data} (any format Android reads) as a PNG at {@code out}, its
     * longer side no more than {@code maxSide} pixels. False when it is not a picture.
     */
    static boolean savePng(byte[] data, File out, int maxSide) {
        Bitmap bitmap = BitmapFactory.decodeByteArray(data, 0, data.length);
        if (bitmap == null) return false;
        int w = bitmap.getWidth();
        int h = bitmap.getHeight();
        int longer = Math.max(w, h);
        if (longer > maxSide) {
            float scale = maxSide / (float) longer;
            Bitmap smaller = Bitmap.createScaledBitmap(bitmap, Math.max(1, Math.round(w * scale)), Math.max(1, Math.round(h * scale)), true);
            if (smaller != bitmap) bitmap.recycle();
            bitmap = smaller;
        }
        File part = new File(out.getPath() + ".part");
        boolean ok;
        try (FileOutputStream stream = new FileOutputStream(part)) {
            ok = bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream);
        } catch (Exception e) {
            ok = false;
        }
        bitmap.recycle();
        if (!ok) {
            part.delete();
            return false;
        }
        out.delete();
        return part.renameTo(out);
    }

    private static byte[] readAll(InputStream in) throws Exception {
        try {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buffer = new byte[64 * 1024];
            int n;
            while ((n = in.read(buffer)) > 0) out.write(buffer, 0, n);
            return out.toByteArray();
        } finally {
            in.close();
        }
    }

    /** The bytes at url, following redirects, up to {@code limit} bytes. */
    private static byte[] download(String url, int limit) throws Exception {
        for (int hop = 0; hop < 6; hop++) {
            HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
            c.setConnectTimeout(15000);
            c.setReadTimeout(30000);
            c.setInstanceFollowRedirects(false);
            c.setRequestProperty("User-Agent", "Xnes3D");
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
                    if (out.size() > limit) throw new Exception("too big");
                }
                return out.toByteArray();
            } finally {
                c.disconnect();
            }
        }
        throw new Exception("too many redirects");
    }
}

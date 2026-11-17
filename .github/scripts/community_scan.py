"""Xnes3D+: reads the 3dSen Maker "Profile Repository" board on itch.io, collects the download
links each topic gives, and test-downloads them. Writes community/scan.json for a person to
read before choosing what goes into community/profiles.json. Run by community-scan.yml."""

import base64
import html
import io
import json
import re
import sys
import urllib.parse
import urllib.request
import zipfile

BOARD = "https://itch.io/board/872690/profile-repository"
AGENT = {"User-Agent": "Mozilla/5.0 (Xnes3D+ community scan)"}
HOSTS = ("dropbox.com", "drive.google.com", "1drv.ms", "onedrive.live.com", "mediafire.com",
         "mega.nz", "github.com", "githubusercontent.com", "itch.zone")


def get(url, binary=False):
    req = urllib.request.Request(url, headers=AGENT)
    with urllib.request.urlopen(req, timeout=60) as r:
        data = r.read()
        return (data, r.headers, r.geturl()) if binary else data.decode("utf-8", "replace")


def topics():
    found = []
    page = BOARD
    for _ in range(10):
        text = get(page)
        links = ["https://itch.io" + l if l.startswith("/") else l
                 for l in re.findall(r'href="((?:https://itch\.io)?/t/\d+/[^"#?]+)"', text)]
        found += [l for l in dict.fromkeys(links) if l not in found]
        # The board pages with a "Next page" link (itch.io pages by a cursor, not a number).
        nxt = re.search(r'<a[^>]+href="([^"]+)"[^>]*>\s*Next page', text)
        print(f"board page: {len(found)} topics so far, next: {bool(nxt)}", file=sys.stderr)
        if not nxt:
            break
        page = urllib.parse.urljoin(BOARD, html.unescape(nxt.group(1)))
    return found


def direct(url):
    """The address that hands out the file itself, for the hosts the app knows."""
    u = html.unescape(url)
    if "dropbox.com" in u:
        u = re.sub(r"([?&])dl=0", r"\1dl=1", u)
        if "dl=1" not in u and "raw=1" not in u:
            u += ("&" if "?" in u else "?") + "dl=1"
        return u
    m = re.search(r"drive\.google\.com/file/d/([^/]+)", u) or re.search(r"[?&]id=([^&]+)", u)
    if "drive.google.com" in u and m:
        return f"https://drive.google.com/uc?export=download&id={m.group(1)}"
    if "1drv.ms" in u or "onedrive.live.com" in u:
        # OneDrive's share API hands out the file itself for a personal share link.
        token = base64.urlsafe_b64encode(u.encode()).decode().rstrip("=")
        return f"https://api.onedrive.com/v1.0/shares/u!{token}/root/content"
    return u


def probe(url):
    try:
        data, headers, final = get(direct(url), binary=True)
    except Exception as e:  # noqa: BLE001 (a report, not a program)
        return {"ok": False, "error": str(e)[:200]}
    name = ""
    cd = headers.get("Content-Disposition", "")
    m = re.search(r"filename\*?=(?:UTF-8'')?\"?([^\";]+)", cd)
    if m:
        name = urllib.parse.unquote(m.group(1))
    kind = "other"
    inside = []
    if data[:2] == b"PK":
        kind = "zip"
        try:
            inside = [n for n in zipfile.ZipFile(io.BytesIO(data)).namelist() if n.lower().endswith(".3dn")]
        except Exception:  # noqa: BLE001
            kind = "bad-zip"
    elif b"<html" in data[:2000].lower() or data[:15].lower().startswith(b"<!doctype html"):
        kind = "html"
        t = re.search(rb"<title>(.*?)</title>", data[:20000], re.S | re.I)
        name = (t.group(1).decode("utf-8", "replace").strip()[:80] if t else "") or name
    elif name.lower().endswith(".3dn") or final.lower().split("?")[0].endswith(".3dn"):
        kind = "3dn"
    return {"direct": direct(url), "ok": kind in ("3dn", "zip") and (kind != "zip" or bool(inside)), "kind": kind,
            "bytes": len(data), "file": name, "zip_3dn": inside}


def scan_topic(url):
    text = get(url)
    title = html.unescape(re.search(r"<title>(.*?)</title>", text, re.S).group(1)).strip()
    title = re.sub(r"\s*-\s*3dSen Maker.*$", "", title)
    posts = re.split(r'class="community_post\b', text)[1:]
    out = []
    for i, post in enumerate(posts):
        author = re.search(r'class="post_author"[^>]*>\s*<a[^>]*>([^<]+)</a>', post)
        date = re.search(r'title="([^"]+)"[^>]*class="post_date"|class="post_date"[^>]*title="([^"]+)"', post)
        body = re.search(r'class="post_body[^"]*"[^>]*>(.*?)</div>\s*<div class="post_footer', post, re.S)
        body = body.group(1) if body else post
        for href, label in re.findall(r'<a[^>]+href="([^"]+)"[^>]*>(.*?)</a>', body, re.S):
            href = html.unescape(href)
            if any(h in href for h in HOSTS):
                out.append({"post": i, "author": author.group(1).strip() if author else "",
                            "date": next((g for g in (date.groups() if date else ()) if g), ""),
                            "label": re.sub(r"<[^>]+>", "", html.unescape(label)).strip(),
                            "url": href})
    first_author = re.search(r'class="post_author"[^>]*>\s*<a[^>]*>([^<]+)</a>', posts[0]) if posts else None
    return {"topic": url, "title": title,
            "starter": first_author.group(1).strip() if first_author else "", "links": out}


def main():
    result = []
    for url in topics():
        try:
            t = scan_topic(url)
        except Exception as e:  # noqa: BLE001
            result.append({"topic": url, "error": str(e)[:200]})
            continue
        for link in t["links"]:
            link["probe"] = probe(link["url"])
        result.append(t)
        print(t["title"], len(t["links"]), "links", file=sys.stderr)
    with open("community/scan.json", "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2, ensure_ascii=False)
        f.write("\n")


if __name__ == "__main__":
    main()

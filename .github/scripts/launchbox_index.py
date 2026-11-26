"""Xnes3D+: builds the box art and game information index the app downloads.

Reads Metadata.zip, the daily download of the whole LaunchBox Games Database
(https://gamesdb.launchbox-app.com/Metadata.zip), and keeps only the NES and Famicom games:
the platforms "Nintendo Entertainment System" and "Nintendo Famicom Disk System" (LaunchBox
files Famicom cartridges under the NES platform, with a Japanese region).

Writes community/launchbox-nes.json: a list of games, each with its names, its information
and the file names of its pictures, which are at https://images.launchbox-app.com/<file>.

Usage: python3 launchbox_index.py Metadata.zip community/launchbox-nes.json
"""

import json
import sys
import xml.etree.ElementTree as ET
import zipfile

PLATFORMS = {"Nintendo Entertainment System", "Nintendo Famicom Disk System"}

# The picture types the app shows, and the field each goes in.
PICTURES = {
    "Box - Front": "box",
    "Screenshot - Gameplay": "shot",
    "Cart - Front": "cart",
}
# When a game has a picture of a type for several regions, the first region found here wins.
REGIONS = ["North America", "United States", "World", "Europe", "United Kingdom", "Japan", ""]


def text(element, tag):
    found = element.find(tag)
    return (found.text or "").strip() if found is not None else ""


def region_rank(region):
    return REGIONS.index(region) if region in REGIONS else len(REGIONS)


def main(archive, out):
    games = {}
    alternates = {}
    pictures = {}
    with zipfile.ZipFile(archive) as zipped:
        name = next(n for n in zipped.namelist() if n.lower().endswith("metadata.xml"))
        with zipped.open(name) as xml:
            for _, element in ET.iterparse(xml, events=("end",)):
                tag = element.tag
                if tag == "Game":
                    if text(element, "Platform") in PLATFORMS:
                        year = text(element, "ReleaseYear") or text(element, "ReleaseDate")[:4]
                        games[text(element, "DatabaseID")] = {
                            "n": text(element, "Name"),
                            "y": year,
                            "d": text(element, "Developer"),
                            "p": text(element, "Publisher"),
                            "pl": text(element, "MaxPlayers"),
                            "g": text(element, "Genres").replace(";", ", "),
                            "o": text(element, "Overview"),
                            "pf": "fds" if "Famicom" in text(element, "Platform") else "nes",
                        }
                    element.clear()
                elif tag == "GameAlternateName":
                    alternates.setdefault(text(element, "DatabaseID"), []).append(
                        text(element, "AlternateName"))
                    element.clear()
                elif tag == "GameImage":
                    field = PICTURES.get(text(element, "Type"))
                    if field:
                        key = (text(element, "DatabaseID"), field)
                        candidate = (region_rank(text(element, "Region")), text(element, "FileName"))
                        if candidate[1] and (key not in pictures or candidate < pictures[key]):
                            pictures[key] = candidate
                    element.clear()
    out_games = []
    for database_id, game in games.items():
        if not game["n"]:
            continue
        names = [a for a in alternates.get(database_id, []) if a and a != game["n"]]
        if names:
            game["a"] = names
        for field in PICTURES.values():
            found = pictures.get((database_id, field))
            if found:
                game[field] = found[1]
        # Empty fields are left out, which keeps the file small.
        out_games.append({k: v for k, v in game.items() if v})
    out_games.sort(key=lambda game: game["n"].lower())
    with open(out, "w", encoding="utf-8") as file:
        json.dump({"v": 1, "images": "https://images.launchbox-app.com/", "games": out_games},
                  file, ensure_ascii=False, separators=(",", ":"))
    with_box = sum(1 for game in out_games if "box" in game)
    print(f"{len(out_games)} NES and Famicom games, {with_box} with box art")
    if len(out_games) < 500:
        sys.exit("Too few games: the database's format may have changed.")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

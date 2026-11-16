# Community profiles

`profiles.json` lists the community 3D profiles that **Zaxnes Plus profiles ▸ Get community
profiles** downloads. Each profile is fetched from its creator's own link (nothing is hosted
here), and the app shows who made it. Profiles come from the
[Profile Repository](https://itch.io/board/872690/profile-repository) of the 3dSen Maker forum.

## Adding a profile

Add an entry to `profiles`:

```json
{
  "game": "Mega Man 4",
  "creator": "Name of the creator",
  "page": "https://itch.io/t/…",
  "url": "https://www.dropbox.com/s/…/MegaMan4.3dn?dl=0",
  "version": "1"
}
```

| Field | |
|---|---|
| `game` | the game's name, as shown in the app |
| `creator` | who made the profile, shown in the app |
| `page` | the post or page where the creator shared it |
| `url` | a link to the file: a `.3dn` profile, or a `.zip` holding `.3dn` files |
| `version` | optional; change it when the creator posts an update, and the app downloads it again |
| `file` | optional; the name to save it under (`Mega Man 4.3dn`) when the link's own name isn't good |

The link has to give the file itself, not a page. Dropbox (`?dl=0`) and Google Drive
(`/file/d/<id>/view`) share links are turned into download links by the app. Files hosted on
itch.io need a direct mirror, as itch.io only hands them out through its download page.

Some older profiles have to be opened in 3dSen Maker and saved again before Zaxnes can read them;
if one fails to load, it may need that.

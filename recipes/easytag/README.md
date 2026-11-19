# easytag

Snap recipe for [EasyTAG](https://gitlab.gnome.org/GNOME/easytag), a GTK3
audio file tag editor.

## Upstream

- Project: https://gitlab.gnome.org/GNOME/easytag
- Pinned release: `easytag-2.4.3` (2016-12-05, the latest upstream tag)
- Build system: GNU Autotools (`autogen.sh`, `configure`, `make`)
- Version in the snap: `2.4.3`

Upstream master has continued to receive build fixes (it identifies itself as
2.5.1) but has never published a newer release or tag. The recipe therefore
pins the 2.4.3 release tag and carries the compatibility patches that Debian
applies in `easytag 2.4.3-12`, which is the version in the Ubuntu 26.04
archive:

| Patch | Purpose |
| --- | --- |
| `0001-remove-gdk-pixbuf-pixdata.patch` | Stop using the retired `gdk-pixbuf-pixdata` preprocessor for image resources. |
| `0002-revert-ogg-corruption.patch` | Revert an upstream change that corrupts Ogg files when writing tags. |
| `0003-port-to-taglib-2.patch` | Port `mp4_tag.cc` to the TagLib 2 API. |
| `0004-taglib-2-32bit-fix.patch` | Use TagLib 2's `size_t`/`offset_t` I/O signatures (also needed on arm64). |
| `0005-install-appstream-metadata-manually.patch` | Install AppStream metadata without the removed `appdata-tools` macros. |

The patches are copied verbatim from the Debian packaging; their DEP-3 headers
record the original authors and bug references.

## Dependencies

The `gnome` extension supplies the `gnome-core26` content snap at runtime and
the `gnome-core26-sdk` snap at build time. The SDK already contains GTK3,
GLib/GIO, gdk-pixbuf, pkg-config, the autotools, gettext, libtool and
gdk-pixbuf loaders, so those are not listed as build packages.

Build-only packages missing from the SDK: `intltool`, `yelp-tools` (Mallard
help and the `YELP_HELP_INIT` macro), and the development files for FLAC,
id3lib, libid3tag, Ogg, Opus/opusfile, Speex, TagLib, Vorbis and WavPack.

At runtime `gnome-core26` already ships GTK3, gdk-pixbuf, `libFLAC14`,
`libtag.so.2`, `libwavpack.so.1`, `libspeex.so.1`, `libvorbis*`, `libogg.so.0`
and `libopus.so.0`. Only three runtime libraries are staged from the 26.04
archive:

- `libid3-3.8.3v5` (id3lib, writing ID3v2.3 tags)
- `libid3tag0` (reading MP3 tags)
- `libopusfile0` (Ogg Opus)

`-std=gnu17` is forced for the C build because id3lib 3.8.3's public header
does `typedef int bool`, which is invalid with GCC 15's C23 default. This is
the same workaround Debian uses.

All the format backends are enabled: MP3/MP2, FLAC, Ogg Vorbis, Ogg Opus, Ogg
Speex, MP4/AAC (TagLib), WavPack, MusePack and Monkey's Audio (built-in APE
reader). The Nautilus context-menu extension is disabled: its API is not in
the SDK and current Nautilus no longer loads that extension type.

## Strict confinement limitations

This snap is strict because it targets Core Desktop, which has no classic
snaps. There is no classic variant, and the following limitations are
accepted rather than worked around:

- **File locations.** EasyTAG can only open and write files below the user's
  home directory and on removable media. The `removable-media` plug is not
  auto-connected; the user has to run `snap connect easytag:removable-media`
  once before USB sticks and similar media appear in the file chooser. Music
  on NFS/SMB mounts elsewhere, in `/srv`, or in other system paths cannot be
  reached, and snapd's `home` interface excludes hidden top-level entries
  such as `~/.music`.
- **No raw device or optical drive access.** EasyTAG edits files; it cannot
  read an audio CD directly. CDDB album lookup is a network operation that
  uses the (auto-connected) `network` plug.
- **No host-wide data.** The application sees the content snap's GTK data
  and its own files only; it does not read `/usr/share` from the host.
- **Local help viewer.** The snap does not ship `yelp` (it would pull in a
  second browser engine). The Help action opens `help:easytag`, which may
  fail inside the sandbox, and EasyTAG then falls back to the online manual
  at https://help.gnome.org/users/easytag/stable/.
- **Settings.** Preferences use GSettings/dconf through the `gsettings` plug
  and live in the session's dconf database, shared with the desktop.
- **No Nautilus integration.** The "Open with EasyTAG" context-menu extension
  is not built, so files are opened from EasyTAG's own file chooser or from
  the desktop's normal "Open With" entry for the snap.

A user who needs to edit tags in a directory outside these locations has to
move or copy the collection into the home directory or removable media; the
recipe deliberately declares no `personal-files` or `system-files` interfaces
for arbitrary paths.

# rhythmbox

Snap recipe for [Rhythmbox](https://gitlab.gnome.org/GNOME/rhythmbox), the
GNOME music player and library organizer.

## Upstream

- Project: https://gitlab.gnome.org/GNOME/rhythmbox
- Pinned release: `3.5.1` (2026-08-24, the latest stable upstream tag)
- Build system: Meson (`meson.build`, `meson_options.txt`)
- Version in the snap: `3.5.1`

The source is fetched from the upstream Git repository at the `3.5.1` tag with
`source-depth: 1`. The release adds the new playback backend and the fixes
needed for current PyGObject/libpeas/girepository combinations (3.5.1
specifically fixes ReplayGain with the new backend).

## Dependencies

The `gnome` extension supplies the `gnome-core26` content snap at runtime and
`gnome-core26-sdk` at build time. The SDK already contains GTK3, GLib/GIO,
cairo, pango, gdk-pixbuf, gobject-introspection, gettext, meson/ninja and the
usual GNOME development files, so those are not listed as build packages.

Build-only packages missing from the SDK:

- `libpeas-dev` (libpeas 1.38; the SDK moved to libpeas 2, but Rhythmbox
  still uses the 1.x `libpeas` and `libpeas-gtk` APIs)
- `libtotem-pl-parser-dev`, `libgrilo-0.3-dev`
- `libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev` (also provide the
  `Gst-1.0` and `GstPbutils-1.0` GIRs needed by the `RB` typelib)
- `libjson-glib-dev`, `libsoup-3.0-dev`, `libxml2-dev`, `libtdb-dev`,
  `libnotify-dev`
- `gir1.2-freedesktop-dev` (the `libxml2-2.0` GIR is in this package),
  `libgirepository-2.0-dev`
- `python-gi-dev` (`pygobject-3.0.pc`) and `python3-dev` for the Python
  plugin loader

At runtime `gnome-core26` already ships GTK3, GLib/GIO, gdk-pixbuf, libsoup 3,
json-glib, libtdb, libnotify, libsecret, libgudev, the GStreamer core and the
GStreamer base, good and extra plugin sets. The recipe stages only what it
lacks:

| Package | Why |
| --- | --- |
| `libpeas-1.0-1`, `gir1.2-peas-1.0` | Plugin engine (with the Python 3 loader) and its typelibs. |
| `libtotem-pl-parser18` | Playlist import/export. |
| `libgrilo-0.3-0`, `grilo-plugins-0.3` | Grilo media browser and its sources. |
| `gstreamer1.0-plugins-ugly` | ASF/WMA demuxing, AC-3, MPEG audio, CDIO, SID. |
| `libfaad2` | Runtime library for the extracted AAC decoder plugin. |

The GStreamer split is deliberate:

- `gnome-core26` provides the base, good and extra plugin sets. The content
  snap's launch script puts them on `GST_PLUGIN_SYSTEM_PATH`, so they are not
  staged again.
- `gstreamer1.0-plugins-ugly` is staged normally. It adds the ASF/WMA
  demuxer (`asfdemux`), AC-3, MPEG audio, CDIO, SID and video plugins.
- `gstreamer1.0-plugins-bad` would pull the complete ONNX/TensorFlow/OpenCV
  decoder stack (several hundred packages) for the sake of two plugins.
  Rhythmbox only needs `id3tag` (`id3mux`/`id3v2mux`, used when writing MP3
  metadata) and `faad` (AAC decoding), so the `gst-bad-extras` part downloads
  that one `.deb` without its dependencies and primes only those two plugin
  files.
- `gstreamer1.0-libav` is not staged: its codec set is large and the common
  music formats (MP3, FLAC, Vorbis, Opus, WAV, AAC) are covered by the
  plugins above. ALAC, WMA decoding and other libav-only formats are
  therefore not available.

A cleanup part removes primed libraries and plugin files whose path already
exists in `gnome-core26` (they reach the snap as dependencies of the extra
GStreamer packages) and recompiles the GSettings schema cache.

Python plugin support is enabled. It uses the Python 3.14, PyGObject and
GStreamer typelibs from `gnome-core26`; nothing Python is staged.

## Strict confinement limitations

This snap is strict because it targets Core Desktop, which has no classic
snaps. There is no classic variant, and the following limitations are
accepted rather than worked around:

- **File locations.** Rhythmbox can only read and write files below the
  user's home directory (excluding hidden top-level directories such as
  `~/.music`) and on removable media. The `removable-media` plug is not
  auto-connected; one `snap connect rhythmbox:removable-media` is needed
  before USB sticks and SD cards are visible. Music on NFS/SMB mounts
  outside the home directory, in `/srv`, `/opt` or other system paths is not
  reachable.
- **Audio CDs.** The Audio CD plugin is built, but a confined snap cannot
  open `/dev/sr*` or talk to udisks, so CD playback and CD ripping do not
  work. For the same reason the Brasero CD-recording plugin, which would
  also need optical device access, is disabled.
- **Portable players.** The MTP and iPod plugins are disabled: they access
  USB devices directly, which would require the `raw-usb` interface (and
  `hardware-observe` for device detection) that this snap deliberately does
  not declare. Mass-storage players that appear as ordinary mounted volumes
  are still handled by the generic-player plugin through `removable-media`.
- **DAAP sharing.** The DAAP server needs `network-bind` and Avahi
  publishing. It is disabled, matching the upstream default. The Grilo DAAP
  browser source is present through `grilo-plugins-0.3` and only uses
  outgoing network connections.
- **FM radio and IR remotes.** The FM radio plugin needs V4L2 radio device
  access and the LIRC plugin needs the IR device; both are disabled.
- **Grilo sources.** `grilo-plugins-0.3` is staged, but not every source can
  work in the sandbox: the Tracker-backed metadata source has no Tracker
  miner in the snap, and Avahi-based UPnP/DLNA discovery would need the
  `avahi-observe` interface, which is not declared. Local files and direct
  network streams are unaffected.
- **Python plugins.** The loader is enabled, but plugins that need typelibs
  or interfaces the snap does not ship cannot work: Zeitgeist
  (`rbzeitgeist`), instant-messaging status (`im-status`), and the web remote
  (`webremote`) needs `network-bind` to listen for connections. Magnatune and
  gpodder.net logins cannot use the Secret Service because the
  `password-manager-service` interface is not declared.
- **No network services.** The snap only makes outgoing connections
  (`network`). It does not listen on any port.
- **Help.** `-Dhelp=false` skips the Mallard user manual, which needs
  `yelp-tools`; the help viewer (`yelp`) is not available in the snap or
  reachable from strict confinement. `X-GNOME-DocPath` in the desktop file
  therefore points at a file that is not shipped.
- **Settings and data.** Preferences and the library database use
  GSettings/dconf through the `gsettings` plug and live in the snap's private
  home (library database, podcast downloads, playlists) or the session dconf
  database (preferences), not in the host's Rhythmbox configuration.

Strict confinement does not prevent normal playback, library management,
metadata editing for the formats covered by the staged plugins, internet
radio, podcasts, Last.fm scrobbling, notifications or MPRIS control.

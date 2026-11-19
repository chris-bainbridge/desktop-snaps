# snaps

Build recipes for the Ubuntu desktop snaps that only repackage another
upstream project, stored in a single repository using the Gentoo/Nixpkgs
"one directory per package" model.

This is a local investigation (2026-11-15) into replacing the current
one-repository-per-snap setup. It contains 190 repackage snaps: the 13
migrated to `core26` in the 2026-11 migration, 42 packaging recipes imported
from `~desktop-snappers` and the personal packaging repositories of
Canonical staff on 2026-11-17, one infrastructure recipe imported from
Canonical's pipewire-snap repository on 2026-11-18, 64 new recipes written
on 2026-11-17, 20 apps/utilities added on 2026-11-19, and 50 more new
recipes written on 2026-11-19.

All are published by Canonical or by Canonical desktop-team members, except
the 132 packages written here, which are not in the store yet;
`gthumb` and `quadrapassel` are refreshed imports of existing store snaps.
Every package is on `core26`; the 36 legacy imports were migrated during
the 2026-11-19 refresh.

Projects that develop their own source code and keep their snap recipe in
the same repository (app-center, authd, firmware-updater, the
ubuntu-desktop-provision snaps, ...) are out of scope: their recipe belongs
with the source. They are inventoried in `excluded-projects.yaml` so that
bulk changes can still target them. Firefox is also excluded here: it is a
repackage, but Mozilla publishes it.

Nothing here is wired to CI or to the Snap Store. No build ever uploads.

## Redistribution

A package is in scope if either:

- its upstream is open source, so the licence permits redistribution, or
- the software is already published as a snap by Canonical or by Canonical
  staff; existing Canonical distribution is treated as permission.

Being the first to distribute a proprietary application requires explicit
permission. `metadata.yaml` records the basis in `redistribution:`
(`open-source` or `canonical-published`).

## Layout

Every package lives in a top-level directory named after its **snap name**
(the store identity):

```
gnome-calculator/
gnome-contacts/
simple-scan/
thunderbird/
...
```

A two-letter shard (`gn/`, `th/`, as in Nixpkgs) is deliberately not used:
a flat layout is easier to scan, and sharding can be added later if the
repository grows to many hundreds.

The directory name matches `snap:` in `metadata.yaml`. A package with a single
recipe is **flat**; that recipe is implicitly the `stable` track:

```
gnome-logs/
├── metadata.yaml       # provenance and build mode
├── snapcraft.yaml      # the recipe, implicitly stable
└── ...                 # patches, snap/, or whatever the recipe needs
```

If a package has more than one release track, it is **nested**: the stable
recipe moves too, and every track is a complete build project of its own:

```
ardour/
├── metadata.yaml       # provenance and build mode, shared by all tracks
├── stable/
│   ├── snapcraft.yaml  # latest stable upstream release
│   └── ...
└── edge/
    ├── snapcraft.yaml  # latest upstream git / nightly
    └── ...
```

`beta/` is an official upstream beta or pre-release; `edge/` is the upstream
development branch, latest tag or nightly. `candidate/` is accepted for a
recipe that needs it, but do not create a track without a real upstream
stream behind it. Flat and nested layouts are never mixed, and a nested
package always keeps a `stable/` recipe.

`metadata.yaml` stays at the package root, never inside a track, and is never
copied into a build workspace.

## Building

```console
./build --list                     # name, track, base and publisher
./build gnome-logs                 # flat package, implicit stable
./build gnome-calculator --dry-run # show what would happen
./build gnome-calculator --keep    # keep the workspace for inspection
./build gnome-calculator -- --build-for=arm64   # extra args go to snapcraft
```

For a nested package, `./build <name>` builds its stable track. Use
`--track edge`, `./build <name>/edge`, or `./build <name>:edge` for another
track.

Builds happen **out of tree**, in
`${SNAPS_BUILD_DIR:-/var/tmp/opencode/snaps-build}/<snap>` for a flat or
`<snap>-<track>` for a nested package, so the repository never accumulates
`parts/`, `stage/`, `prime/` or `*.snap`. The built `.snap` is moved to
`<workspace>/artifacts/`, the workspace is removed on success (unless
`--keep`) and kept on failure. The build runs `snapcraft pack --use-lxd`.

Requirements: `bash`, `git`, `rsync`, `python3` with PyYAML, `snapcraft`,
and a working LXD.

## Upstream versions and tracks

The default is the **latest stable upstream release**, not whatever version
happened to be frozen in Ubuntu at release time. Snaps exist precisely so
that the application can move ahead of the distribution.

- Build the application from upstream source at its latest stable release.
- Use libraries from the Ubuntu archive wherever they work, instead of
  rebuilding them: this saves build time and the application usually does
  not care about the library's exact version.
- When a recipe stages an application from the Ubuntu archive rather than
  building it, check that the archive version really is the latest upstream
  release. If it is not, build from upstream or refresh the recipe.
- Imported recipes are starting points, not version pins: refresh them to the
  latest upstream release when importing.
- `stable` is the latest stable upstream release. `beta` is an official
  upstream beta/pre-release. `edge` is the upstream development branch,
  latest tag or nightly, and may be refreshed on a daily or weekly basis.
  Use the upstream's own terminology when deciding which track a release
  belongs to.

## Two build modes

`metadata.yaml` declares one of two modes.

### `mode: vendored`

The package directory is the upstream packaging project, checked in verbatim
(minus `.git`, `.github/` and `.gitignore`). The build copies it into the
workspace and runs snapcraft there. Every package here uses this mode: the
Launchpad `~desktop-snappers` recipes and `ubuntu/thunderbird` contain only
packaging.

### `mode: overlay`

Only the `snap/` subtree is checked in. The build clones the upstream source
repository at a pinned commit and copies `snap/` over the checkout, because
those recipes use `source: .` and other repo-relative paths and cannot build
from the recipe alone.

No package currently uses this mode: every project whose recipe lives inside
its source tree is excluded (see `excluded-projects.yaml`). The mode is kept
for a repackage snap whose upstream ships a recipe that we can only augment.

A package may also set `prepare:` to run a command in the workspace before
snapcraft.

## metadata.yaml

```yaml
snap: gnome-calculator           # store name = directory name
base: core26                     # snap base of the migrated recipe
mode: vendored                   # vendored | overlay
redistribution: open-source      # open-source | canonical-published

upstream: https://gitlab.gnome.org/GNOME/gnome-calculator
recipe: https://git.launchpad.net/~desktop-snappers/gnome-calculator (snap)
recipe-commit: 89048f8fae2e...   # commit these recipe files came from
merge-request: https://code.launchpad.net/~desktop-snappers/gnome-calculator/+git/snap/+merge/514128

store:
  publisher: Canonical           # actual publisher on the store
  verified: true
```

`upstream` is the project itself, where its source is developed, never a
packaging repository. `recipe` is the packaging repository the vendored
recipe came from (with its branch), and `recipe-commit` is the commit it was
copied from.

`metadata.yaml` stays at the package root even when the package is nested.
The track directories contain the complete build projects, so the version
and source of a track live in that track's recipe, not in the shared
metadata.

Overlay packages additionally set `overlay-repo`, `overlay-commit` and
`overlay-to`; none currently do. Imported packages (2026-11-17) carry
`imported: YYYY-MM-DD` and no `merge-request`; their `recipe-commit` is the
packaging repository head they were copied from.

`redistribution` records why we may ship the package: `open-source` when the
licence permits it, `canonical-published` for proprietary software already
distributed as a snap by Canonical or Canonical staff.

The recipes are pinned to the `core26` migration branches. Once the merge
proposals land, refresh the vendored copies from the merged refs.

Recipe provenance, per package, also lives in
`~/git/notes/todo/20261104-desktop-snaps-core26-migration-list.txt`.

## Snaps in scope (190)

### Migrated to core26 (13)

`gnome-calculator`, `gnome-contacts`, `gnome-characters`, `eog`,
`gnome-logs`, `evince`, `cheese`, `simple-scan`, `gnome-text-editor`,
`gnome-calendar`, `gedit`, `gnome-clocks`, `thunderbird`.

### Imported 2026-11-17 (42)

From `~desktop-snappers` packaging repositories.

Published by Canonical (5):

`gnome-boxes`, `shotwell`, `gnome-system-monitor`, `gnome-sudoku`, `loupe`.

Published by Canonical desktop staff (23):

`epiphany`, `drawing`, `glade`, `gnome-2048`, `gnome-chess`,
`gnome-font-viewer`, `gnome-hitori`, `gnome-klotski`, `gnome-weather`,
`gnome-mahjongg`, `gnome-mines`, `gnome-nibbles`, `gnome-recipes`,
`gnome-robots`, `gnome-taquin`, `gnome-tetravex`, `iagno`, `lightsoff`,
`swell-foop`, `tali`, `five-or-more`, `fractal`, `gnome-terminal`.

From personal packaging repositories of Canonical staff (14):

`dippi`, `epic-games-launcher`, `freemind`, `freeplane-mindmapping`,
`geforcenow`, `ghostty`, `godot-4`, `lunar-client`, `projectlibre`,
`squirrelsql`, `sweethome3d-homedesign`, `thrive`, `tigervnc`, `widelands`.

All 42 are on `core26` now. Thirty-six needed a base migration on
2026-11-19; the other six (`epic-games-launcher`, `freeplane-mindmapping`,
`ghostty`, `projectlibre`, `squirrelsql`, `sweethome3d-homedesign`) were
already `core26`.

`thunderbird` is the flat-layout exception: its recipe is the core26
packaging from upstream's `beta` branch, but it builds the current stable
160.0 release. The store's `latest/stable` is still a core24 build, and the
upstream stable branch has not been migrated yet.

### New core26 GNOME apps (2026-11-19, 16)

`papers`, `showtime`, `gnome-snapshot`, `gnome-music`, `gnome-maps`,
`file-roller`, `gnome-connections`, `gnome-usage`, `ghex`, `gitg`, `gnote`,
`polari`, `gucharmap`, `aisleriot`, `four-in-a-row`, `atomix`.

These fill the gaps in the GNOME 50/51 desktop app set: the new document,
video and camera apps, the music and maps apps, the archive manager, the
remote-desktop client and resource monitor, plus a set of small utilities
and games. All are strict and built from upstream source; archive libraries
are used where they work.

### New audio utilities (2026-11-19, 2)

`vocalis` 1.0.0 is the actively maintained GJS/GTK4 continuation of GNOME
Sound Recorder, pinned to its current `main` commit because it has no newer
release tags. `pavucontrol` 6.2 is the PulseAudio per-application volume
mixer. Both use the `audio-record`/`audio-playback` or `pulseaudio` slots
provided by the `pipewire` snap.

### Imported 2026-11-19 (2)

`gthumb` (from `nbuechner/gthumb-snap`) and `quadrapassel` (from Canonical's
`ubuntu/quadrapassel`), both refreshed to the current upstream release and
migrated to `core26`.

### New recipes written 2026-11-19 (50)

A second gap-fill batch, all strict and built from upstream source. They use
only default desktop plugs (`home`, `network`, `desktop`, `wayland`/`x11`,
`audio-playback`, `opengl`, `gsettings`), and deliberately exclude
Flatpak/AppImage tooling, Chromium/CEF apps that need a disabled sandbox or
DRM, and apps that need `udisks2` or other manual interfaces.

GNOME, design and productivity (30):

`apostrophe`, `app-icon-preview`, `bookup`, `buffer`, `contrast`,
`convolution`, `crossword-editor`, `decibels`, `emblem`, `endeavour`,
`errands`, `fava`, `frogr`, `genius`, `glabels`, `gmetronome`,
`gnome-crosswords`, `gtranslator`, `hamster`, `iotas`, `lollypop`, `lorem`,
`manuals`, `ocrfeeder`, `palette`, `pinpoint`, `planify`, `pulp`,
`solanum`, `typography`.

Media, graphics and games (9):

`cine`, `koreader`, `libresprite`, `mediathekview`, `parabolic`, `ruffle`,
`soundconverter`, `spotube`, `tauon`.

Utilities, network and development (11):

`akizip`, `keypunch`, `kitsune`, `minder`, `packet`, `peazip`, `railway`,
`rapidraw`, `rufin`, `text-pieces`, `yacreader`.

`fava` and `iotas` use WebKitGTK, which disables its own sandbox inside a
snap and relies on snap confinement. `errands` and `endeavour` bundle a
private, D-Bus-namespaced evolution-data-server for local tasks; online
accounts and CalDAV passwords are unavailable without keyring/GOA plugs.
`frogr` keeps its OAuth token in its own config rather than the host keyring.
All 50 build on `core26`; the less obvious confinement limits are documented
in each recipe description.

### Imported 2026-11-18 (1)

Infrastructure provider, from Canonical's
[pipewire-snap](https://github.com/canonical/pipewire-snap) repository
(`desktop/26`, commit `13c610e8`):

`pipewire`

Already `core26` and strict upstream. It ships the PipeWire server,
wireplumber and pipewire-pulse as user daemons plus the `pw-*`/`wpctl`
tools, and provides the `pipewire`, `audio-playback`, `audio-record` and
`pulseaudio` slots that the desktop application snaps connect to. It
requires a snapd with the `pipewire-server` interface. It is not an
application and is listed separately from the app imports above.

Since 2026-11-18 the recipe builds PipeWire 1.6.11 and WirePlumber 0.5.19
from upstream source; the 26.04 archive still has 1.6.2/0.5.13. WirePlumber
0.5.19 replaced the snapd permission-prompting C module with its Lua
snap-access policy, so there is no interactive snapd prompt for
`audio-record`.

### New core26 recipes written 2026-11-17 (64)

`terminator`, `meld`, `gnome-tweaks`, `dconf-editor`, `baobab`,
`gnome-disk-utility`, `homebank`, `bleachbit`, `gnumeric`, `dia`,
`mypaint`, `easytag`, `rhythmbox`, `totem`, `pitivi`, `quodlibet`,
`tilix`, `guake`, `bluefish`, `mupdf`, `geary`, `deluge`, `brasero`,
`cpu-x`, `lutris`, `rawtherapee`, `xfburn`, `wesnoth`, `scribus`,
`texstudio`, `lyx`, `strawberry`, `gnucash`, `abiword`, `resources`,
`stacer`, `geany`, `filezilla`, `handbrake`, `qbittorrent`, `ardour`,
`lmms`, `mixxx`, `stellarium`, `zotero`, `mysql-workbench`,
`prismlauncher`, `barrier`, `synaptic`, `hardinfo`, `virt-manager`,
`input-leap`, `kitty`, `wezterm`, `qutebrowser`, `yakuake`, `heroic`,
`bottles`, `qgis`, `evolution`, `calligra`, `pgadmin4`, `librewolf`,
`texmaker`.

Written from scratch on 2026-11-17; not published yet. All use strict
confinement because these snaps must run on Core Desktop, which has no
classic snaps. Strict limits are documented in each recipe; the notable
ones: terminator sees the snap's filesystem and private HOME; baobab scans
only the base root, home and removable media; gnome-disk-utility needs the
udisks2, mount-observe and hardware-observe plugs connected manually;
homebank does not reuse the host configuration; bleachbit can only clean
user-visible paths, not system data or hidden directories.

`gnumeric` builds a matching goffice from source (the archive version is too
old), `dia` pins the unreleased 0.98.0 master commit because upstream has no
newer release tag, and `mypaint`/`easytag` carry compatibility patches for
the modern Python/PyGObject/TagLib toolchain. `tilix` and `guake` are
terminals and share terminator's strict limits; `mupdf` uses the `gpu`
extension (GLUT/OpenGL, not GTK) instead of `gnome`.

`geary` needs manual connections for the keyring and GNOME Online Accounts;
`deluge` ships the GTK client, user daemon, console and web UI; `brasero` and
`xfburn` cannot access optical devices under strict confinement (image
mastering only); `cpu-x` runs unprivileged, so DMI and debugfs data are
unavailable; `lutris` downloads runners into its private area and bundles no
Wine; `wesnoth` hosts LAN games with `network-bind`.

There is no KDE/Qt extension for core26 in snapcraft 9.2.1, so the Qt
applications (`scribus`, `texstudio`, `lyx`, `strawberry`, `stacer`,
`texmaker`) stage Qt 5/6 from the archive instead. `lyx` and `texmaker` do
not bundle TeX Live, so they can edit but not compile documents until a TeX
content snap (or a TeX installation in a visible home path) exists. `gnucash` builds
AqBanking support in but cannot use smartcard readers or Perl quote modules;
`abiword` is a packaging of a largely unmaintained upstream; `stacer` cannot
perform root-level cleaning.

`filezilla` builds libfilezilla/fzssh from source (the archive versions are
too old) and uses Arch's byte-identical mirrors for the official tarballs;
`handbrake` is GTK4 now and builds against system FFmpeg/x264/x265 instead of
its contrib bundle; `qbittorrent` stages Qt 6 and libtorrent.

`evince` loads PDF, TIFF and comic-book archives; its DjVu, XPS and DVI
backends are built but their libraries are not in the platform, so those
formats do not load. `tigervnc` builds upstream 1.16.2 with H.264 RFB
encoding disabled to avoid the jammy FFmpeg stack, so H.264 servers fall
back to Tight/RLE/raw. `gedit` 51 folded the remaining gedit-plugins into
the application; there is no separate plugins package upstream any more.

The audio applications (`ardour`, `lmms`, `mixxx`) can only use
PulseAudio/PipeWire under strict confinement: no ALSA/JACK, no hardware MIDI
or HID controllers. `stellarium` bundles its catalogues and stages Qt 6.

`zotero` packages the official AGPL tarball rather than rebuilding Firefox
ESR; `mysql-workbench` is a best-effort build of the EOL 8.0 series;
`prismlauncher` bundles no Java runtime; `barrier`'s X11 grab/XTEST backend
cannot work on Wayland, so a successor (Input Leap/Deskflow) would be needed.

`synaptic` is effectively non-functional under strict confinement (private
empty APT cache, no host dpkg/apt, no pkexec); `hardinfo` uses hardinfo2 but
lacks root-only DMI/IPMI data; `virt-manager` can only reach remote libvirt
over SSH/TCP; `input-leap` uses the libei/portal path on GNOME 50, with the
X11 backend limited to real X11 sessions.

`kitty`, `wezterm` and `yakuake` are terminals and share terminator's strict
limits; `yakuake` also has no global hotkey and no drop-down anchoring on
GNOME Wayland. `qutebrowser` ships QtWebEngine with Chromium's sandbox
disabled (snapd denies `allow-sandbox` without a store declaration) and no
DRM support.

`heroic` repackages the official Electron deb (Wine/Proton downloaded into
its private area); `bottles` provides the Wine prefix manager without Wine;
`qgis` stages Qt 6/GDAL/GEOS/PROJ and disables GRASS/SAGA/OTB/QtWebEngine;
`evolution` builds a matching evolution-data-server from source with
`org.gnome.Evolution.*` D-Bus service names.

`calligra` stages Qt 6/KF6 and ships Words/Sheets/Stage only; `pgadmin4`
keeps the Flask backend but replaces the Electron window with a QtWebEngine
shell; `librewolf` packages the official Firefox-based tarball with Gecko's
user-namespace sandbox unavailable and no DRM.

The 2026-11-19 GNOME apps have these strict-confinement limits: `papers`
cannot register its thumbnailer with the host or reach the host keyring;
`showtime` has no software AV1, HLS/DASH or DVD support; `gnome-snapshot`
records through the camera portal and falls back to VP8/WebM without a
hardware encoder; `gnome-music` uses a private LocalSearch index (MP3 only
by default); `gnome-maps` needs `location-observe` and
`password-manager-service` connected; `file-roller` has no Nautilus
extension and no remote gvfs; `gnome-connections` is VNC/RDP only, with
SPICE absent and FUSE file transfer blocked; `gnome-usage` cannot see host
Storage/Network data; `gthumb` has RAW support disabled (26.04 ships
libraw 0.21, gthumb 4 needs 0.22); `quadrapassel` aborts when no audio
device is available; `polari` bundles the legacy Telepathy stack; `gnote`
keeps notes in its private data directory; `aisleriot` carries a 48 MB
Guile compiled-module cache. `vocalis` writes recordings to its private
data directory and needs `audio-record`; `pavucontrol` needs the
`pulseaudio` connection, and may need `PULSE_SERVER` adjusted if the
PipeWire pulse socket lives under its per-snap runtime directory.

Among the migrated imports, `gnome-boxes` runs a private session
libvirt/QEMU and cannot use the host's system libvirt; `shotwell` has no
network plug or gvfs, so publishing uploads and remote browsing are
unavailable and face detection is not built; `gnome-terminal` still needs
the `dot-files` personal-files connection for `~/.bashrc` and friends;
`tigervnc` is built from upstream 1.16.2 with H.264 disabled.

The 2026-11-17 and 2026-11-19 batches repackage distinct upstream projects;
only the recipes are checked in. snapcraft fetches upstream sources, and for
the proprietary launchers the vendor artifacts, at build time - no vendor
binaries are tracked in this repository. `element-desktop` is not included:
it has no public recipe.

All 190 repackage another upstream project. Nearly all are GNOME
applications whose source lives at gitlab.gnome.org; the others include
Mozilla (thunderbird), Matrix (fractal), Element (not imported), Epic,
NVIDIA, Lunar Client, Godot, Ghostty, TigerVNC, Widelands, Thrive,
Freeplane, ProjectLibre, SQuirreL SQL, Sweet Home 3D and FreeMind.

Store publishers: Canonical 15, Ken VanDine 29, Jeremy Bicha (`jbicha`) 5,
Jean-Baptiste Lallement (`jibel`) 5, Robert Ancell 1. They are desktop-team
snaps and are included.

## Excluded upstream projects

`excluded-projects.yaml` lists the snaps whose project develops its own
source and carries its own recipe. It records the snap, repository, target
branch, local working copy and pending merge request for each, so bulk
changes can target them along with the packages here. They are not built by
`./build`.

Firefox is not in that list either: it is a repackage, but Mozilla publishes
it.

Content snaps (`mesa-2604`, `gnome-core26`) are separate projects and are not
in this repository.

## Not done here

- No CI workflows, no store uploads, no track/channel management. The
  `tracks:` idea from the design note needs the publishing story first.
- No fetch manifest. Most downloads are declared as snapcraft sources with
  tags/commits, but thunderbird's `override-pull` downloads a source tarball
  with `wget` and no hash; a manifest would have to cover that class of
  download.
- No history rewrite or migration of the per-snap repositories; this
  repository is additive.

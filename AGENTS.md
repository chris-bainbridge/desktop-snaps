# Repository guide for agents

This file records how this repository is organised and how to change it.
End-user documentation lives in [`README.md`](README.md); the upstream
metadata schema is documented in
[`docs/upstream-schema.md`](docs/upstream-schema.md).

## What this repository is

Build recipes for Ubuntu desktop snaps that repackage another upstream
project, using the Gentoo/Nixpkgs "one directory per package" model. The
repository replaces the previous one-repository-per-snap setup. It is
additive: no per-snap history was imported.

The `snap-repo` command is a Rust tool in `tool/`, invoked through the
`snap-repo` shell wrapper at the repository root. The wrapper rebuilds the
release binary when `tool/Cargo.toml` or `tool/src` changes.

## Ground rules

- Keep `README.md` end-user facing. Repository internals, conventions,
  history and inventory belong in this file or in `docs/`.
- Update the counts and inventory in this file when packages are added,
  removed or migrated.
- Nothing here runs in CI and nothing uploads to the Snap Store. Do not add
  automatic publishing.
- Do not commit build output: `parts/`, `stage/`, `prime/`, `*.snap`,
  `*.snap.debug` and `tool/target/` are ignored. Packed snaps are moved to
  the top-level `snaps/` directory, whose contents are also ignored.
- Use `/var/tmp/opencode/<task>/` for scratch work, not `/tmp`. The tool's
  own builds use a temporary directory under `/tmp` and clean it up.
- After `snapcraft pack` finishes, delete the LXD container you used:
  `lxc list --project snapcraft`, then `lxc delete <container>`.

## Repository layout

Every package lives in a directory under `recipes/`, named after its **snap
name** (the store identity). The directory name matches `snap:` in
`metadata.yaml`. A flat layout is deliberate: a two-letter shard (`gn/`,
`th/`, as in Nixpkgs) can be added later if the repository grows to many
hundreds of packages.

A package with a single recipe is **flat**; that recipe is implicitly the
`stable` track:

```text
recipes/gnome-logs/
├── metadata.yaml       # provenance and build mode
├── snapcraft.yaml      # the recipe, implicitly stable
└── ...                 # patches, snap/, or whatever the recipe needs
```

If a package has more than one release track it is **nested**: the stable
recipe moves too, and every track is a complete build project of its own:

```text
recipes/ardour/
├── metadata.yaml       # provenance and build mode, shared by all tracks
├── stable/
│   ├── snapcraft.yaml  # latest stable upstream release
│   └── ...
└── edge/
    ├── snapcraft.yaml  # latest upstream git / nightly
    └── ...
```

- `stable` is the latest stable upstream release.
- `beta` is an official upstream beta or pre-release.
- `edge` is the upstream development branch, latest tag or nightly.
- `candidate` is accepted for a recipe that needs it, but do not create a
  track without a real upstream stream behind it.

Flat and nested layouts are never mixed, and a nested package always keeps
a `stable/` recipe. `metadata.yaml` stays at the package root under
`recipes/`, never inside a track, and is never copied into a build
workspace.

## metadata.yaml

```yaml
snap: gnome-calculator           # store name = directory name
base: core26                     # snap base
mode: vendored                   # vendored | overlay
redistribution: open-source      # open-source | canonical-published

upstream:
  provider: gitlab
  host: gitlab.gnome.org
  project: GNOME/gnome-calculator
recipe: https://git.launchpad.net/~desktop-snappers/gnome-calculator (snap)
recipe-commit: 89048f8fae2e...   # commit these recipe files came from
merge-request: https://code.launchpad.net/~desktop-snappers/gnome-calculator/+git/snap/+merge/514128

store:
  publisher: Canonical           # actual publisher on the store
  verified: true
```

`upstream` is the project itself, where its source is developed, never a
packaging repository. It records only where to look; the current version
stays in the recipe. The tool parses `snap`, `base`, `mode`, `prepare`,
`overlay-*`, `store` and `upstream`; the other keys (`recipe`,
`recipe-commit`, `merge-request`, `redistribution`, `imported`, store
`verified`) are provenance recorded for humans and are ignored by
`snap-repo`. Unknown keys inside `upstream` are an error.

See [`docs/upstream-schema.md`](docs/upstream-schema.md) for the provider
fields, tag patterns and manual entries.

`redistribution` records why the package may be shipped: `open-source` when
the licence permits it, `canonical-published` for proprietary software
already distributed as a snap by Canonical or Canonical staff. Being the
first to distribute a proprietary application requires explicit
permission.

Imported packages (2026-11-17) carry `imported: YYYY-MM-DD` and no
`merge-request`; their `recipe-commit` is the packaging repository head they
were copied from.

### Build modes

`mode: vendored` means the package directory under `recipes/` is the
upstream packaging project, checked in verbatim (minus `.git`, `.github/`
and `.gitignore`). The build copies it into the workspace and runs
snapcraft there. Every current package uses this mode: the Launchpad
`~desktop-snappers` recipes and `ubuntu/thunderbird` contain only
packaging.

`mode: overlay` means only the `snap/` subtree is checked in. The build
would clone the upstream source repository at a pinned commit and copy
`snap/` over the checkout, because such recipes use `source: .` and other
repo-relative paths. No package currently uses it; the mode is kept for a
repackage snap whose upstream ships a recipe we can only augment. Overlay
packages additionally set `overlay-repo`, `overlay-commit` and
`overlay-to`.

A package may also set `prepare:` to run a command in the workspace before
snapcraft.

**Tool limitation:** the native build in `tool/src/build_cmd.rs` currently
supports only `mode: vendored` and rejects `prepare:` and overlay fields.
Do not add a package that relies on them without implementing build
support first.

## Authoring and changing recipes

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
- Imported recipes are starting points, not version pins: refresh them to
  the latest upstream release when importing.
- Use the upstream's own terminology when deciding which track a release
  belongs to.
- Prefer strict confinement, because snaps must run on Core Desktop, which
  has no classic snaps. Document non-obvious strict-confinement limits in
  the recipe description.
- Do not track vendor binaries or build artifacts. snapcraft fetches
  upstream sources, and for proprietary launchers the vendor artifacts, at
  build time.
- There is no KDE/Qt extension for core26 in snapcraft 9.2.1. Stage Qt 5/6
  from the archive instead.
- Desktop app snaps connect to the `pipewire`, `audio-playback`,
  `audio-record` and `pulseaudio` slots provided by the `pipewire` snap.

`./snap-repo sync` checks upstream tags and rewrites the app part's
`source-tag` and the literal top-level `version:`, preserving comments and
quoting. Only tag-mode packages are updated; URL, script, manual and other
report-only packages are left alone. Always inspect a sync with `-n`
(`--dry-run`) first.

## Working on the snap-repo tool

The tool is a Rust crate in `tool/` (edition 2024, GPL-2.0-or-later),
invoked through the root `snap-repo` wrapper. Module map:

| File | Responsibility |
|---|---|
| `src/main.rs` | Repo root resolution and subcommand dispatch |
| `src/cli.rs` | clap definitions for all commands and options |
| `src/discover.rs` | Find packages and validates flat/nested layout |
| `src/metadata.rs` | Parse `metadata.yaml` and build modes |
| `src/package.rs` | Package/track model and bump modes |
| `src/affected.rs` | Commit ranges to CI build selectors |
| `src/list.rs` | The `list` table and its JSON form |
| `src/build_cmd.rs` | Build plans, workspaces, LXD, parallel jobs |
| `src/sync.rs` | Upstream checks, rate limiting, writing recipes |
| `src/upstream.rs` | Provider clients and version discovery |
| `src/recipe_edit.rs` | Surgical `source-tag`/`version:` rewrites |
| `src/version.rs` | Version comparison and ordering |
| `src/rate_limit.rs` | The `rate-limit` subcommand |
| `src/quota.rs` | Cached provider quota |

Command behaviour to preserve:

- `build` resolves every requested package before starting, attempts all of
  them even when some fail, reports failures at the end and exits non-zero.
  Snapcraft output is captured unless `-v` is given; failure output shows
  the last 20 lines plus the log path.
- Builds run out of tree and move the packed snap into `snaps/`. On
  success, the workspace and per-build LXD container are removed unless
  `--keep` is used. On failure the workspace is removed unless `--keep`.
- `build --all` is not implemented yet and should keep failing explicitly.
- `sync` writes only after all selected packages have been checked, keeps a
  global request interval, respects provider rate-limit headers and quotas,
  and reports skipped packages. `--allow-errors` still writes the successful
  bumps but exits zero, for the nightly sync job.
- `list` prints name, track, version, base and publisher, one row per track.
  The version is the literal top-level `version:` or, for recipes that take it
  from the source, the app part's `source-tag`; `-` means neither is present.
  `list --json` emits the same rows as an array of objects.
- `affected` prints the build selectors implied by a commit range. A selector
  is a package name for stable or `package:track` otherwise; changes to a
  track directory affect that track, while the package root affects every
  track. `--worktree` compares against `HEAD`, `--select` forces a list,
  `--fallback` applies when no recipe changed, and `--github-output` writes
  `matrix=`/`has_work=`. It never emits more than `--max` (256) entries.

Before handing off tool changes, run:

```console
cargo fmt --manifest-path tool/Cargo.toml
cargo clippy --manifest-path tool/Cargo.toml --all-targets
cargo test --manifest-path tool/Cargo.toml
```

Commit `tool/Cargo.lock` with manifest changes. Do not add dependencies
without a reason; the crate already uses `clap`, `anyhow`, `serde`,
`serde_yaml`, `serde_json`, `reqwest`, `regex`, `percent-encoding`,
`tempfile`, `tokio` and `signal-hook`.

`repo_root()` is resolved when the binary runs, never at compile time, so a
renamed, moved or copied checkout keeps working. It prefers `SNAP_REPO_ROOT`,
which the wrapper sets to its own directory, and otherwise walks up from the
executable's real path to the directory containing both `tool/Cargo.toml` and
`recipes/`. If neither finds a root, the tool stops with an error naming the
variable. The wrapper always builds the binary from this checkout, so invoking
`./snap-repo` from another working directory still operates on this repository.

## Inventory and history

The repository holds 190 recipes. This section records how it grew and the
known confinement limits of each batch; keep it current when adding
packages.

The initial 190: 13 migrated to `core26` in the 2026-11 migration, 42
packaging recipes imported from `~desktop-snappers` and the personal
packaging repositories of Canonical staff on 2026-11-17, one infrastructure
recipe imported from Canonical's pipewire-snap repository on 2026-11-18, 64
new recipes written on 2026-11-17, 20 apps/utilities added on 2026-11-19,
and 50 more new recipes written on 2026-11-19.

All are published by Canonical or by Canonical desktop-team members, except
the 132 packages written here, which are not in the store yet; `gthumb` and
`quadrapassel` are refreshed imports of existing store snaps. Every package
is on `core26`; the 36 legacy imports were migrated during the 2026-11-19
refresh.

Projects that develop their own source code and keep their snap recipe in
the same repository (app-center, authd, firmware-updater, the
ubuntu-desktop-provision snaps, ...) are out of scope: their recipe belongs
with the source. They are inventoried in `docs/excluded-projects.yaml` so
that bulk changes can still target them. Firefox is also excluded here: it
is a repackage, but Mozilla publishes it.

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
`frogr` keeps its OAuth token in its own config rather than the host
keyring. All 50 build on `core26`; the less obvious confinement limits are
documented in each recipe description.

### Imported 2026-11-18 (1)

Infrastructure provider, from Canonical's
[pipewire-snap](https://github.com/canonical/pipewire-snap) repository
(`desktop/26`, commit `13c610e8`): `pipewire`.

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

The Qt applications (`scribus`, `texstudio`, `lyx`, `strawberry`, `stacer`,
`texmaker`) stage Qt 5/6 from the archive instead of using a KDE extension.
`lyx` and `texmaker` do not bundle TeX Live, so they can edit but not
compile documents until a TeX content snap (or a TeX installation in a
visible home path) exists. `gnucash` builds AqBanking support in but cannot
use smartcard readers or Perl quote modules; `abiword` is a packaging of a
largely unmaintained upstream; `stacer` cannot perform root-level cleaning.

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

Store publishers: Canonical 17, Ken VanDine 29, Jeremy Bicha (`jbicha`) 5,
Jean-Baptiste Lallement (`jibel`) 5, Robert Ancell 1, Sergio Schvezov 1.
They are desktop-team snaps and are included.

### Excluded upstream projects

`docs/excluded-projects.yaml` lists the snaps whose project develops its
own source and carries its own recipe. It records the snap, repository,
target branch, local working copy and pending merge request for each, so
bulk changes can target them along with the packages here. It is
documentation only: no tool reads it, and these projects are not built by
`./snap-repo build`.

Content snaps (`mesa-2604`, `gnome-core26`) are separate projects and are not
in this repository.

### Provenance

The recipes are pinned to the `core26` migration branches. Once the merge
proposals land, refresh the vendored copies from the merged refs.

Recipe provenance, per package, also lives in
`~/git/notes/todo/20261104-desktop-snaps-core26-migration-list.txt`.

## Continuous integration

`.github/workflows` builds recipes and keeps the results in the rolling
`snaps` prerelease. No workflow uploads to the Snap Store.

- `build-snaps.yml` is the reusable read-only worker. It installs LXD and
  snapcraft, runs `./snap-repo build <selector>` as the runner user in the
  `lxd` group, names the packed snap
  `<package>-<track>-<version>.snap`, and uploads it with `archive: false`.
  It has `contents: read` only.
- `pr-build.yml` runs on `pull_request` and builds changed recipes only for
  trusted authors. The gate uses
  `github.event.pull_request.author_association`: under `canonical/` only
  `MEMBER`/`OWNER` build; while the repository is personal,
  `OWNER`/`COLLABORATOR` build so the pipeline can be tested. PR snaps are
  per-run artifacts and never enter the pool.
- `pr-comment.yml` reacts to `workflow_run` completion, lists the failed
  matrix jobs and creates or updates one marked comment. It deletes that
  comment after a later success. It has `pull-requests: write`, checks out
  nothing and runs no PR code.
- `build.yml` runs on `push` to `main` and on `workflow_dispatch` (with an
  optional comma-separated package list). On push it sleeps
  `SETTLE_SECONDS` (300) first, so a follow-up fix cancels the sleep rather
  than a build. Detection starts at the `ci-last-built` tag when present,
  otherwise at the push's `before`. The `record` job moves that tag only after
  the build and publish succeeded, or when there was nothing to build; a
  cancelled or failed run leaves it alone.
- `publish` downloads the run's artifacts and runs
  `.github/scripts/snaps_pool.py publish`. It is the only job with
  `contents: write`. It creates the `snaps` prerelease if needed, uploads
  `<package>-<track>-<version>.snap`, updates `pool.json`, and deletes the
  previous asset for that selector immediately. Assets whose package or track
  is no longer in `recipes/`, and any unreferenced asset, are deleted on every
  publish and by the nightly prune. There is no retention window.
- `nightly.yml` runs `sync --all --allow-errors` only when the
  `ENABLE_NIGHTLY_SYNC` repository variable is `true`. It uses the automatic
  `GITHUB_TOKEN` for public GitHub APIs and the read-only `GITLAB_TOKEN`
  secret (exported as `GITLAB_TOKEN_GITLAB_GNOME_ORG`) for
  `gitlab.gnome.org`. It commits to `automated/upstream-sync` and opens or
  updates one pull request; it does not build or publish. The prune job runs
  nightly regardless. Enabling the sync also requires the repository setting
  Settings → Actions → General → "Allow GitHub Actions to create and approve
  pull requests", because the bot opens the PR with the automatic token.
- No secret is committed. The sync job fails closed without the variable and
  token.
- `snaps_pool.py` is CI glue, unit-tested by
  `.github/scripts/test_snaps_pool.py` (`python3 -m unittest discover -s
  .github/scripts`). It talks to the GitHub API only through `gh` and never
  touches the checkout.

PR builds and the store: there is no publish step anywhere and no store
credential is used. Publishing, when it exists, must be a separate job on
`main` behind a protected environment.

## Not done here

- No Snap Store uploads and no track/channel management. CI builds and the
  `snaps` release pool are internal test artifacts. The `tracks:` idea from
  the design note needs the publishing story first.
- No fetch manifest. Most downloads are declared as snapcraft sources with
  tags/commits; thunderbird's source tarball is declared with a pinned
  `source-checksum`, and its langpack XPIs are checked against the release's
  signed `SHA256SUMS` at build time. The remaining unhashed downloads are
  `hunspell`'s `apt download` and `kerberos`'s `pull-lp-source`; a manifest
  would have to cover that class of download.
- No history rewrite or migration of the per-snap repositories; this
  repository is additive.

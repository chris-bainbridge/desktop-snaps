# snaps

Build recipes for the Ubuntu desktop snaps that only repackage another
upstream project, stored in a single repository using the Gentoo/Nixpkgs
"one directory per package" model.

This is a local investigation (2026-11-15) into replacing the current
one-repository-per-snap setup. It contains 75 repackage snaps: the 13
migrated to `core26` in the 2026-11 migration, 28 packaging recipes imported
from other `~desktop-snappers` repositories on 2026-11-17, 14 from the
personal packaging repositories of Canonical staff, and 20 new recipes
written on 2026-11-17.

All are published by Canonical or by Canonical desktop-team members, except
the twenty new recipes, which are not in the store yet. Bases: 39 `core26`,
29 `core24`, 4 `core22`, 2 `core20`, 1 `core18`. The non-`core26` packages
have not been migrated.

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
with a dozen packages a flat layout is easier to scan, and sharding can be
added later if the repository grows to hundreds.

The directory name matches `snap:` in `metadata.yaml`. Each package directory
is a complete build project in whatever shape its upstream uses:

```
gnome-logs/
├── metadata.yaml       # provenance and build mode
├── snapcraft.yaml      # the recipe
└── ...                 # patches, snap/, or whatever the recipe needs
```

`metadata.yaml` is never copied into a build workspace.

## Building

```console
./build --list                     # all packages, base and publisher
./build gnome-logs                 # build the gnome-logs package
./build gnome-calculator --dry-run # show what would happen
./build gnome-calculator --keep    # keep the workspace for inspection
./build gnome-calculator -- --build-for=arm64   # extra args go to snapcraft
```

Builds happen **out of tree**, in `${SNAPS_BUILD_DIR:-/var/tmp/opencode/snaps-build}/<snap>`,
so the repository never accumulates `parts/`, `stage/`, `prime/` or `*.snap`.
The built `.snap` is moved to `<workspace>/artifacts/`, the workspace is
removed on success (unless `--keep`) and kept on failure. The build runs
`snapcraft pack --use-lxd`.

Requirements: `bash`, `git`, `rsync`, `python3` with PyYAML, `snapcraft`,
and a working LXD.

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

upstream: https://git.launchpad.net/~desktop-snappers/gnome-calculator (snap)
recipe-commit: 89048f8fae2e...   # commit these recipe files came from
merge-request: https://code.launchpad.net/~desktop-snappers/gnome-calculator/+git/snap/+merge/514128

store:
  publisher: Canonical           # actual publisher on the store
  verified: true
```

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

## Snaps in scope (75)

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

### New core26 recipes (20)

`terminator`, `meld`, `gnome-tweaks`, `dconf-editor`, `baobab`,
`gnome-disk-utility`, `homebank`, `bleachbit`, `gnumeric`, `dia`,
`mypaint`, `easytag`, `rhythmbox`, `totem`, `pitivi`, `quodlibet`,
`tilix`, `guake`, `bluefish`, `mupdf`.

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

The last group repackages distinct upstream projects; only the recipes are
checked in. snapcraft fetches upstream sources, and for the proprietary
launchers the vendor artifacts, at build time - no vendor binaries are
tracked in this repository. `element-desktop` is not included: it has no
public recipe.

All 55 repackage another upstream project. Nearly all are GNOME
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
- `core26` migration of the 36 imported packages still on older bases; 39 of
  the 75 are on `core26` (13 migrated, 6 imported and 20 new), and `freemind`
  is still on `core18`.
- No history rewrite or migration of the per-snap repositories; this
  repository is additive.

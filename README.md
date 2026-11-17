# snaps

Build recipes for the Ubuntu desktop snaps that only repackage another
upstream project, stored in a single repository using the Gentoo/Nixpkgs
"one directory per package" model.

This is a local investigation (2026-11-15) into replacing the current
one-repository-per-snap setup. It contains the 13 repackage snaps from the
2026-11 `core26` migration, all of which are published by Canonical or by
Canonical desktop-team members.

Projects that develop their own source code and keep their snap recipe in
the same repository (app-center, authd, firmware-updater, the
ubuntu-desktop-provision snaps, ...) are out of scope: their recipe belongs
with the source. They are inventoried in `excluded-projects.yaml` so that
bulk changes can still target them. Firefox is also excluded here: it is a
repackage, but Mozilla publishes it.

Nothing here is wired to CI or to the Snap Store. No build ever uploads.

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
./build --list                     # all packages, mode and publisher
./build gnome-logs                 # build the gnome-logs package
./build gnome-calculator --dry-run # show what would happen
./build gnome-calculator --keep    # keep the workspace for inspection
./build gnome-calculator -- --build-for=arm64   # extra args go to snapcraft
```

Builds happen **out of tree**, in `${SNAPS_BUILD_DIR:-/var/tmp/opencode/snaps-build}/<snap>`,
so the repository never accumulates `parts/`, `stage/`, `prime/` or `*.snap`.
The built `.snap` is moved to `<workspace>/artifacts/`, the workspace is
removed on success (unless `--keep`) and kept on failure. snapcraft is
invoked with `--use-lxd`.

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

upstream: https://git.launchpad.net/~desktop-snappers/gnome-calculator (snap)
recipe-commit: 89048f8fae2e...   # commit these recipe files came from
merge-request: https://code.launchpad.net/~desktop-snappers/gnome-calculator/+git/snap/+merge/514128

store:
  publisher: Canonical           # actual publisher on the store
  verified: true
```

Overlay packages additionally set `overlay-repo`, `overlay-commit` and
`overlay-to`; none currently do.

The recipes are pinned to the `core26` migration branches. Once the merge
proposals land, refresh the vendored copies from the merged refs.

Recipe provenance, per package, also lives in
`~/git/notes/todo/20261104-desktop-snaps-core26-migration-list.txt`.

## Snaps in scope (13)

`gnome-calculator`, `gnome-contacts`, `gnome-characters`, `eog`,
`gnome-logs`, `evince`, `cheese`, `simple-scan`, `gnome-text-editor`,
`gnome-calendar`, `gedit`, `gnome-clocks`, `thunderbird`.

All of them repackage another upstream project: the first twelve are GNOME
applications whose source lives at gitlab.gnome.org, thunderbird is Mozilla
source. The `~desktop-snappers` and `ubuntu/thunderbird` repositories contain
only packaging.

Store publishers are not all `Canonical`: `evince`, `cheese` and
`gnome-text-editor` are published by Ken VanDine. They are desktop-team
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

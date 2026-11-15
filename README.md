# snaps

Build recipes for the Ubuntu desktop snaps maintained by the desktop team,
stored in a single repository using the Gentoo/Nixpkgs "one directory per
package" model.

This is a local investigation (2026-11-15) into replacing the current
one-repository-per-snap setup. It contains the 23 snaps migrated to `core26`
in the 2026-11 migration, all of which are published by Canonical or by
Canonical desktop-team members. Firefox is deliberately excluded: it is
published by Mozilla.

Nothing here is wired to CI or to the Snap Store. No build ever uploads.

## Layout

Packages are sharded by the first two characters of the **snap name** (the
store identity), as in Nixpkgs:

```
gn/gnome-calculator/
gn/gnome-contacts/
sn/snap-store/
...
```

The directory name is always the snap name, even when the upstream project is
called something else (`sn/snap-store` comes from `ubuntu/app-center`).

Each package directory is a complete build project in whatever shape its
upstream uses, plus a `metadata.yaml`:

```
gn/gnome-logs/
├── metadata.yaml       # provenance and build mode
├── snapcraft.yaml      # the recipe
└── ...                 # patches, snap/, or whatever the recipe needs
```

`metadata.yaml` is never copied into a build workspace.

## Building

```console
./build --list                     # all packages, mode and publisher
./build gnome-logs                 # build gn/gnome-logs
./build gn/gnome-logs              # same, shard prefix accepted
./build snap-store --dry-run       # show what would happen
./build snap-store --keep          # keep the workspace for inspection
./build snap-store -- --build-for=arm64   # extra args go to snapcraft
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
workspace and runs snapcraft there. Used for the Launchpad
`~desktop-snappers` recipes (which are packaging-only repositories) and for
the `ubuntu-desktop-provision` snap branches.

### `mode: overlay`

Only the `snap/` subtree is checked in. The build clones the upstream source
repository at a pinned commit and copies `snap/` over the checkout, because
those recipes use `source: .` and other repo-relative paths and cannot build
from the recipe alone. Used for app-center, authd, desktop-security-center,
firmware-updater, prompting-client, snap-tpmctl and
snapd-desktop-integration.

A package may also set `prepare:` (authd-google runs
`snap/scripts/prepare-variant --broker google --copy` after the overlay).

## metadata.yaml

```yaml
snap: snap-store                 # store name = directory name
base: core26                     # snap base of the migrated recipe
mode: overlay                    # vendored | overlay

# overlay mode only
overlay-repo: https://github.com/ubuntu/app-center
overlay-commit: bb0c969d1932...  # upstream base commit the recipe applies to
overlay-to: snap                 # where snap/ is copied inside the checkout
prepare: ...                     # optional command, run in the workspace

upstream: https://github.com/ubuntu/app-center (main)
recipe-commit: d85d71ef3e9e...   # commit these recipe files came from
merge-request: https://github.com/ubuntu/app-center/pull/2280

store:
  publisher: Canonical           # actual publisher on the store
  verified: true
```

The core26 recipes are currently pinned to the migration branches. Once the
merge proposals and pull requests land, the overlay commits should be
re-pinned to upstream and the overlays re-checked against upstream `snap/`.

Recipe provenance, per package, also lives in
`~/git/notes/todo/20261104-desktop-snaps-core26-migration-list.txt`.

## Snaps in scope (23)

`snap-store`, `firmware-updater`, `desktop-security-center`,
`prompting-client`, `snapd-desktop-integration`, `snap-tpmctl`,
`authd-google`, `ubuntu-desktop-bootstrap`, `ubuntu-desktop-init`,
`factory-reset-tools`, `gnome-calculator`, `gnome-contacts`,
`gnome-characters`, `eog`, `gnome-logs`, `evince`, `cheese`, `simple-scan`,
`gnome-text-editor`, `gnome-calendar`, `gedit`, `gnome-clocks`,
`thunderbird`.

Store publishers are not all `Canonical`: `evince`, `cheese` and
`gnome-text-editor` are published by Ken VanDine, and `ubuntu-desktop-init`
by Sebastien Bacher (seb128). They are desktop-team snaps and are included.

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

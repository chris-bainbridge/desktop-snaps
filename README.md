# desktop-snaps

Build recipes for Ubuntu desktop snaps that repackage another upstream
project, stored in one repository using the Gentoo/Nixpkgs "one directory
per package" model.

The repository holds 190 recipes, all on `core26`. Nearly all are strictly
confined, so they can run on Core Desktop, and most package GNOME
applications; the rest cover development tools, media players, games and
other desktop software. `snap-repo` is the small command-line tool that
lists the available packages, builds any of them, and checks upstream
releases.

## Requirements

- A Rust toolchain. The `snap-repo` wrapper builds the tool on demand.
- `rsync`.
- `snapcraft` 9.x with a working LXD.
- Network access to the upstream sources at build time.
- A GitHub and/or GitLab token for `snap-repo sync` (see
  [Sync upstream versions](#sync-upstream-versions)).

## Getting started

```console
git clone <repository-url> snaps
cd snaps
./snap-repo list
./snap-repo build gnome-calculator
```

The built snap is written to `snaps/`. The first command that needs the
tool compiles it before running.

## Commands

Run `./snap-repo --help` or `./snap-repo <command> --help` for the full
option list.

### List packages

```console
./snap-repo list
```

Prints one row per package track with the snap name, track, base and store
publisher:

```text
NAME                    TRACK   BASE    PUBLISHER
abiword                 stable  core26
gnome-calculator        stable  core26  Canonical
```

### Build a snap

```console
./snap-repo build gnome-logs                  # flat package, stable track
./snap-repo build gnome-calculator -n         # show the build plan only
./snap-repo build gnome-calculator --keep     # keep workspace and container
./snap-repo build gnome-calculator -v         # stream snapcraft output
./snap-repo build -j3 abiword barrier crossword-editor   # three in parallel
./snap-repo build gnome-calculator -- --build-for=arm64  # args to snapcraft
```

For a nested package, `./snap-repo build <name>` builds its `stable` track.
Use `--track edge`, `<name>/edge` or `<name>:edge` to build another track.
`-j`/`--jobs` sets how many packages may build at once; each gets its own
temporary workspace and LXD container. Every requested package is
attempted; failures are reported at the end and make the command exit
non-zero.

Builds happen out of tree in a temporary directory, so the repository never
accumulates `parts/`, `stage/`, `prime/` or `*.snap` files. The packed snap
is moved to `snaps/`, whose contents are git-ignored. After packing, the
per-build LXD container and the temporary workspace are removed; `--keep`
retains both for inspection. On failure the temporary workspace is also
removed unless `--keep` is given.

Snapcraft output is captured. A successful build prints only the
`snap-repo` lifecycle lines; on failure the last 20 lines of snapcraft
output and the execution log path are printed after all workers finish.
`-v` streams snapcraft output live to stderr, prefixed with
`[package (track)]`.

### Sync upstream versions

```console
./snap-repo sync -n gnome-calculator   # report only
./snap-repo sync gnome-calculator      # update the recipe
./snap-repo sync --all --dry-run       # inspect every package
```

`sync` checks the selected packages (or every package with `--all`) and
rewrites the app part's `source-tag` and the literal top-level `version:`,
if present. Comments and quoting style are preserved. Only tag-mode
packages are updated; URL, script, manual and other report-only packages
are left alone. Use `--verbose` to show the resolved recipe and app part
for each track.

GitHub and `gitlab.gnome.org` require authentication: a GitHub token from
`GITHUB_TOKEN`, `GH_TOKEN` or `gh auth token`, and a GitLab token from
`GITLAB_TOKEN_<HOST>` or `glab config`. Small GitLab hosts may fall back to
unauthenticated access with a warning.

To see provider quota and reset times, run:

```console
./snap-repo rate-limit
./snap-repo rate-limit --refresh   # ignore cached quota
```

## Repository layout

Every package lives in a directory under `recipes/`, named after its snap
name (the store identity):

```text
recipes/
├── gnome-calculator/
├── gnome-contacts/
├── simple-scan/
├── thunderbird/
└── ...
```

A package with one recipe is **flat**; that recipe is implicitly the
`stable` track:

```text
recipes/gnome-logs/
├── metadata.yaml       # provenance and build mode
├── snapcraft.yaml      # the recipe, implicitly stable
└── ...                 # patches or files the recipe needs
```

A package with more than one release track is **nested**, and every track
is a complete build project of its own:

```text
recipes/ardour/
├── metadata.yaml       # provenance and build mode, shared by all tracks
├── stable/
│   └── snapcraft.yaml
└── edge/
    └── snapcraft.yaml
```

`metadata.yaml` sits at the package root and records where the upstream
project and the recipe came from; the version and source of a track live in
that track's recipe. See [`docs/upstream-schema.md`](docs/upstream-schema.md)
for the format.

[`docs/excluded-projects.yaml`](docs/excluded-projects.yaml) lists desktop
snaps that keep their recipe in their own source repository. They are
inventoried there so bulk changes can still reach them, but they are not
part of `recipes/` and are not built here. The file is documentation only;
no tool reads it.

## Scope

A package is included if its upstream is open source, so the licence
permits redistribution, or if the software is already published as a snap
by Canonical or Canonical staff; existing Canonical distribution is treated
as permission. Being the first to distribute a proprietary application
requires explicit permission. Each package records the basis in
`metadata.yaml` under `redistribution:`.

Projects that develop their own source code and keep their snap recipe in
the same repository are out of scope: their recipe belongs with the source.
Firefox is excluded for a different reason: it is a repackage, but Mozilla
publishes it. Content snaps such as `mesa-2604` and `gnome-core26` are
separate projects and are not in this repository.

This repository is not wired to CI or to the Snap Store, and no build ever
uploads. It does not include the history of the per-snap repositories it
replaces; it is additive.

## Documentation and conventions

- [`docs/upstream-schema.md`](docs/upstream-schema.md) documents the
  `upstream:` metadata schema, provider fields, tag patterns and manual
  entries.
- [`AGENTS.md`](AGENTS.md) records the repository conventions for changing
  recipes or the `snap-repo` tool, plus the package inventory and migration
  history.

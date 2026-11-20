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

Prints one row per package track with the snap name, track, version, base and
store publisher:

```text
NAME                    TRACK   VERSION  BASE    PUBLISHER
abiword                 stable  3.0.8    core26
gnome-calculator        stable  51.1     core26  Canonical
```

The version is the literal top-level `version:` or, for recipes that take it
from the source, the app part's `source-tag`; `-` means neither is present.
Use `./snap-repo list --json` for a machine-readable array.

### Find affected packages

```console
./snap-repo affected --base origin/main --head HEAD
./snap-repo affected --worktree --json
./snap-repo affected --select gnome-logs,five-or-more --json
```

`affected` maps a commit range to the recipes it touches and prints one build
selector per line (or a compact JSON array with `--json`). A selector is a
package name for the stable track, or `package:track` for another track. A
change to a track's own directory affects only that track; `metadata.yaml` and
other files at the package root affect every track. Deleted and unknown
packages are ignored.

`--worktree` compares against `HEAD` instead of two revisions, which is useful
after `sync` edits the working tree. `--fallback` supplies packages to use when
the range changes no recipe, and `--github-output FILE` appends `matrix=` and
`has_work=` for `$GITHUB_OUTPUT`. The command refuses more than `--max` entries
(256 by default), because that is GitHub's matrix limit.

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
for each track. `--allow-errors` writes the bumps that succeeded and exits
successfully even when some packages could not be checked; the errors are
still reported.

GitHub and `gitlab.gnome.org` require authentication: a GitHub token from
`GITHUB_TOKEN`, `GH_TOKEN` or `gh auth token`, and a GitLab token from
`GITLAB_TOKEN_<HOST>` or `glab config`. Small GitLab hosts may fall back to
unauthenticated access with a warning.

To see provider quota and reset times, run:

```console
./snap-repo rate-limit
./snap-repo rate-limit --refresh   # ignore cached quota
```

## Continuous integration

The workflows in `.github/workflows` build recipes and keep the results in a
rolling prerelease named `snaps`. Nothing in them uploads to the Snap Store.

- **Pull requests** from trusted authors build only the recipes the PR
  changes and upload the raw snaps as per-run artifacts. Nothing is added to
  the release pool. The author check uses `author_association`: once the
  repository lives under `canonical/`, only organization members (`MEMBER`)
  build. While the repository is still personal, its owner and collaborators
  are trusted so the pipeline can be exercised. A separate `workflow_run`
  workflow posts one comment on a failed build, updates it and removes it when
  a later build passes; it has `pull-requests: write` but never runs PR code.
- **Pushes to main** wait five quiet minutes, then build everything merged
  since the `ci-last-built` tag. Cancelled or failed runs do not move the tag,
  so their packages are picked up by the next run. A separate job with
  `contents: write` then publishes whatever built successfully; recipe code
  never runs in that job.
- **The release pool**: one asset per buildable package track, named
  `<package>-<track>-<version>.snap` (for example
  `gnome-calculator-stable-51.1.snap`). `pool.json` maps each
  `package:track` selector to its current asset. A new successful build
  replaces the old asset immediately, and assets for packages or tracks that
  no longer exist on `main` are deleted immediately rather than left around.
  The `snaps` release is marked prerelease and is not a distribution channel.
- **Nightly sync**: disabled until an administrator sets the
  `ENABLE_NIGHTLY_SYNC` repository variable. It uses the automatic GitHub
  token for public GitHub API reads and for the pull request, and a read-only
  `gitlab.gnome.org` token from the `GITLAB_TOKEN` secret. It opens or updates
  one pull request; it does not build or publish, and merging the PR triggers
  the normal main build. The repository setting Settings → Actions → General
  → "Allow GitHub Actions to create and approve pull requests" must also be
  enabled, because the bot opens the PR with the automatic token. A nightly
  prune job reconciles the pool independently.

The workflows require Rust, `snapcraft` 9.x and LXD on the runner. The build
jobs run `snapcraft pack --use-lxd` through `snap-repo build`.

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

This repository keeps its CI results in the `snaps` release pool and never
uploads to the Snap Store. It does not include the history of the per-snap
repositories it replaces; it is additive.

## Documentation and conventions

- [`docs/upstream-schema.md`](docs/upstream-schema.md) documents the
  `upstream:` metadata schema, provider fields, tag patterns and manual
  entries.
- [`AGENTS.md`](AGENTS.md) records the repository conventions for changing
  recipes or the `snap-repo` tool, plus the package inventory and migration
  history.

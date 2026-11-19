# Freeplane Snap

This repository packages [Freeplane](https://www.freeplane.org/) as a Snap,
published as [`freeplane-mindmapping`](https://snapcraft.io/freeplane-mindmapping).
It is a packaging project, separate from the upstream Freeplane source code:
the Snap uses an official upstream distribution ZIP and adds the launcher,
desktop integration, and a bundled OpenJDK 21 runtime. It does not develop or
maintain the application itself.

## For users

Install the Snap from the store:

```sh
sudo snap install freeplane-mindmapping
```

For general Freeplane usage, see the
[upstream project site](https://www.freeplane.org/).

## Reporting issues

Please report an issue to the project responsible for it:

- **Snap packaging or installation** (for example, the Snap does not launch,
  printing fails, or there is a desktop integration problem):
  [open an issue in this repository](https://github.com/jibel/freeplane-snap/issues).
- **Freeplane application behavior** (for example, mind map editing, scripting,
  or application feature bugs):
  [report it to upstream](https://github.com/freeplane/freeplane/issues).
  The [upstream website](https://www.freeplane.org/) has additional project
  information and support links.

When reporting a Snap issue, include your Ubuntu version, Snap version, and
the steps or command that reproduce the problem.

## Build the Snap

Requirements: Snapcraft 9 and a working `snapcraft` installation. The build
uses Snapcraft's managed build environment for the `core26` base.

From the repository root, run:

```sh
snapcraft pack
```

The build downloads the official Freeplane ZIP distribution
(`freeplane_bin-<version>.zip`) from the upstream
[GitHub releases](https://github.com/freeplane/freeplane/releases) and
verifies it against a pinned SHA-256 before unpacking it; no manual download
is needed. Both the version and the checksum are pinned in the
`freeplane-mindmapping` part of `snapcraft.yaml` and must be updated together
when moving to a new release.

The part sets the Snap version from the ZIP name, so the 1.13.5 archive
produces `freeplane-mindmapping_1.13.5_amd64.snap`.
The package currently targets amd64 only; the bundled Java runtime paths and
Freeplane launcher are configured for that architecture.

The generated `.snap` can be installed locally for a trial with
`sudo snap install --dangerous ./<generated-snap-file>.snap`.

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

1. Download the official Freeplane ZIP distribution
   (`freeplane_bin-<version>.zip`) from
   [SourceForge](https://sourceforge.net/projects/freeplane/files/).
2. Place the archive in `download/`. Files there are ignored by Git.
3. From the repository root, run:

   ```sh
   snapcraft pack
   ```

If multiple ZIP archives are present, the build selects the highest version
and sets the Snap version from the selected archive name, so
`freeplane_bin-1.13.3.zip` produces `freeplane-mindmapping_1.13.3_amd64.snap`.
The package currently targets amd64 only; the bundled Java runtime paths and
Freeplane launcher are configured for that architecture.

The generated `.snap` can be installed locally for a trial with
`sudo snap install --dangerous ./<generated-snap-file>.snap`.

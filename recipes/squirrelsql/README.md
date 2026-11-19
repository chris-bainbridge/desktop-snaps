# SQuirreL SQL Snap

This repository packages [SQuirreL SQL Client](https://squirrel-sql.sourceforge.io/)
as a Snap. It is a packaging project, separate from the upstream
SQuirreL SQL source code: the Snap uses an official upstream standard
distribution and adds the launcher, desktop integration, and bundled JDBC
drivers. It does not develop or maintain the SQL client itself.

## For users

The Snap includes JDBC drivers for:

- SQLite
- PostgreSQL
- MariaDB and MySQL-compatible databases (MariaDB Connector/J)

SQuirreL SQL can also connect to other JDBC-compatible databases when you
provide a suitable driver. The drivers listed above are the ones bundled by
this Snap by default; the database server itself is not included.

For installation and general SQuirreL SQL usage, see the
[upstream project site](https://squirrel-sql.sourceforge.io/).

## Reporting issues

Please report an issue to the project responsible for it:

- **Snap packaging or installation** (for example, the Snap does not launch,
  is missing a bundled driver, or has a packaging/desktop integration problem):
  [open an issue in this repository](https://github.com/jibel/squirrelsql-snap/issues).
- **SQuirreL SQL application behavior** (for example, SQL editor, database
  metadata, or application feature bugs):
  [report it to upstream](https://github.com/squirrel-sql-client/squirrel-sql-code/issues).
  The [upstream website](https://squirrel-sql.sourceforge.io/) has additional
  project information and support links.

When reporting a Snap issue, include your Ubuntu version, Snap version, and
the steps or command that reproduce the problem. For an upstream issue, include
the SQuirreL SQL version and relevant database/JDBC driver details.

## Build the Snap

Requirements: Snapcraft 9 and a working `snapcraft` installation. The build
uses Snapcraft's managed build environment for the `core26` base.

The build downloads the pinned SQuirreL SQL **standard ZIP** distribution
from the [upstream downloads](https://sourceforge.net/projects/squirrel-sql/files/)
and verifies it against a SHA-256 checksum recorded in `snapcraft.yaml`, so
there is no manual download step. To move to a new upstream release, update
`version` and `checksum` in the `override-pull` of the `squirrelsql` part and
the value in `UPSTREAM_VERSION`.

From the repository root, run:

```sh
snapcraft pack
```

The generated `.snap` can be installed locally for a trial with
`sudo snap install --dangerous ./<generated-snap-file>.snap`.

The Snap version is taken from the distribution archive name.

## Tests

Install the Snap manually before running the tests. For a locally built Snap:

```sh
sudo snap install --dangerous ./squirrelsql_<version>_amd64.snap
```

Then run the tests with no arguments. They target the installed `squirrelsql`
Snap through its confined runtime and do not install or modify the Snap:

```sh
tests/startup.sh
tests/sqlite-cli.sh
```

`startup.sh` launches the installed GUI app and checks that it remains running
for 10 seconds. It requires an active graphical session; set `STARTUP_TIMEOUT`
to change the check duration. For headless CI, run it under a virtual X server
such as `xvfb-run`.
`sqlite-cli.sh` uses SQuirreL's CLI to connect to an in-memory SQLite database
and verifies that `select 42` returns 42.

## Automated releases

The GitHub Actions workflow checks SourceForge's stable-files feed daily and
can also be started manually from the Actions tab. When a new standard ZIP is
available newer than the version in `UPSTREAM_VERSION` and its version has not
already been released here, the workflow builds the Snap, installs it, runs
both smoke tests, and creates a GitHub Release with the `.snap` file attached.
The release is published only after both tests pass. The version file is the
baseline for the Snap already packaged when automation was introduced.

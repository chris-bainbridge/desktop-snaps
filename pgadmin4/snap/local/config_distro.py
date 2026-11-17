# Installed next to pgAdmin's config.py by the snap recipe.
#
# pgAdmin invokes pg_dump, pg_restore, pg_dumpall and psql by name.  The
# PostgreSQL 18 client is staged at $SNAP/usr/lib/postgresql/18/bin; the
# Debian pg_wrapper symlinks in /usr/bin are not used because /etc and
# /usr/share/postgresql-common inside the snap are the base's, not the
# staged tree's.  Configure the real binaries as the default for every
# supported server generation so Backup, Restore and the PSQL tool work
# without the user having to fill in Preferences first.
import os

_SNAP = os.environ.get("SNAP", "/snap/pgadmin4/current")
_BIN = os.path.join(_SNAP, "usr/lib/postgresql/18/bin")

# The snap has no Secret Service/desktop keyring behind the password-manager
# interface.  Use pgAdmin's own encrypted SQLite storage with a master
# password instead of probing D-Bus at startup.
USE_OS_SECRET_STORAGE = False
KEYRING_NAME = ''

DEFAULT_BINARY_PATHS = {
    "pg": _BIN,
    "pg-13": _BIN,
    "pg-14": _BIN,
    "pg-15": _BIN,
    "pg-16": _BIN,
    "pg-17": _BIN,
    "pg-18": _BIN,
}

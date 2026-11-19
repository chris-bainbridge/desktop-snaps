# -*- coding: utf-8; -*-
"""Installation paths for Guake inside the snap.

Upstream generates this module from ``guake/paths.py.in`` with the install
prefix baked in (``make generate-paths`` / ``make install``). A snap is not
installed under ``/usr``: the application lives under ``$SNAP`` and the
interpreter that imports this module comes from the gnome-core26 content snap
mounted at ``$SNAP/gnome-platform``. The directories are therefore resolved
against ``$SNAP`` at runtime; when ``SNAP`` is unset (running from a source
tree) they fall back to the usual ``/usr`` layout.
"""

import os
import subprocess

_PREFIX = os.environ.get("SNAP") or "/"
_SHARE_DIR = os.path.join(_PREFIX, "usr", "share")

# The schema, the glade files, the pixmaps and the autostart template are
# installed side by side in /usr/share/guake, as in upstream's Makefile.
_DATA_DIR = os.path.join(_SHARE_DIR, "guake")

LOCALE_DIR = os.path.join(_SHARE_DIR, "locale")
IMAGE_DIR = os.path.join(_DATA_DIR, "pixmaps")
GLADE_DIR = _DATA_DIR
SCHEMA_DIR = _DATA_DIR
GUAKE_THEME_DIR = _DATA_DIR

# "Start at login" copies autostart-guake.desktop into the user's
# ~/.config/autostart. A strictly confined snap cannot write to that hidden
# directory of the real home, and an entry written to the snap's private home
# would never be read by the host session, so the feature is disabled:
# refresh_user_start() in guake/prefs.py returns early when either path is
# empty.
LOGIN_DESTOP_PATH = ""
AUTOSTART_FOLDER = ""


def try_to_compile_glib_schemas():
    """Compile the schema in place, as upstream does on a version change.

    /usr/share/guake is read-only inside the snap and gschemas.compiled is
    built when the snap is built, so this only ever fails with an error that
    guake/guake_app.py logs and ignores.
    """
    subprocess.check_call(["glib-compile-schemas", "--strict", SCHEMA_DIR])

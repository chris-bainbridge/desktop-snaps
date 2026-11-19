// Installed by the snap build.  The snap is mounted read-only and is updated
// by snapd, so Zotero must not try to replace its own files.  This file is
// read after the defaults inside app/omni.ja.
pref("app.update.auto", false);
pref("app.update.background.scheduling.enabled", false);

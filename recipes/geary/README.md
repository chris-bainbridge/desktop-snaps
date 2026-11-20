# geary

Email client for GNOME, packaged for Core Desktop from the upstream
GitLab repository at the latest stable release tag, **46.0** (2024-05-20).

* Upstream: <https://gitlab.gnome.org/GNOME/geary>
* Build system: Meson (Vala + C, GTK3)
* Base: `core26`, confinement: `strict`

## Runtime dependencies

`gnome-core26` already provides most of Geary's stack, including GTK3, GLib,
WebKitGTK 4.1 (with its GIR typelib and web/network processes), libsecret,
gspell, gsound, GNOME Online Accounts, GCR/GCK, libhandy, libsoup-3.0,
SQLite (with FTS3/FTS5), ICU, json-glib, enchant-2, libstemmer and libgee.
The snap therefore does not stage WebKitGTK, GTK or the other libraries from
that list.

The `libraries` part stages only what `gnome-core26` lacks:

| Package | Reason |
| --- | --- |
| `libgmime-3.0-0t64` | MIME parsing, a hard dependency of Geary |
| `libpeas-1.0-1` | plugin engine (the GNOME 49 SDK moved to libpeas 2) |
| `libfolks26` | contact aggregation; Geary links `libfolks` directly |
| `libunwind8` | stack traces for problem reports |
| `libytnef0` | winmail.dat (TNEF) attachment support |
| `libxml2-16` | XML/HTML helpers; present in the SDK but not the content snap |

libxml2 is moved from `usr/lib/<triplet>` to `usr/lib` because the GNOME
extension's cleanup part removes primed libraries that also exist in the SDK.
Only the `key-file` Folks backend is staged; it is exposed at the compiled-in
backend path through a layout entry.

WebKitGTK is not bundled: the `gnome` extension binds the content snap's
WebKitGTK 4.1 directory at `/usr/lib/<triplet>/webkit2gtk-4.1`, and Geary
points its own web extension at `/usr/lib/<triplet>/geary/web-extensions`,
which the layout entry exposes from the snap.

## Strict confinement

Core Desktop has no classic snaps, so this package is strict-only. The
following limitations are inherent to that choice:

* **Files.** `home` allows reading and writing the user's home directory
  (apart from top-level hidden entries). `GTK_USE_PORTAL=1` from the GNOME
  extension makes GTK file dialogs use the desktop document portal, so
  attachments can also be picked from locations outside `$HOME`. Geary's
  configuration, account database and mail store live in the snap's private
  XDG directories under `$SNAP_USER_DATA`; an existing `~/.config/geary` or
  `~/.local/share/geary` is not migrated automatically.
* **Password storage.** Geary uses libsecret (`org.freedesktop.secrets`).
  The `password-manager-service` interface is deny-auto-connection and must
  be connected manually:
  `snap connect geary:password-manager-service`. Without it, Geary still
  works but cannot save or reload account passwords, so they have to be
  entered each time.
* **GNOME Online Accounts / OAuth2.** GOA is a host session service and is
  only reachable through the `accounts-service` interface, which is
  deny-auto-connection:
  `snap connect geary:accounts-service`. Without that connection, accounts
  configured in GNOME Settings do not appear in Geary and OAuth2 sign-in for
  Gmail/Outlook (which goes through GOA) is unavailable. Manually configured
  IMAP/SMTP accounts with passwords or app-specific passwords work.
* **Contacts.** Geary's contact autocompletion uses Folks. The Evolution
  Data Server address-book stack is not staged (its client libraries are not
  in `gnome-core26`), so only the empty key-file backend is present and no
  address-book suggestions are shown. The host `contacts-service` interface
  exists, but wiring it up would require staging the whole EDS client
  library stack.
* **D-Bus activation.** The snap owns `org.gnome.Geary` on the session bus
  through its D-Bus slot, but upstream's
  `usr/share/dbus-1/services/org.gnome.Geary.service` is not visible to the
  host bus. Launching Geary from the desktop/menu works; D-Bus activation
  from another application only works while Geary is already running.
* **Autostart/background service.** Upstream's
  `geary-autostart.desktop` is not installed as a host autostart entry, so
  "run in background at login" does not start Geary automatically. The
  background service can still be started manually and stays reachable for
  the rest of the session.
* **Notifications** use the session bus `org.freedesktop.Notifications`
  service through the desktop interface; the desktop shell shows them.
* **Help.** The help pages are installed inside the snap, but Geary opens
  `help:geary` through the host's help viewer, which cannot see them, and no
  help viewer is bundled. F1 therefore does not open the user manual.
* **Spell checking** uses the `enchant-2` runtime and the `en_US` dictionary
  from `gnome-core26`; other dictionaries are not bundled.
* **Sent sound** uses the `audio-playback` interface (auto-connected).
  `browser-support` is declared for the embedded WebKitGTK engine; inside a
  snap WebKitGTK disables its own bubblewrap sandbox and relies on snapd's
  confinement instead.

## Building

```
./snap-repo build geary
```

The recipe builds Geary with the conventional `--prefix=/usr` and
`-Dprofile=release`. `layout` entries expose the compiled-in
`/usr/share/geary`, `/usr/lib/<triplet>/geary` and
`/usr/lib/<triplet>/folks` paths inside the snap. `-Dprofile=release`
prevents the development app ID `org.gnome.Geary.Devel`, which would not
match the D-Bus slot or the desktop file.

Geary decides whether it is installed by comparing the directory of the
running executable with its compiled-in prefix. Inside a snap the executable
is at `/snap/geary/<revision>/usr/bin` while the prefix is `/usr`, so the
`override-pull` step patches that one check to also accept a
`SNAP_NAME=geary` runtime; without it Geary would look for its resources in
the build directory.

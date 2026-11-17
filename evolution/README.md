# evolution

Repackage of the GNOME Evolution groupware suite for core26, strictly
confined for Core Desktop.

- Upstream: https://gitlab.gnome.org/GNOME/evolution
- Version: 3.62.1 (2026-10-09), the latest stable Evolution tag
- Base: core26, `confinement: strict`, `grade: stable`
- Architectures: amd64, arm64

## Evolution Data Server

`gnome-core26` does not ship evolution-data-server. Its content has GTK3,
WebKitGTK 4.1, libsoup-3.0, libsecret, libgcr, libgspell, libcanberra,
libgoa, libgweather, geocode-glib, gnome-desktop, libnotify, libunity and
the other GNOME runtime libraries, but no `libedataserver`, `libcamel`,
`libebook`, `libecal` or EDS services. It also lacks libxml2 at runtime.

Evolution 3.62.1 expects a matching EDS release (the two are released
together and Evolution requires at least the same version). The recipe
therefore builds evolution-data-server 3.62.1 from source in the same snap,
links Evolution against that staged copy, and stages libical, libytnef,
gnome-autoar and libxml2, which the content snap does not provide.

EDS is built with `-DDBUS_SERVICES_PREFIX=org.gnome.Evolution`, the scheme
Flathub uses. The snap's EDS services are therefore named

```
org.gnome.Evolution.org.gnome.evolution.dataserver.Sources5
org.gnome.Evolution.org.gnome.evolution.dataserver.Calendar8
org.gnome.Evolution.org.gnome.evolution.dataserver.AddressBook10
org.gnome.Evolution.org.gnome.evolution.dataserver.UserPrompter0
org.gnome.Evolution.org.gnome.evolution.dataserver.OAuth2Response0
org.gnome.Evolution.org.gnome.Evolution-alarm-notify
```

instead of the unprefixed names a host EDS uses. This prevents the snap
from accidentally talking to, or fighting over names with, a host
evolution-data-server. The prefixed names are compiled into the staged
libedataserver/libecal/libebook, so Evolution uses them automatically.

The GUI keeps the normal `org.gnome.Evolution` bus name. A host Evolution
cannot run at the same time as the snap because both use that name.

## Layout

Evolution and EDS are built with the conventional `/usr` prefix but their
private libraries, modules, backends, helpers and data live below paths
that the base and content snap do not provide. `layout` binds:

```
/usr/lib/evolution
/usr/share/evolution
/usr/lib/evolution-data-server
/usr/share/evolution-data-server
/usr/libexec/evolution-data-server
/usr/libexec/evolution
/usr/share/dbus-1/services
```

The last bind lets Evolution's backup tool read the EDS service files from
the compiled-in `/usr/share/dbus-1/services` path.

D-Bus activation for the EDS services is provided by snapd service files
generated from the `daemon: dbus` apps plus their `activates-on` slots.
`evolution-alarm-notify` is spawned directly by Evolution through the
`/usr/libexec/evolution-data-server` layout bind, so it only needs the
D-Bus slot, not a snap app.

## Manual connections

`password-manager-service`, `accounts-service` and `cups` are
deny-auto-connection. Connect the ones that are needed:

```console
snap connect evolution:password-manager-service   # keyring passwords/OAuth tokens
snap connect evolution:accounts-service           # GNOME Online Accounts
snap connect evolution:cups                       # printing
```

Without the keyring connection Evolution cannot store or reload account
passwords or OAuth2 tokens. Without the accounts connection GOA sources
do not appear.

## Strict-confinement limitations

- Configuration, mail, calendars, contacts and caches live in the snap's
  private XDG directories below `$SNAP_USER_DATA`. A host Evolution's
  `~/.config/evolution` and `~/.local/share/evolution` are hidden paths
  that the `home` interface does not expose, so existing data is not
  imported. Export to vCard/iCalendar/mbox files in a visible directory
  and import them into the snap.
- `home` only exposes non-hidden files. `~/.gnupg` and `~/.pki` are not
  visible, so OpenPGP and S/MIME keys/certificates from the host cannot be
  used; signing and encryption are unavailable. Removable media needs the
  `removable-media` interface, which is not declared.
- There is no host sendmail and no access to `/var/mail` or the local mail
  spool. SMTP submission works; local delivery and the sendmail transport
  do not.
- EDS `evolution-alarm-notify` is started by Evolution the first time the
  calendar backend runs, not by a systemd user unit at login. Reminders
  therefore start only after Evolution has been launched in a session.
  Evolution itself is not autostarted, so background mail checking needs
  Evolution to be running.
- Exchange/EWS and MAPI support are separate upstream modules and are not
  built. Bogofilter and SpamAssassin filters are not shipped. PST import,
  Markdown composition, text highlighting and contact maps are disabled
  because their dependencies (libpst, cmark, highlight, libchamplain) are
  not in the build SDK or, for libchamplain, not in the 26.04 archive.
- Opening attachments works from non-hidden home directories. Launching
  host applications that are not in the snap is limited to what the
  desktop portal provides, the Evolution help files have no shipped yelp
  viewer, and spell checking only has the en_US dictionary shipped in
  gnome-core26.
- The snap runs its own EDS; it does not use the host `calendar-service`
  or `contacts-service`. This is deliberate: the host EDS in the 26.04
  archive is 3.56.2, while this snap needs 3.62.1.

## Validation

Performed locally on the 26.04 host:

```console
python3 -c "import yaml; yaml.safe_load(open('evolution/snapcraft.yaml'))"
python3 -c "import yaml; yaml.safe_load(open('evolution/metadata.yaml'))"
cd /home/chrb/git/snaps && ./build evolution --dry-run
snapcraft expand-extensions   # validates the recipe and the gnome extension expansion
```

`apt-cache show` was used to confirm every build and stage package exists
in the 26.04 archive:

```
build (EDS):   gperf, libgoa-1.0-dev, libical-dev
build (Evo):   intltool, iso-codes, libgnome-autoar-0-dev, libical-dev,
               libytnef0-dev
stage:         libical3t64, libytnef0, libgnome-autoar-0-0, libxml2-16
```

No snapcraft or LXD build was run.

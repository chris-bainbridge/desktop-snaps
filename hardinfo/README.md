# hardinfo

Strictly confined HardInfo2 snap for Ubuntu Core 26 / Core Desktop.

## Upstream choice

The original repository, [lpereira/hardinfo](https://github.com/lpereira/hardinfo),
is no longer the maintained upstream. Its README carries a warning that the
repository "is being decommissioned and will remain as a personal archive" and
that development continues in
[hardinfo2/hardinfo2](https://github.com/hardinfo2/hardinfo2). It has no
tagged releases (the source still says `0.6-alpha`), and distributions have
moved on: Ubuntu 26.04 ships `hardinfo2` 2.2.16 and keeps `hardinfo` only as a
transitional package that depends on `hardinfo2`.

This recipe therefore uses hardinfo2, pinned to the latest stable release,
**2.3.1** (tag `release-2.3.1`, released 2026-07-03). The release tarball is
the tag archive from GitHub:

    https://github.com/hardinfo2/hardinfo2/archive/refs/tags/release-2.3.1.tar.gz
    sha256:59b30378127dde8e0af92de1a23ebcee706820a862581a603543a99c357bcecb

## Build system

HardInfo2 builds with CMake and GTK3 by default. The recipe uses the
`gnome` extension on `core26`, so GTK3, GLib/GIO, cairo, pango, gdk-pixbuf,
json-glib, libsoup 3, X11, zlib, gettext, the C toolchain and CMake come from
`gnome-core26-sdk`. Only `libsensors-dev` (plus `make`) is added as a build
package.

The upstream top-level `CMakeLists.txt` sets
`CMAKE_INSTALL_PREFIX "/usr"` unconditionally, overriding any `-D` value.
`override-pull` rewrites that line to `/snap/hardinfo/current/usr`, which is
where the snap is mounted at run time; `organize` then moves the installed
tree back under `usr/`. Without this, the compiled-in `PREFIX` and `LIBPREFIX`
(`/usr/share/hardinfo2` and `/usr/lib/hardinfo2`) would point into the core26
base instead of the snap.

CMake options:

* `HARDINFO2_GTK3=1`, `HARDINFO2_LIBSOUP3=1` — the versions provided by
  `gnome-core26`.
* `HARDINFO2_SERVICE=0` — a strict snap cannot install or run upstream's root
  systemd helper. The helper script (`hwinfo2_fetch_sysdata`) is also
  excluded from prime.
* `HARDINFO2_QT5=0`, `HARDINFO2_QT6=0`, `HARDINFO2_VK=0` — the OpenGL and
  Vulkan benchmark panels are optional and would require the complete Qt5
  and Vulkan/libdecor/glslang stacks. core26 has no Qt extension, and those
  stacks are disproportionate for two benchmark panels.

Only `libsensors5` is staged at run time (`usr/lib/*/libsensors.so.*`). Every
other runtime library is in `gnome-core26`, `mesa-2604` (through the GNOME
extension's `gpu-2604` plug) or the `core26` base.

## Strict-confinement limitations

Strict confinement is mandatory: these snaps target Core Desktop, which has
no classic snaps. The main consequences are documented in `snapcraft.yaml`
and summarised here:

* **Root-only data is unavailable.** Upstream's helper service normally
  pre-collects root-only information. Without it, there is no dmidecode (the
  detailed SMBIOS tables in `/sys/firmware/dmi/tables` are mode 0400), no
  DIMM/SPD EEPROM data, no IPMI sensors, and no debugfs MMC/AMDGPU values.
  The world-readable DMI strings in `/sys/devices/virtual/dmi/id` still work
  with `hardware-observe`.
* **Resources page is mostly blank.** `/proc/iomem` and `/proc/ioports` are
  readable but their addresses are zeroed for unprivileged readers.
* **Some benchmark and network helpers are missing.** `sysbench`, `iperf3`,
  `fwupd`, `vulkan-tools`, `mesa-utils` and `x11-xserver-utils` are not
  shipped. The built-in CPU, memory, zlib and GTK drawing benchmarks run.
  The OpenGL (Qt5) and Vulkan panels are not built.
* **The OS page describes the base.** `/usr/lib/os-release` and
  `/etc/os-release` inside the snap belong to `core26`, so the OS page
  reports Ubuntu Core 26 rather than the host distribution. The `dpkg -l`
  probe used to identify derivatives also finds no dpkg in the base. The
  Users page reads the base's `/etc/passwd`, so it lists the base's
  accounts, not the host's.
* **No udisks2 plug.** The `udisks2` interface is privileged and must be
  connected manually. Storage information comes from `/sys` and `lsblk`;
  udisks2-only fields such as some drive models and SMART attributes are
  absent.

`hardware-observe` and `system-observe` are not auto-connected. Connect them
after installation:

    sudo snap connect hardinfo:hardware-observe
    sudo snap connect hardinfo:system-observe

`home` is also manual (for saving reports); `network` and `gsettings` are
auto-connected.

## Validation

    python3 -c "import yaml; yaml.safe_load(open('hardinfo/snapcraft.yaml'))"
    python3 -c "import yaml; yaml.safe_load(open('hardinfo/metadata.yaml'))"
    ./build hardinfo --dry-run

The recipe was not built with snapcraft/LXD here. A native CMake build of
upstream 2.3.1 with the same options (`HARDINFO2_SERVICE=0`,
`HARDINFO2_QT5=0`, `HARDINFO2_QT6=0`, `HARDINFO2_VK=0`) succeeded on the
host's 26.04 toolchain, including a build with `libsensors` enabled.

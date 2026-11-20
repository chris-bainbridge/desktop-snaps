# geforcenow

NVIDIA distributes the GeForce NOW Linux client only as a flatpak. The recipe
repacks that flatpak OSTree repository at build time, so a build needs network
access to `international.download.nvidia.com`.

It always pulls whatever NVIDIA currently publishes on the `master` branch, so
re-running the build picks up their latest release and there is no version to
bump in `snapcraft.yaml`.

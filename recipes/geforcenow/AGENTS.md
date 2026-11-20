# geforcenow

## Pinned dependencies

The `sdl2` part in `snapcraft.yaml` must stay on the SDL **2.x** line
(`source-tag: release-2.*`). Never bump it to `release-3.x`: NVIDIA's client
links `libSDL2-2.0.so.0` and the build swaps in this SDL2 build, which SDL3
cannot replace. Version-bump checks must skip major-version changes here.

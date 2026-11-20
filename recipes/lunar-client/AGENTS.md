# lunar-client

## Upstream release detection

Lunar Client is closed source with no public repository or release feed, so the
generic `snap-repo sync` checkers do not apply. Query the download endpoint
directly:

1. Resolve the redirect target of
   `https://api.lunarclientprod.com/site/download?os=linux`:

   ```console
   curl -sIL -o /dev/null -w '%{url_effective}' "https://api.lunarclientprod.com/site/download?os=linux"
   ```

2. The resulting URL contains `Lunar%20Client-<VERSION>-`; the version is the
   `[0-9.]+` sequence immediately after `Lunar%20Client-`.
3. Compare it with the published edge-channel version from the Snap Store API:

   ```console
   curl -s -H "Snap-Device-Series: 16" https://api.snapcraft.io/v2/snaps/info/lunar-client \
     | jq -r '.["channel-map"][] | select(.channel.name=="edge" and .channel.architecture=="amd64") | .version'
   ```

There is no `version:` field to bump in `snapcraft.yaml`: the build downloads
and repackages the current AppImage, so a differing version only means a
rebuild is due.

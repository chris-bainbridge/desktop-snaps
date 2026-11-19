# projectlibre-snap

This is the `snapcraft.yaml` file to build the snap package of  ProjectLibre.

## How to build the package

The build downloads the pinned ProjectLibre release from
[SourceForge](https://sourceforge.net/projects/projectlibre/files/) and
verifies it against a SHA-256 checksum recorded in `snapcraft.yaml`, so there
is no manual download step. To move to a new upstream release, update
`version` and `checksum` in the `override-pull` of the `projectlibre` part.

From the repository root, run the command:
```
$ snapcraft
```

The Snap version is taken from the archive name.

## Known Issues
Printing works via the `cups` interface (verified end-to-end against a test
queue). File association for ProjectLibre's `.pod` documents is limited by a
glob clash with Perl POD in shared-mime-info; a real fix needs an upstream
change.

## Contact Information
If you have a problem with the packaging as a snap don't hesitate to contact
me. If it's an issue with the software itselft please contact the upstream
developer.

Thanks.


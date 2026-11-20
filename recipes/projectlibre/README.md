# projectlibre

Upstream has been frozen at 1.9.8 since 2021. The recipe pins that release and
fetches it from SourceForge in `override-pull`, verifying a SHA-256 checksum, so
a clean checkout needs no manual download. Build it with
`./snap-repo build projectlibre`.

## Known issues

- Printing works through the `cups` interface, verified end-to-end against a
  test queue.
- `.pod` documents cannot be associated with ProjectLibre: the glob clashes
  with Perl POD in shared-mime-info, so host file managers still treat them as
  Perl documentation. A real fix needs an upstream extension or MIME-type
  change.

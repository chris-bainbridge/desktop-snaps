# thrive

## Upstream release detection

Upstream releases live at
`https://github.com/Revolutionary-Games/Thrive/releases`. The recipe's `source:`
is a full release-asset download URL
(`https://github.com/Revolutionary-Games/Thrive/releases/download/v$SNAPCRAFT_PROJECT_VERSION/...`),
not a bare repository URL, so the upstream slug cannot be inferred from the
end of the string alone.

To check for a new version:

- Query `https://api.github.com/repos/Revolutionary-Games/Thrive/releases/latest`
  (excludes drafts and pre-releases) and strip the leading `v` from `tag_name`.
- Compare against the top-level `version:` field in `snap/snapcraft.yaml` and
  update it directly if different.

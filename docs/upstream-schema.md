# Upstream metadata schema

`metadata.yaml` records where a package's upstream project lives. It never
records the current version: the recipe is the source of truth for what is
built, and the update tool compares upstream tags against the recipe's
`source-tag` (or reports packages whose version is embedded in a URL or a
script).

The schema is deliberately small and provider-neutral:

```yaml
upstream:
  provider: github            # github | gitlab | sourceforge | launchpad | git | manual
  host: github.com            # optional; defaults per provider
  project: owner/repository   # provider-specific; required unless manual
  tag-pattern: '^v?(\d+(?:\.\d+)*)$'   # optional; exactly one capture group
  # url: https://example.com/download   # manual only
  # reason: no release feed             # manual only
```

Only the fields that apply are present. Unknown keys are an error in the
tool, so the schema stays honest.

## Providers

| Provider | `project` | `host` | Version discovery |
|---|---|---|---|
| `github` | `owner/repo` | defaults to `github.com` | releases, then tags |
| `gitlab` | full path, e.g. `GNOME/gnome-calculator` or `World/design/app-icon-preview` | always written; GitLab is many instances | tags API |
| `sourceforge` | project name, e.g. `bluefish` | defaults to `sourceforge.net` | project RSS/releases |
| `launchpad` | project name, e.g. `homebank` | `git.launchpad.net` | `git ls-remote --tags` |
| `git` | path below the host, e.g. `repos/lyx` | required | `git ls-remote --tags` |
| `manual` | omitted | omitted | none; the tool reports it |

`provider: git` covers any Git host that is not GitHub or GitLab, including
Gitea/Forgejo instances such as `altlinux.space` and `librewolf.dev`. `svn`
repositories are recorded as `manual` with their URL and a reason; v1 has no
SVN provider.

`provider: manual` is a terminal state, not a failure. It means the project
is known but has no machine-readable release feed. `url` and `reason` are
required so a later tool or a human can revisit it.

## Tag patterns

The default pattern is:

```text
^v?(\d+(?:\.\d+)*)$
```

It accepts `1.2.3` and `v1.2.3`. Projects that decorate tags set a
`tag-pattern` with one capture group:

| Package | Tag | Pattern |
|---|---|---|
| `deluge` | `deluge-2.2.0` | `^deluge-(\d+(?:\.\d+)*)$` |
| `easytag` | `easytag-2.4.3` | `^easytag-(\d+(?:\.\d+)*)$` |
| `frogr` | `RELEASE_1.8.1` | `^RELEASE_(\d+(?:\.\d+)*)$` |
| `glabels` | `glabels-3_4_1` | `^glabels-(\d+(?:_\d+)*)$` |
| `glade` | `GLADE_3_40_0` | `^GLADE_(\d+(?:_\d+)*)$` |
| `quodlibet` | `release-4.7.1` | `^release-(\d+(?:\.\d+)*)$` |

The captured group is used for version-aware ordering and, when a recipe has
a literal `version:`, for the new version string. For `glabels` and `glade`
the capture contains underscores; the tool normalizes `_` to `.` for display
versions. Tags that do not match the pattern are ignored. If no tag matches,
the package is reported rather than guessed at.

## Examples

GNOME, `adopt-info`, source-tag:

```yaml
upstream:
  provider: gitlab
  host: gitlab.gnome.org
  project: GNOME/gnome-calculator
```

GitHub with a literal version:

```yaml
upstream:
  provider: github
  project: gnome-terminator/terminator
```

A decorated tag:

```yaml
upstream:
  provider: github
  project: deluge-torrent/deluge
  tag-pattern: '^deluge-(\d+(?:\.\d+)*)$'
```

A vendor download page:

```yaml
upstream:
  provider: manual
  url: https://www.xm1math.net/texmaker/
  reason: vendor download page; no machine-readable release feed
```

## Migration notes

Metadata written before 2026-11-19 used a free-form scalar:

```yaml
upstream: https://gitlab.gnome.org/GNOME/gnome-calculator (50.1)
```

The parenthetical version was only a hint and was dropped. The provider and
project are derived from the URL. The current version remains in the recipe:
`source-tag` for tag mode, a literal `version:` for URL mode, or the script
for `override-pull`.

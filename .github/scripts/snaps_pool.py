#!/usr/bin/env python3
"""Manage the rolling "snaps" release used as the CI build pool.

The release holds one asset per buildable package track, named
``<package>-<track>-<version>.snap``.  ``pool.json`` maps each
``<package>:<track>`` selector to the asset that is currently buildable from
``main``.  A newer build replaces the entry and the previous asset is deleted
immediately; assets whose package or track is no longer in the repository are
deleted immediately as well.

The script only calls the GitHub API through ``gh``; it never touches the
repository working tree.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile

TAG = "snaps"
MANIFEST = "pool.json"
NOTES = (
    "Rolling CI build pool. Each asset is the latest successful build of one "
    "package track from main. These are test builds, not store releases."
)


class GhError(RuntimeError):
    pass


def gh(*args: str, check: bool = True) -> str:
    """Run a gh command and return its stdout."""
    command = ["gh", *args]
    process = subprocess.run(command, text=True, capture_output=True)
    if check and process.returncode != 0:
        detail = process.stderr.strip() or process.stdout.strip()
        raise GhError(f"{' '.join(command)} failed: {detail}")
    return process.stdout


def repository(explicit: str | None) -> str:
    if explicit:
        return explicit
    value = os.environ.get("GITHUB_REPOSITORY")
    if value:
        return value
    return gh("repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner").strip()


def release_exists(repo: str) -> bool:
    process = subprocess.run(
        ["gh", "release", "view", TAG, "--repo", repo, "--json", "tagName"],
        text=True,
        capture_output=True,
    )
    return process.returncode == 0


def ensure_release(repo: str) -> None:
    if not release_exists(repo):
        print(f"creating release {TAG}")
        gh("release", "create", TAG, "--repo", repo, "--prerelease", "--title", TAG, "--notes", NOTES)


def release_id(repo: str) -> int:
    release = json.loads(gh("api", f"repos/{repo}/releases/tags/{TAG}"))
    return int(release["id"])


def list_assets(repo: str) -> list[dict]:
    identifier = release_id(repo)
    assets: list[dict] = []
    page = 1
    while True:
        batch = json.loads(
            gh("api", f"repos/{repo}/releases/{identifier}/assets?per_page=100&page={page}")
        )
        assets.extend(batch)
        if len(batch) < 100:
            return assets
        page += 1


def download_manifest(repo: str, workdir: str, assets: list[dict]) -> dict:
    if not any(asset["name"] == MANIFEST for asset in assets):
        return {}
    gh("release", "download", TAG, "--repo", repo, "--pattern", MANIFEST, "--dir", workdir, "--clobber")
    path = os.path.join(workdir, MANIFEST)
    with open(path, encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        raise ValueError(f"{MANIFEST} is not a JSON object")
    return data


def delete_asset(repo: str, asset: dict) -> None:
    gh("api", "-X", "DELETE", f"repos/{repo}/releases/assets/{asset['id']}")
    print(f"deleted {asset['name']}")


def upload_asset(repo: str, path: str, name: str, workdir: str, assets: list[dict]) -> None:
    """Upload path as a release asset named name, replacing any existing asset."""
    existing = next((asset for asset in assets if asset["name"] == name), None)
    if existing is None:
        if os.path.basename(path) != name:
            path = copy_into(workdir, path, name)
        gh("release", "upload", TAG, path, "--repo", repo)
        print(f"uploaded {name}")
        return

    # Upload under a temporary name first, then delete the old asset and
    # rename, so a failed upload never destroys the working asset.
    temporary_name = f"{name}.{os.getpid()}.tmp"
    temporary_path = copy_into(workdir, path, temporary_name)
    gh("release", "upload", TAG, temporary_path, "--repo", repo)
    refreshed = list_assets(repo)
    temporary = next((asset for asset in refreshed if asset["name"] == temporary_name), None)
    if temporary is None:
        raise GhError(f"uploaded {temporary_name} but cannot find it in the release")
    delete_asset(repo, existing)
    gh(
        "api",
        "-X",
        "PATCH",
        f"repos/{repo}/releases/assets/{temporary['id']}",
        "-f",
        f"name={name}",
    )
    print(f"replaced {name}")


def copy_into(workdir: str, source: str, name: str) -> str:
    destination = os.path.join(workdir, name)
    shutil.copyfile(source, destination)
    return destination


def write_manifest(workdir: str, manifest: dict) -> str:
    path = os.path.join(workdir, MANIFEST)
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return path


def load_current(path: str) -> list[dict]:
    with open(path, encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, list):
        raise ValueError(f"{path} is not a JSON array")
    return data


def current_keys(current: list[dict]) -> set[str]:
    return {f"{row['name']}:{row['track']}" for row in current}


def parse_selector(asset_name: str, current: list[dict]) -> tuple[str, str] | None:
    """Return (selector, version) for an asset name, or None.

    Asset names are ``<package>-<track>-<version>.snap``.  Package names may
    contain hyphens and even track names, so choose the longest matching
    selector prefix.
    """
    if not asset_name.endswith(".snap"):
        return None
    best: tuple[str, str, str] | None = None
    for row in current:
        name = row["name"]
        track = row["track"]
        prefix = f"{name}-{track}-"
        if not asset_name.startswith(prefix):
            continue
        version = asset_name[len(prefix) : -len(".snap")]
        if not version:
            continue
        if best is None or len(prefix) > len(best[0]):
            best = (prefix, f"{name}:{track}", version)
    if best is None:
        return None
    return best[1], best[2]


def reconstruct(assets: list[dict], current: list[dict]) -> dict:
    """Rebuild a manifest from the release assets, newest asset per selector."""
    chosen: dict[str, dict] = {}
    for asset in sorted(assets, key=lambda asset: (asset.get("created_at", ""), asset["name"])):
        parsed = parse_selector(asset["name"], current)
        if parsed is None:
            continue
        selector, version = parsed
        chosen[selector] = {
            "asset": asset["name"],
            "version": version,
            "commit": "",
        }
    return chosen


def reconcile(manifest: dict, current: list[dict], assets: list[dict]) -> tuple[dict, list[dict]]:
    """Drop entries that are no longer buildable and return assets to delete."""
    keys = current_keys(current)
    by_name = {asset["name"]: asset for asset in assets if asset["name"] != MANIFEST}
    referenced: set[str] = set()
    removed: list[dict] = []

    for selector in sorted(manifest):
        entry = manifest[selector]
        if selector not in keys:
            asset = by_name.get(entry.get("asset"))
            if asset is not None:
                removed.append(asset)
            del manifest[selector]
            print(f"removed {selector} (no longer in main)")
            continue
        if entry.get("asset") not in by_name:
            del manifest[selector]
            print(f"removed {selector} entry (asset is missing)")
            continue
        referenced.add(entry["asset"])

    for name, asset in sorted(by_name.items()):
        if name not in referenced:
            removed.append(asset)

    # Do not delete the same asset twice.
    unique: dict[str, dict] = {asset["name"]: asset for asset in removed}
    return manifest, sorted(unique.values(), key=lambda asset: asset["name"])


def publish(args: argparse.Namespace, workdir: str) -> None:
    repo = repository(args.repo)
    current = load_current(args.current)
    local_assets = sorted(
        name for name in os.listdir(args.assets) if name.endswith(".snap")
    )

    if not local_assets:
        print("no built snaps; reconciling only")
        if not release_exists(repo):
            print("release does not exist; nothing to do")
            return
        assets = list_assets(repo)
        manifest = download_manifest(repo, workdir, assets)
        if not manifest:
            manifest = reconstruct(assets, current)
        before = json.dumps(manifest, sort_keys=True)
        manifest, removed = reconcile(manifest, current, assets)
        if json.dumps(manifest, sort_keys=True) != before:
            upload_asset(repo, write_manifest(workdir, manifest), MANIFEST, workdir, assets)
        else:
            print("manifest unchanged")
        for asset in removed:
            delete_asset(repo, asset)
        return

    ensure_release(repo)
    assets = list_assets(repo)
    manifest = download_manifest(repo, workdir, assets) or reconstruct(assets, current)

    for name in local_assets:
        parsed = parse_selector(name, current)
        if parsed is None:
            raise ValueError(f"{name} does not match a package track in main")
        selector, version = parsed
        upload_asset(repo, os.path.join(args.assets, name), name, workdir, assets)
        manifest[selector] = {
            "asset": name,
            "version": version,
            "commit": args.commit,
        }
        assets = list_assets(repo)

    manifest, removed = reconcile(manifest, current, assets)
    manifest_path = write_manifest(workdir, manifest)
    upload_asset(repo, manifest_path, MANIFEST, workdir, assets)
    for asset in removed:
        delete_asset(repo, asset)
    print(f"pool has {len(manifest)} package track(s)")


def prune(args: argparse.Namespace, workdir: str) -> None:
    repo = repository(args.repo)
    if not release_exists(repo):
        print("release does not exist; nothing to prune")
        return
    current = load_current(args.current)
    assets = list_assets(repo)
    manifest = download_manifest(repo, workdir, assets)
    if not manifest:
        manifest = reconstruct(assets, current)
        print("rebuilt a missing manifest from the release assets")
    manifest, removed = reconcile(manifest, current, assets)
    upload_asset(repo, write_manifest(workdir, manifest), MANIFEST, workdir, assets)
    for asset in removed:
        delete_asset(repo, asset)
    print(f"pool has {len(manifest)} package track(s)")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", help="GitHub repository (default: $GITHUB_REPOSITORY)")
    subparsers = parser.add_subparsers(dest="command", required=True)

    publish_parser = subparsers.add_parser("publish", help="upload built snaps and reconcile")
    publish_parser.add_argument("--assets", required=True, help="directory of *.snap files")
    publish_parser.add_argument("--current", required=True, help="JSON from `snap-repo list --json`")
    publish_parser.add_argument("--commit", default=os.environ.get("GITHUB_SHA", ""))

    prune_parser = subparsers.add_parser("prune", help="delete assets no longer buildable")
    prune_parser.add_argument("--current", required=True, help="JSON from `snap-repo list --json`")

    args = parser.parse_args(argv)
    if not shutil.which("gh"):
        print("gh is required", file=sys.stderr)
        return 1
    try:
        with tempfile.TemporaryDirectory(prefix="snaps-pool-") as workdir:
            if args.command == "publish":
                publish(args, workdir)
            else:
                prune(args, workdir)
    except (GhError, ValueError, OSError) as error:
        print(f"snaps_pool: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

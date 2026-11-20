import unittest

import snaps_pool


def row(name, track="stable", version="1.0"):
    return {"name": name, "track": track, "version": version, "base": "core26", "publisher": None}


CURRENT = [
    row("gnome-logs"),
    row("five-or-more"),
    row("foo", "edge"),
    row("foo-edge"),
]


class ParseSelectorTests(unittest.TestCase):
    def test_matches_a_simple_asset(self):
        self.assertEqual(
            snaps_pool.parse_selector("gnome-logs-stable-50.0.snap", CURRENT),
            ("gnome-logs:stable", "50.0"),
        )

    def test_chooses_the_longest_selector_prefix(self):
        self.assertEqual(
            snaps_pool.parse_selector("foo-edge-9.10.snap", CURRENT),
            ("foo:edge", "9.10"),
        )
        self.assertEqual(
            snaps_pool.parse_selector("foo-edge-stable-1.2.snap", CURRENT),
            ("foo-edge:stable", "1.2"),
        )

    def test_ignores_unknown_or_non_snap_assets(self):
        self.assertIsNone(snaps_pool.parse_selector("removed-stable-1.0.snap", CURRENT))
        self.assertIsNone(snaps_pool.parse_selector("gnome-logs-stable-50.0.zip", CURRENT))
        self.assertIsNone(snaps_pool.parse_selector("gnome-logs-stable-.snap", CURRENT))


class ReconcileTests(unittest.TestCase):
    def test_deletes_superseded_and_orphaned_assets(self):
        manifest = {
            "gnome-logs:stable": {"asset": "gnome-logs-stable-49.0.snap", "version": "49.0"},
            "removed:stable": {"asset": "removed-stable-1.0.snap", "version": "1.0"},
        }
        assets = [
            {"name": "gnome-logs-stable-49.0.snap", "id": 1, "created_at": "2026-11-01"},
            {"name": "gnome-logs-stable-50.0.snap", "id": 2, "created_at": "2026-11-20"},
            {"name": "removed-stable-1.0.snap", "id": 3, "created_at": "2026-11-01"},
            {"name": "stray.snap", "id": 4, "created_at": "2026-11-01"},
        ]
        manifest["gnome-logs:stable"] = {
            "asset": "gnome-logs-stable-50.0.snap",
            "version": "50.0",
        }

        remaining, removed = snaps_pool.reconcile(manifest, CURRENT, assets)

        self.assertEqual(
            remaining,
            {"gnome-logs:stable": {"asset": "gnome-logs-stable-50.0.snap", "version": "50.0"}},
        )
        self.assertEqual(
            [asset["name"] for asset in removed],
            ["gnome-logs-stable-49.0.snap", "removed-stable-1.0.snap", "stray.snap"],
        )

    def test_drops_entries_for_missing_assets(self):
        manifest = {"gnome-logs:stable": {"asset": "gone.snap", "version": "1.0"}}
        remaining, removed = snaps_pool.reconcile(manifest, CURRENT, [])
        self.assertEqual(remaining, {})
        self.assertEqual(removed, [])

    def test_never_removes_the_manifest(self):
        manifest = {"gnome-logs:stable": {"asset": "gnome-logs-stable-50.0.snap", "version": "50.0"}}
        assets = [{"name": snaps_pool.MANIFEST, "id": 1}, {"name": "gnome-logs-stable-50.0.snap", "id": 2}]
        remaining, removed = snaps_pool.reconcile(dict(manifest), CURRENT, assets)
        self.assertIn("gnome-logs:stable", remaining)
        self.assertEqual(removed, [])


class ReconstructTests(unittest.TestCase):
    def test_picks_the_newest_asset_per_selector(self):
        assets = [
            {"name": "gnome-logs-stable-49.0.snap", "created_at": "2026-11-01"},
            {"name": "gnome-logs-stable-50.0.snap", "created_at": "2026-11-20"},
            {"name": "gone-stable-1.0.snap", "created_at": "2026-11-20"},
        ]
        manifest = snaps_pool.reconstruct(assets, CURRENT)
        self.assertEqual(
            manifest,
            {
                "gnome-logs:stable": {
                    "asset": "gnome-logs-stable-50.0.snap",
                    "version": "50.0",
                    "commit": "",
                }
            },
        )


if __name__ == "__main__":
    unittest.main()

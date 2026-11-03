import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1] / "tools"
sys.path.insert(0, str(TOOLS))
SPEC = importlib.util.spec_from_file_location("android_prepare", TOOLS / "android_prepare.py")
prepare = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(prepare)


class AndroidPrepareTests(unittest.TestCase):
    def test_download_uses_the_locked_immutable_manifest(self):
        manifest_sha1 = "a" * 40
        client_sha1 = "c" * 40
        server_sha1 = "d" * 40
        manifest = {
            "downloads": {
                "client": {"url": "https://example.invalid/client.jar", "sha1": client_sha1},
                "server": {"url": "https://example.invalid/server.jar", "sha1": server_sha1},
            },
            "libraries": [],
            "assetIndex": {
                "id": "34",
                "url": "https://example.invalid/assets.json",
                "sha1": "b" * 40,
            },
        }
        pins = {
            "version": "26.3",
            "manifest_sha1": manifest_sha1,
            "client_sha1": client_sha1,
            "server_sha1": server_sha1,
        }

        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "research-lock.json").write_text(
                json.dumps({"minecraft": pins}), encoding="utf-8"
            )
            destination = root / "minecraft"
            downloads = []

            def download(url, path, checksum):
                path = Path(path)
                downloads.append((url, path, checksum))
                path.parent.mkdir(parents=True, exist_ok=True)
                if path.name == "26.3.json":
                    path.write_text(json.dumps(manifest), encoding="utf-8")
                elif path.parent.name == "indexes":
                    path.write_text(json.dumps({"objects": {}}), encoding="utf-8")
                else:
                    raise AssertionError("Unexpected direct download: " + str(path))

            with (
                patch.object(prepare, "ROOT", root),
                patch.object(prepare, "fetch_json", side_effect=AssertionError("mutable catalog used")),
                patch.object(prepare.lab, "download", side_effect=download),
                patch.object(prepare.lab, "client_library_artifacts", return_value=[]),
                patch.object(prepare, "parallel_download") as parallel_download,
            ):
                result, server = prepare.download_game("26.3", destination)

        expected_url = (
            "https://piston-meta.mojang.com/v1/packages/"
            + manifest_sha1
            + "/26.3.json"
        )
        self.assertEqual(downloads[0], (
            expected_url,
            destination / "versions" / "26.3" / "26.3.json",
            manifest_sha1,
        ))
        self.assertEqual(result, manifest)
        self.assertEqual(server, destination / "versions" / "26.3" / "26.3-server.jar")
        self.assertEqual(len(downloads), 2)
        parallel_download.assert_called_once()


if __name__ == "__main__":
    unittest.main()

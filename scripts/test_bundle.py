import hashlib
import json
import os
import plistlib
import tempfile
import unittest
import zipfile
from pathlib import Path
from dataclasses import replace
from unittest.mock import patch

import bundle


class PackagingContracts(unittest.TestCase):
    def fixture(self, root, profile="debug", target=None, host="x86_64-pc-windows-msvc", local_ocr=True, portable=False):
        target_dir = root / "custom build cache"
        output_dir = target_dir / target / profile if target else target_dir / profile
        output_dir.mkdir(parents=True)
        triple = target or host
        executable = output_dir / ("MinnowSnap.exe" if "windows" in triple else "MinnowSnap")
        executable.write_bytes(b"current executable")
        icons = output_dir / "build" / "minnow-app-current" / "out" / "icons"
        icons.mkdir(parents=True)
        (icons / "icon.ico").write_bytes(b"current ico")
        (icons / "icon.icns").write_bytes(b"current icns")
        manifest = root / "app" / "Cargo.toml"
        manifest.parent.mkdir(exist_ok=True)
        (manifest.parent / "Info.plist.extra").write_text(
            "<key>NSUserNotificationAlertStyle</key><string>banner</string>"
        )
        package = {
            "name": bundle.APP_PACKAGE,
            "id": "path+file:///app#minnow-app@0.1.0",
            "version": "0.1.0",
            "manifest_path": str(manifest),
            "metadata": {"bundle": {
                "identifier": "com.lortunate.minnowsnap",
                "osx_info_plist_exts": ["Info.plist.extra"],
            }},
        }
        recognition = {"name": "minnow-recognition", "id": "path+file:///recognition#minnow-recognition@0.1.0"}
        metadata = {"packages": [package, recognition], "target_directory": str(target_dir)}
        messages = [
            {"reason": "build-script-executed", "package_id": package["id"], "out_dir": str(icons.parent)},
            {
                "reason": "compiler-artifact", "package_id": recognition["id"],
                "target": {"name": "minnow_recognition", "kind": ["lib"]},
                "features": ["default", "local-ocr"] if local_ocr else [],
            },
            {
                "reason": "compiler-artifact", "package_id": package["id"],
                "target": {"name": bundle.APP_NAME, "kind": ["bin"]},
                "executable": str(executable), "fresh": True,
                "features": (["default", "local-ocr"] if local_ocr else []) + (["portable"] if portable else []),
                "profile": {
                    "opt_level": "0" if profile == "debug" else "3",
                    "debuginfo": 0,
                    "debug_assertions": profile == "debug",
                    "overflow_checks": profile == "debug",
                    "test": False,
                },
            },
            {"reason": "build-finished", "success": True},
        ]
        return metadata, messages, executable

    def resolve(self, metadata, messages, host="x86_64-pc-windows-msvc", explicit_base=False):
        return replace(
            bundle.artifact_from_messages(metadata, map(json.dumps, messages), host, explicit_base=explicit_base),
            source_revision="a" * 40, source_dirty=False,
        )

    def test_custom_target_dir_profile_and_target_use_reported_paths(self):
        for profile, target in [
            ("debug", None),  # Cargo dev profile output, including a cached artifact.
            ("release", "x86_64-pc-windows-msvc"),
            ("dist", "aarch64-apple-darwin"),  # A custom Cargo profile.
        ]:
            with self.subTest(profile=profile, target=target), tempfile.TemporaryDirectory() as tmp:
                metadata, messages, executable = self.fixture(Path(tmp), profile, target)
                artifact = self.resolve(metadata, messages)
                self.assertEqual(artifact.executable, executable.resolve())
                self.assertEqual(artifact.deploy_dir, Path(metadata["target_directory"]).resolve() / "deploy")
                self.assertEqual(artifact.target_triple, target or "x86_64-pc-windows-msvc")
                self.assertEqual(artifact.profile, profile)

    def test_stale_executable_cannot_replace_missing_or_failed_build_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            metadata, messages, executable = self.fixture(root)
            guessed = root / "target" / "release" / "MinnowSnap.exe"
            guessed.parent.mkdir(parents=True)
            guessed.write_bytes(b"stale executable")
            cases = [
                messages[:-1],  # No completed build.
                [*messages[:-1], {"reason": "build-finished", "success": False}],
                [messages[0], messages[-1]],  # No reported executable.
            ]
            for case in cases:
                with self.subTest(case=case):
                    with self.assertRaises(ValueError):
                        self.resolve(metadata, case)
            executable.unlink()
            with self.assertRaises(ValueError):
                self.resolve(metadata, messages)

    def test_windows_archive_contains_only_current_reported_executable(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            metadata, messages, executable = self.fixture(root, "dist", "aarch64-pc-windows-msvc")
            artifact = self.resolve(metadata, messages)
            stage = root / "stage"
            stage.mkdir()
            options = bundle.DistOptions(False, False, "dist", False)
            archive, manifest = bundle.dist_windows(artifact, options, stage)
            with zipfile.ZipFile(archive) as package:
                self.assertEqual(set(package.namelist()), {"MinnowSnap/MinnowSnap.exe", "MinnowSnap/build-manifest.json"})
                payload = package.read("MinnowSnap/MinnowSnap.exe")
                self.assertEqual(payload, executable.read_bytes())
                recorded = json.loads(package.read("MinnowSnap/build-manifest.json"))
                self.assertEqual(recorded, manifest)
                self.assertEqual(recorded["executable"]["sha256"], hashlib.sha256(payload).hexdigest())
                self.assertEqual(recorded["profile"], "dist")
                self.assertEqual(recorded["data_layout"], "installed")
                self.assertEqual(recorded["ocr"], "full")
                self.assertEqual(recorded["source"]["revision"], "a" * 40)
            self.assertIn("aarch64-pc-windows-msvc-dist-installed-full-", archive.name)
            compressed_stage = root / "compressed"
            compressed_stage.mkdir()
            with patch.object(bundle, "maybe_compress_exe_with_upx", side_effect=lambda path, **kwargs: path.write_bytes(b"packed executable")):
                compressed, packed_manifest = bundle.dist_windows(artifact, options, compressed_stage)
            with zipfile.ZipFile(compressed) as package:
                self.assertEqual(package.read("MinnowSnap/MinnowSnap.exe"), b"packed executable")
            self.assertEqual(packed_manifest["executable"]["sha256"], hashlib.sha256(b"packed executable").hexdigest())
            self.assertEqual(packed_manifest["executable"]["cargo_sha256"], hashlib.sha256(executable.read_bytes()).hexdigest())

    def test_macos_bundle_uses_current_binary_icons_and_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            metadata, messages, executable = self.fixture(root, "debug", "aarch64-apple-darwin")
            executable.chmod(0o755)
            artifact = self.resolve(metadata, messages)
            contents = bundle.create_macos_app(artifact, root / "stage") / "Contents"
            self.assertEqual((contents / "MacOS" / bundle.APP_NAME).read_bytes(), b"current executable")
            if os.name != "nt":
                self.assertTrue((contents / "MacOS" / bundle.APP_NAME).stat().st_mode & 0o111)
            self.assertEqual((contents / "Resources" / "icon.icns").read_bytes(), b"current icns")
            info = plistlib.loads((contents / "Info.plist").read_bytes())
            self.assertEqual(info["CFBundleExecutable"], bundle.APP_NAME)
            self.assertEqual(info["CFBundleIdentifier"], "com.lortunate.minnowsnap")
            self.assertEqual(info["CFBundleVersion"], "0.1.0")
            self.assertEqual(info["NSUserNotificationAlertStyle"], "banner")
            manifest = json.loads((contents / "Resources" / bundle.BUILD_MANIFEST).read_text())
            self.assertEqual(manifest["target"], "aarch64-apple-darwin")
            self.assertEqual(manifest["profile"], "debug")
            self.assertEqual(manifest["executable"]["sha256"], hashlib.sha256(b"current executable").hexdigest())
            (artifact.icons / "icon.icns").unlink()
            with self.assertRaisesRegex(ValueError, "icon missing"):
                self.resolve(metadata, messages)

    def test_intel_macos_is_rejected_before_build_and_after_cargo_target_resolution(self):
        for target, environment in [
            ("x86_64-apple-darwin", {}),
            (None, {"CARGO_BUILD_TARGET": "x86_64-apple-darwin"}),
            (None, {}),
        ]:
            with self.subTest(target=target, environment=environment):
                options = bundle.DistOptions(False, False, "release", False, target=target)
                with patch.dict(os.environ, environment, clear=True), patch.object(
                    bundle.subprocess, "check_output",
                    side_effect=['{"packages":[]}', "host: x86_64-apple-darwin\n"],
                ), patch.object(bundle.subprocess, "Popen") as build:
                    with self.assertRaisesRegex(ValueError, "no verified ONNX Runtime"):
                        bundle.cargo_build(options)
                    build.assert_not_called()
        # A target selected through Cargo configuration must also be rejected,
        # even when the host and command-line preflight appeared supported.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            metadata, messages, executable = self.fixture(root, "release", "x86_64-apple-darwin")
            with self.assertRaisesRegex(ValueError, "no verified ONNX Runtime"):
                self.resolve(metadata, messages, host="aarch64-apple-darwin")
            intel = bundle.BuildArtifact(
                executable, Path(messages[0]["out_dir"]) / "icons",
                Path(metadata["target_directory"]), "x86_64-apple-darwin", metadata["packages"][0],
            )
            with self.assertRaisesRegex(ValueError, "no verified ONNX Runtime"):
                bundle.create_macos_app(intel, root / "stage")
            self.assertFalse((root / "stage").exists())

    def test_intel_base_requires_explicit_request_and_reported_absence_of_local_ocr(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            metadata, messages, _ = self.fixture(root, "release", "x86_64-apple-darwin", local_ocr=False)
            with self.assertRaisesRegex(ValueError, "no verified ONNX Runtime"):
                self.resolve(metadata, messages, host="aarch64-apple-darwin")
            artifact = self.resolve(metadata, messages, host="aarch64-apple-darwin", explicit_base=True)
            self.assertFalse(artifact.local_ocr)
            self.assertIn("-release-installed-base-", artifact.filename("dmg"))
            contents = bundle.create_macos_app(artifact, root / "stage") / "Contents"
            self.assertTrue((contents / "MacOS" / bundle.APP_NAME).is_file())

            messages[1]["features"] = ["local-ocr"]
            with self.assertRaisesRegex(ValueError, "unexpectedly enables local-ocr"):
                self.resolve(metadata, messages, explicit_base=True)
            messages[1].pop("features")
            with self.assertRaisesRegex(ValueError, "resolved features"):
                self.resolve(metadata, messages, explicit_base=True)
            with self.assertRaisesRegex(ValueError, "resolved features"):
                self.resolve(metadata, [messages[0], *messages[2:]], explicit_base=True)

    def test_base_build_forwards_no_default_features_and_verifies_cargo_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            metadata, messages, _ = self.fixture(Path(tmp), "release", "x86_64-apple-darwin", local_ocr=False)
            options = bundle.DistOptions(
                False, False, "release", False, target="x86_64-apple-darwin",
                features="portable", no_default_features=True,
            )
            with patch.dict(os.environ, {}, clear=True), patch.object(
                bundle.subprocess, "check_output",
                side_effect=[json.dumps(metadata), "host: aarch64-apple-darwin\n"],
            ), patch.object(bundle, "read_source_revision", return_value=("b" * 40, False)), patch.object(bundle.subprocess, "Popen") as build:
                process = build.return_value.__enter__.return_value
                process.stdout = list(map(json.dumps, messages))
                process.wait.return_value = 0
                artifact = bundle.cargo_build(options)
                self.assertIn("--no-default-features", build.call_args.args[0])
                self.assertEqual(build.call_args.args[0][-2:], ["--features", "portable"])
                self.assertTrue(artifact.explicit_base)
                self.assertFalse(artifact.local_ocr)
                # A requested feature is not proof it was enabled: identity
                # must follow the compiler-artifact's actual feature list.
                self.assertEqual(artifact.data_layout, "installed")
                self.assertEqual(artifact.source_revision, "b" * 40)

    def test_base_request_cannot_enable_local_ocr_through_feature_aliases(self):
        for features in ["local-ocr", "minnow-recognition/local-ocr", "minnow-app/default", "portable,local-ocr"]:
            options = bundle.DistOptions(False, False, "release", False, features=features, no_default_features=True)
            with self.subTest(features=features), patch.object(bundle.subprocess, "Popen") as build:
                with self.assertRaisesRegex(ValueError, "cannot request"):
                    bundle.cargo_build(options)
                build.assert_not_called()

    def test_profiles_layouts_ocr_and_additional_resolved_features_have_distinct_names(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            names = set()
            for profile in ["debug", "release"]:
                for portable in [False, True]:
                    for local_ocr in [False, True]:
                        case = root / f"{profile}-{portable}-{local_ocr}"
                        metadata, messages, _ = self.fixture(case, profile, portable=portable, local_ocr=local_ocr)
                        artifact = self.resolve(metadata, messages)
                        names.add(artifact.filename("zip"))
            self.assertEqual(len(names), 8)
            metadata, messages, _ = self.fixture(root / "diagnostics", "release")
            standard = self.resolve(metadata, messages).filename("zip")
            messages[2]["features"].append("overlay-diagnostics")
            diagnostic = self.resolve(metadata, messages).filename("zip")
            self.assertNotEqual(standard, diagnostic)

    def test_incomplete_or_ambiguous_cargo_identity_cannot_be_packaged(self):
        with tempfile.TemporaryDirectory() as tmp:
            metadata, messages, _ = self.fixture(Path(tmp))
            for missing in ["features", "profile"]:
                current = json.loads(json.dumps(messages))
                current[2].pop(missing)
                with self.subTest(missing=missing), self.assertRaisesRegex(ValueError, "resolved"):
                    self.resolve(metadata, current)
            ambiguous = json.loads(json.dumps(messages[2]))
            ambiguous["executable"] += ".other"
            with self.assertRaisesRegex(ValueError, "multiple application executables"):
                self.resolve(metadata, [*messages[:-1], ambiguous, messages[-1]])
            artifact = self.resolve(metadata, messages)
            with self.assertRaisesRegex(ValueError, "source revision"):
                bundle.write_build_manifest(replace(artifact, source_revision=""), artifact.executable, Path(tmp))


if __name__ == "__main__":
    unittest.main()

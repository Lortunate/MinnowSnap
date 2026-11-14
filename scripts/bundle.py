import argparse
import hashlib
import json
import os
import platform
import plistlib
import shutil
import subprocess
import sys
import tempfile
import zipfile
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Iterable, Sequence

APP_NAME = "MinnowSnap"
APP_PACKAGE = "minnow-app"
BUILD_MANIFEST = "build-manifest.json"
PROJECT_ROOT = Path(__file__).resolve().parent.parent
WINDOWS_DEBUG_GLOBS = ("*.pdb", "*.ilk", "*.exp", "*.lib", "*.a", "*.cmake")


@dataclass(frozen=True)
class DistOptions:
    upx: bool
    upx_aggressive: bool
    cargo_profile: str
    zip_lzma: bool
    target: str | None = None
    features: str | None = None
    no_default_features: bool = False


@dataclass(frozen=True)
class BuildArtifact:
    """Paths emitted by this Cargo invocation, including fresh cached artifacts."""

    executable: Path
    icons: Path
    target_dir: Path
    target_triple: str
    package: dict
    local_ocr: bool = True
    explicit_base: bool = False
    profile: str = ""
    profile_settings: dict = field(default_factory=dict)
    resolved_features: dict[str, list[str]] = field(default_factory=dict)
    source_revision: str = ""
    source_dirty: bool = True

    @property
    def data_layout(self) -> str:
        return "portable" if "portable" in self.resolved_features[APP_PACKAGE] else "installed"

    @property
    def ocr_variant(self) -> str:
        return "full" if self.local_ocr else "base"

    @property
    def configuration_id(self) -> str:
        data = {"profile": self.profile_settings, "features": self.resolved_features}
        return hashlib.sha256(json.dumps(data, sort_keys=True, separators=(",", ":")).encode()).hexdigest()[:12]

    @property
    def system(self) -> str:
        if "windows" in self.target_triple:
            return "windows"
        if "apple-darwin" in self.target_triple:
            return "macos"
        raise ValueError(f"Unsupported packaging target: {self.target_triple}")

    @property
    def arch(self) -> str:
        return self.target_triple.split("-")[0]

    @property
    def deploy_dir(self) -> Path:
        return self.target_dir / "deploy"

    def filename(self, extension: str) -> str:
        prefix = "minnowsnap" if self.system == "windows" else APP_NAME
        return (
            f"{prefix}-v{self.package['version']}-{self.target_triple}-{self.profile}"
            f"-{self.data_layout}-{self.ocr_variant}-{self.configuration_id}.{extension}"
        )


def print_action(action: str, message: str) -> None:
    print(f"{action:>12} {message}")


def fail(message: str) -> None:
    print(message)
    raise SystemExit(1)


def run_command(cmd: Sequence[str], *, cwd: Path = PROJECT_ROOT, silent: bool = False) -> None:
    cmd_list = list(cmd)
    run_kwargs: dict[str, object] = {"cwd": cwd, "check": True}
    if silent:
        run_kwargs["stdout"] = subprocess.PIPE
        run_kwargs["stderr"] = subprocess.PIPE
    try:
        subprocess.run(cmd_list, **run_kwargs)
    except subprocess.CalledProcessError as exc:
        if silent:
            if exc.stdout:
                sys.stdout.buffer.write(exc.stdout)
            if exc.stderr:
                sys.stderr.buffer.write(exc.stderr)
        fail(f"\nError executing command: {' '.join(cmd_list)}")


def get_bool_env(name: str, default: bool) -> bool:
    value = os.getenv(name)
    if value is None:
        return default
    return value.strip().lower() in {"1", "true", "yes", "on"}


def build_options(args: argparse.Namespace) -> DistOptions:
    profile = (args.profile or os.getenv("MINNOWSNAP_CARGO_PROFILE") or "release").strip()
    return DistOptions(
        upx=args.upx or get_bool_env("MINNOWSNAP_USE_UPX", False),
        upx_aggressive=args.upx_aggressive or get_bool_env("MINNOWSNAP_UPX_AGGRESSIVE", False),
        cargo_profile="dev" if profile == "debug" else profile,
        zip_lzma=get_bool_env("MINNOWSNAP_ZIP_LZMA", False),
        target=args.target,
        features=args.features,
        no_default_features=args.no_default_features,
    )


def get_dir_size(path: Path) -> int:
    total = 0
    for entry in path.rglob("*"):
        if entry.is_file():
            try:
                total += entry.stat().st_size
            except OSError:
                continue
    return total


def format_size(size_bytes: int) -> str:
    return f"{size_bytes / (1024 * 1024):.2f} MB"


def top_files(path: Path, limit: int = 15) -> list[Path]:
    if not path.exists():
        return []
    files = [p for p in path.rglob("*") if p.is_file()]
    files.sort(key=lambda p: p.stat().st_size, reverse=True)
    return files[:limit]


def create_zip(src_dir: Path, zip_path: Path, *, use_lzma: bool) -> None:
    compression = zipfile.ZIP_LZMA if use_lzma else zipfile.ZIP_DEFLATED
    zip_kwargs = {"compression": compression}
    if not use_lzma:
        zip_kwargs["compresslevel"] = 9

    with zipfile.ZipFile(zip_path, "w", **zip_kwargs) as zf:
        for root, _, files in os.walk(src_dir):
            for file in files:
                file_path = Path(root) / file
                try:
                    arcname = file_path.relative_to(src_dir.parent)
                    zf.write(file_path, arcname)
                except ValueError:
                    zf.write(file_path, file_path.name)


def maybe_compress_exe_with_upx(exe_path: Path, *, enabled: bool, aggressive: bool) -> None:
    if not enabled:
        return
    upx = shutil.which("upx")
    if not upx:
        print_action("Warning", "UPX enabled but upx not found; skip executable compression")
        return

    backup_path = exe_path.with_suffix(exe_path.suffix + ".unpacked")
    shutil.copy2(exe_path, backup_path)

    if aggressive:
        cmd = [upx, "--best", "--lzma", str(exe_path)]
        mode = "aggressive"
    else:
        cmd = [
            upx,
            "-1",
            "--lzma",
            "--compress-exports=0",
            "--compress-icons=0",
            "--compress-resources=0",
            "--strip-relocs=0",
            str(exe_path),
        ]
        mode = "safe"

    print_action("Compressing", f"executable with upx ({mode} mode)")

    try:
        subprocess.run(cmd, cwd=PROJECT_ROOT, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        subprocess.run([upx, "-t", str(exe_path)], cwd=PROJECT_ROOT, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except subprocess.CalledProcessError as exc:
        sys.stdout.buffer.write(exc.stdout or b"")
        sys.stderr.buffer.write(exc.stderr or b"")
        shutil.copy2(backup_path, exe_path)
        print_action("Warning", "UPX failed or test failed; restored original executable")
    finally:
        if not get_bool_env("MINNOWSNAP_KEEP_UPX_BACKUP", False):
            backup_path.unlink(missing_ok=True)


def print_bundle_report(bundle_dir: Path, *, max_files: int = 12) -> None:
    print_action("Report", "largest files in bundle")
    for path in top_files(bundle_dir, limit=max_files):
        rel = path.relative_to(bundle_dir).as_posix()
        print(f"            {format_size(path.stat().st_size):>10}  {rel}")


def validate_packaging_target(target: str, *, local_ocr: bool = True, explicit_base: bool = False) -> None:
    if target == "x86_64-apple-darwin" and (local_ocr or not explicit_base):
        raise ValueError(
            "Intel macOS packages with local OCR are unavailable: no verified ONNX Runtime library "
            "and dependency bundle is supplied. Use --no-default-features for an explicit base "
            "package without local-ocr, or pass an explicitly supported --target."
        )


def read_source_revision() -> tuple[str, bool]:
    revision = subprocess.check_output(["git", "rev-parse", "--verify", "HEAD"], cwd=PROJECT_ROOT, text=True).strip()
    if len(revision) not in (40, 64) or any(character not in "0123456789abcdef" for character in revision):
        raise ValueError("Git did not report a full source revision")
    dirty = bool(subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=normal"], cwd=PROJECT_ROOT
    ).strip())
    return revision, dirty


def write_build_manifest(artifact: BuildArtifact, executable: Path, directory: Path) -> dict:
    if not artifact.source_revision:
        raise ValueError("Cannot package an artifact without a source revision")
    with executable.open("rb") as packaged, artifact.executable.open("rb") as cargo:
        packaged_hash = hashlib.file_digest(packaged, "sha256").hexdigest()
        cargo_hash = hashlib.file_digest(cargo, "sha256").hexdigest()
    manifest = {
        "schema_version": 1,
        "application": APP_NAME,
        "version": artifact.package["version"],
        "target": artifact.target_triple,
        # Cargo JSON does not report a logical profile name. Use its actual
        # output directory (dev -> debug), together with reported codegen flags.
        "profile": artifact.profile,
        "cargo_profile": artifact.profile_settings,
        "data_layout": artifact.data_layout,
        "ocr": artifact.ocr_variant,
        "resolved_features": artifact.resolved_features,
        "configuration_id": artifact.configuration_id,
        "source": {"revision": artifact.source_revision, "dirty": artifact.source_dirty},
        "executable": {
            "name": executable.name,
            "sha256": packaged_hash,
            "cargo_sha256": cargo_hash,
        },
    }
    (directory / BUILD_MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return manifest


def artifact_from_messages(metadata: dict, messages: Iterable[str], host_triple: str, *, explicit_base: bool = False) -> BuildArtifact:
    """Resolve only reported artifacts; never fall back to a guessed executable."""
    package = next(p for p in metadata["packages"] if p["name"] == APP_PACKAGE)
    recognition = next(p for p in metadata["packages"] if p["name"] == "minnow-recognition")
    executable = None
    icons = None
    finished = False
    workspace = {p["id"]: p["name"] for p in metadata["packages"]}
    resolved_features: dict[str, list[str]] = {}
    profile_settings = None
    for line in messages:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            print(line.rstrip())
            continue
        reason = message.get("reason")
        if reason == "build-finished":
            finished = message["success"]
        if reason == "compiler-message":
            rendered = message.get("message", {}).get("rendered")
            if rendered:
                print(rendered, end="", file=sys.stderr)
        target = message.get("target", {})
        is_application = target.get("name") == APP_NAME and "bin" in target.get("kind", [])
        is_workspace_library = "lib" in target.get("kind", [])
        if reason == "compiler-artifact" and message.get("package_id") in workspace and (is_application or is_workspace_library):
            reported_features = message.get("features")
            if not isinstance(reported_features, list) or not all(isinstance(feature, str) for feature in reported_features):
                raise ValueError(f"Cargo did not report {workspace[message['package_id']]}'s resolved features")
            name = workspace[message["package_id"]]
            features = sorted(set(reported_features))
            if name in resolved_features and resolved_features[name] != features:
                raise ValueError(f"Cargo reported multiple resolved feature sets for {name}; package one target at a time")
            resolved_features[name] = features
        if message.get("package_id") != package["id"]:
            continue
        if reason == "build-script-executed":
            icons = Path(message["out_dir"]) / "icons"
        if reason == "compiler-artifact" and is_application:
            if message.get("executable"):
                if executable is not None and executable != Path(message["executable"]):
                    raise ValueError("Cargo reported multiple application executables; package one target at a time")
                executable = Path(message["executable"])
                profile_settings = message.get("profile")
    if not finished:
        raise ValueError("Cargo did not report a successful completed build")
    if executable is None or not executable.is_file():
        raise ValueError("Cargo did not report an existing MinnowSnap executable")
    if icons is None:
        raise ValueError("Cargo did not report minnow-app's generated resources")
    if recognition["name"] not in resolved_features or APP_PACKAGE not in resolved_features:
        raise ValueError("Cargo did not report the application's and minnow-recognition's resolved features")
    if not isinstance(profile_settings, dict) or not {"opt_level", "debug_assertions", "test"} <= profile_settings.keys():
        raise ValueError("Cargo did not report the executable's resolved profile settings")
    if profile_settings["test"] is not False:
        raise ValueError("A test executable cannot be packaged as the application")
    local_ocr = "local-ocr" in resolved_features[recognition["name"]]
    if explicit_base and local_ocr:
        raise ValueError("Explicit base package unexpectedly enables local-ocr in Cargo's resolved features")
    target_dir = Path(metadata["target_directory"]).resolve()
    executable = executable.resolve()
    # Cargo maps dev -> debug itself. A target build has one extra directory.
    relative = executable.relative_to(target_dir)
    if len(relative.parts) not in (2, 3):
        raise ValueError(f"Unexpected Cargo executable layout: {executable}")
    target_triple = relative.parts[0] if len(relative.parts) == 3 else host_triple
    # Also reject targets selected through Cargo configuration, whose actual
    # output is authoritative even when --target / CARGO_BUILD_TARGET is absent.
    validate_packaging_target(target_triple, local_ocr=local_ocr, explicit_base=explicit_base)
    artifact = BuildArtifact(
        executable, icons.resolve(), target_dir, target_triple, package, local_ocr, explicit_base,
        profile=relative.parts[-2], profile_settings=profile_settings, resolved_features=resolved_features,
    )
    icon = artifact.icons / ("icon.ico" if artifact.system == "windows" else "icon.icns")
    if not icon.is_file():
        raise ValueError(f"Generated bundle icon missing: {icon}")
    return artifact


def cargo_build(options: DistOptions) -> BuildArtifact:
    selected_features = set((options.features or "").replace(",", " ").split())
    local_ocr = not options.no_default_features or any(feature.rsplit("/", 1)[-1] in {"local-ocr", "default"} for feature in selected_features)
    if options.no_default_features and local_ocr:
        raise ValueError("A base package cannot request the default or local-ocr feature")
    requested_target = options.target or os.getenv("CARGO_BUILD_TARGET")
    if requested_target:
        validate_packaging_target(requested_target, local_ocr=local_ocr, explicit_base=options.no_default_features)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=PROJECT_ROOT, text=True
    ))
    rustc_info = subprocess.check_output(["rustc", "-vV"], cwd=PROJECT_ROOT, text=True)
    host_triple = next(line.removeprefix("host: ") for line in rustc_info.splitlines() if line.startswith("host: "))
    if not requested_target:
        # Require an explicit supported target when crossing from an Intel
        # host, rather than risking an expensive unsupported native build.
        validate_packaging_target(host_triple, local_ocr=local_ocr, explicit_base=options.no_default_features)
    cmd = [
        "cargo", "build", "--locked", "-p", APP_PACKAGE, "--bin", APP_NAME,
        "--profile", options.cargo_profile, "--message-format=json-render-diagnostics",
    ]
    if options.target:
        cmd.extend(["--target", options.target])
    if options.no_default_features:
        cmd.append("--no-default-features")
    if options.features:
        cmd.extend(["--features", options.features])
    revision, dirty_before = read_source_revision()
    print_action("Building", f"profile={options.cargo_profile} (cargo)")
    with subprocess.Popen(cmd, cwd=PROJECT_ROOT, stdout=subprocess.PIPE, text=True, encoding="utf-8") as process:
        try:
            artifact = artifact_from_messages(metadata, process.stdout, host_triple, explicit_base=options.no_default_features)
        except (ValueError, KeyError, StopIteration):
            # Drain stdout before waiting; Cargo may still be writing diagnostics.
            for line in process.stdout:
                print(line.rstrip(), file=sys.stderr)
            process.wait()
            raise
        if process.wait() != 0:
            raise subprocess.CalledProcessError(process.returncode, cmd)
    revision_after, dirty_after = read_source_revision()
    if revision != revision_after:
        raise ValueError("Source revision changed while Cargo was building; rerun packaging")
    artifact = replace(artifact, source_revision=revision, source_dirty=dirty_before or dirty_after)
    print_action("Artifact", str(artifact.executable))
    return artifact


def prune_windows_bundle(bundle_dir: Path) -> int:
    to_remove: set[Path] = set()
    for pattern in WINDOWS_DEBUG_GLOBS:
        to_remove.update(bundle_dir.rglob(pattern))

    removed_count = 0
    for path in sorted(to_remove, key=lambda p: len(p.parts), reverse=True):
        if path.exists():
            path.unlink(missing_ok=True)
            removed_count += 1
    return removed_count


def dist_windows(artifact: BuildArtifact, options: DistOptions, stage: Path) -> tuple[Path, dict]:
    bundle_dir = stage / APP_NAME
    bundle_dir.mkdir()
    bundle_exe = bundle_dir / f"{APP_NAME}.exe"
    shutil.copy2(artifact.executable, bundle_exe)

    before_prune = get_dir_size(bundle_dir)
    removed = prune_windows_bundle(bundle_dir)
    print_action("Slimming", f"removed {removed} debug artifacts")
    maybe_compress_exe_with_upx(bundle_exe, enabled=options.upx, aggressive=options.upx_aggressive)
    manifest = write_build_manifest(artifact, bundle_exe, bundle_dir)
    after_prune = get_dir_size(bundle_dir)

    print_action("Slimming", f"bundle size {format_size(before_prune)} -> {format_size(after_prune)}")
    print_bundle_report(bundle_dir)

    zip_name = artifact.filename("zip")
    zip_path = stage / zip_name
    print_action("Packaging", zip_name)
    create_zip(bundle_dir, zip_path, use_lzma=options.zip_lzma)
    return zip_path, manifest


def create_macos_app(artifact: BuildArtifact, stage: Path) -> Path:
    validate_packaging_target(artifact.target_triple, local_ocr=artifact.local_ocr, explicit_base=artifact.explicit_base)
    bundle = artifact.package["metadata"]["bundle"]
    bundle_path = stage / f"{APP_NAME}.app"
    contents = bundle_path / "Contents"
    executable_dir = contents / "MacOS"
    resources_dir = contents / "Resources"
    executable_dir.mkdir(parents=True)
    resources_dir.mkdir()
    shutil.copy2(artifact.executable, executable_dir / APP_NAME)
    shutil.copy2(artifact.icons / "icon.icns", resources_dir / "icon.icns")
    info = {
        "CFBundleExecutable": APP_NAME,
        "CFBundleIdentifier": bundle["identifier"],
        "CFBundleName": APP_NAME,
        "CFBundleDisplayName": APP_NAME,
        "CFBundlePackageType": "APPL",
        "CFBundleIconFile": "icon.icns",
        "CFBundleShortVersionString": artifact.package["version"],
        "CFBundleVersion": artifact.package["version"],
        "NSHighResolutionCapable": True,
    }
    manifest_dir = Path(artifact.package["manifest_path"]).parent
    for extra in bundle.get("osx_info_plist_exts", []):
        fragment = (manifest_dir / extra).read_text(encoding="utf-8")
        info.update(plistlib.loads(f"<plist><dict>{fragment}</dict></plist>".encode()))
    with (contents / "Info.plist").open("wb") as output:
        plistlib.dump(info, output)
    write_build_manifest(artifact, executable_dir / APP_NAME, resources_dir)
    return bundle_path


def dist_macos(artifact: BuildArtifact, stage: Path) -> tuple[Path, dict]:
    if platform.system() != "Darwin":
        raise ValueError("macOS DMG packaging requires a macOS host")
    volume = stage / "volume"
    bundle_path = create_macos_app(artifact, volume)
    manifest = json.loads((bundle_path / "Contents" / "Resources" / BUILD_MANIFEST).read_text(encoding="utf-8"))
    (volume / "Applications").symlink_to("/Applications", target_is_directory=True)
    dmg_path = stage / artifact.filename("dmg")
    print_action("Packaging", "DMG installer (macOS hdiutil)")
    cmd = [
        "/usr/bin/hdiutil", "create", "-volname", f"{APP_NAME} Installer",
        "-srcfolder", str(volume), "-format", "UDZO", str(dmg_path),
    ]
    run_command(cmd)
    if not dmg_path.is_file():
        raise ValueError("hdiutil succeeded without producing a DMG")
    return dmg_path, manifest


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=f"Build and package {APP_NAME}")
    parser.add_argument("--profile", help="Cargo profile (default: MINNOWSNAP_CARGO_PROFILE or release)")
    parser.add_argument("--target", help="Cargo target triple (otherwise Cargo's configured target)")
    parser.add_argument("--features", help="Cargo features, for example portable or portable,overlay-diagnostics")
    parser.add_argument("--no-default-features", action="store_true", help="Build an explicit base package with screenshot and QR support, without local OCR")
    parser.add_argument(
        "--upx",
        action="store_true",
        help=f"Enable UPX compression for {APP_NAME}.exe",
    )
    parser.add_argument(
        "--upx-aggressive",
        action="store_true",
        help="Use aggressive UPX mode (smaller, but less stable)",
    )
    return parser.parse_args()


def main() -> None:
    options = build_options(parse_args())
    try:
        artifact = cargo_build(options)
        artifact.deploy_dir.mkdir(parents=True, exist_ok=True)
        # Publish only complete packages, keeping a previous package on failure.
        with tempfile.TemporaryDirectory(prefix=".bundle-", dir=artifact.deploy_dir) as temporary:
            stage = Path(temporary)
            if artifact.system == "windows":
                package, manifest = dist_windows(artifact, options, stage)
            else:
                package, manifest = dist_macos(artifact, stage)
            output = artifact.deploy_dir / package.name
            os.replace(package, output)
        print_action("Finished", f"output: {output}")
        if github_output := os.getenv("GITHUB_OUTPUT"):
            with Path(github_output).open("a", encoding="utf-8") as stream:
                stream.write(f"artifact={output}\n")
                stream.write(f"build_manifest={json.dumps(manifest, sort_keys=True, separators=(',', ':'))}\n")
    except (OSError, ValueError, KeyError, StopIteration, subprocess.CalledProcessError) as error:
        fail(f"Packaging failed: {error}")


if __name__ == "__main__":
    main()

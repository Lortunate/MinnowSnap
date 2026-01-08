import os
import sys
import shutil
import platform
import subprocess
import zipfile
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent.parent
TARGET_DIR = PROJECT_ROOT / "target" / "release"
DEPLOY_DIR = PROJECT_ROOT / "deploy"

def print_action(action, message):
    print(f"{action:>12} {message}")

def run_command(cmd, cwd=None, silent=False):
    try:
        if silent:
            result = subprocess.run(
                cmd, 
                cwd=cwd, 
                check=True, 
                stdout=subprocess.PIPE, 
                stderr=subprocess.PIPE
            )
        else:
            subprocess.run(cmd, cwd=cwd, check=True)
    except subprocess.CalledProcessError as e:
        if silent:
            if e.stdout:
                sys.stdout.buffer.write(e.stdout)
            if e.stderr:
                sys.stderr.buffer.write(e.stderr)
        print(f"\nError executing command: {' '.join(cmd)}")
        sys.exit(1)

def get_version():
    cargo_toml = PROJECT_ROOT / "Cargo.toml"
    with open(cargo_toml, 'r', encoding='utf-8') as f:
        for line in f:
            if line.strip().startswith('version'):
                return line.split('=')[1].strip().strip('"')
    return "0.0.0"

def clean_deploy_dir(name="MinnowSnap"):
    if DEPLOY_DIR.exists():
        shutil.rmtree(DEPLOY_DIR)
    
    dist_dir = DEPLOY_DIR / name
    dist_dir.mkdir(parents=True, exist_ok=True)
    return DEPLOY_DIR, dist_dir

def create_zip(src_dir, zip_path):
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zf:
        for root, _, files in os.walk(src_dir):
            for file in files:
                file_path = Path(root) / file
                try:
                    arcname = file_path.relative_to(DEPLOY_DIR)
                    zf.write(file_path, arcname)
                except ValueError:
                    zf.write(file_path, file_path.name)

def dist_windows():
    print_action("Building", "release binary (cargo)")
    run_command(["cargo", "build", "--release"], cwd=PROJECT_ROOT)

    deploy_dir, bundle_dir = clean_deploy_dir("MinnowSnap")
    
    exe_path = TARGET_DIR / "MinnowSnap.exe"
    if not exe_path.exists():
        print(f"Error: {exe_path} not found")
        sys.exit(1)
        
    shutil.copy2(exe_path, bundle_dir / "MinnowSnap.exe")

    print_action("Deploying", "Qt dependencies (windeployqt)")
    qml_dir = PROJECT_ROOT / "qml"
    run_command([
        "windeployqt",
        "--qmldir", str(qml_dir),
        "--release",
        "--no-translations",
        "--no-compiler-runtime",
        str(bundle_dir / "MinnowSnap.exe")
    ], cwd=PROJECT_ROOT, silent=True)

    print_action("Copying", "resources")
    shutil.copytree(qml_dir, bundle_dir / "qml", dirs_exist_ok=True)
    
    res_dir = PROJECT_ROOT / "resources"
    if res_dir.exists():
        shutil.copytree(res_dir, bundle_dir / "resources", dirs_exist_ok=True)

    version = get_version()
    zip_name = f"minnowsnap-v{version}-portable.zip"
    zip_path = deploy_dir / zip_name
    
    print_action("Packaging", zip_name)
    create_zip(bundle_dir, zip_path)
    
    print_action("Finished", f"output: {zip_path}")

def dist_macos():
    print_action("Building", "bundle (cargo bundle)")
    run_command(["cargo", "bundle", "--release"], cwd=PROJECT_ROOT)

    bundle_path = TARGET_DIR / "bundle" / "osx" / "MinnowSnap.app"
    if not bundle_path.exists():
        print(f"Error: {bundle_path} not found")
        sys.exit(1)

    print_action("Deploying", "Qt dependencies (macdeployqt)")
    qml_dir = PROJECT_ROOT / "qml"
    run_command([
        "macdeployqt",
        str(bundle_path),
        f"-qmldir={qml_dir}"
    ], cwd=PROJECT_ROOT, silent=True)

    if DEPLOY_DIR.exists():
        shutil.rmtree(DEPLOY_DIR)
    DEPLOY_DIR.mkdir(parents=True, exist_ok=True)

    version = get_version()
    dmg_name = f"MinnowSnap-v{version}.dmg"
    dmg_path = DEPLOY_DIR / dmg_name

    print_action("Packaging", "DMG installer (create-dmg)")
    
    if not shutil.which("create-dmg"):
        print("Error: create-dmg not found. Please install it (brew install create-dmg).")
        sys.exit(1)
    
    # Check for icon
    volicon = PROJECT_ROOT / "assets_icons" / "icon.icns"
    
    cmd = [
        "create-dmg",
        "--volname", "MinnowSnap Installer",
        "--window-pos", "200", "120",
        "--window-size", "600", "400",
        "--icon-size", "100",
        "--icon", "MinnowSnap.app", "175", "190",
        "--hide-extension", "MinnowSnap.app",
        "--app-drop-link", "425", "190",
        str(dmg_path),
        str(bundle_path)
    ]
    
    if volicon.exists():
        cmd.insert(1, str(volicon))
        cmd.insert(1, "--volicon")

    run_command(cmd, cwd=PROJECT_ROOT)
    print_action("Finished", f"output: {dmg_path}")

def main():
    system = platform.system()
    if system == "Windows":
        dist_windows()
    elif system == "Darwin":
        dist_macos()
    else:
        print(f"Unsupported OS: {system}")
        sys.exit(1)

if __name__ == "__main__":
    main()
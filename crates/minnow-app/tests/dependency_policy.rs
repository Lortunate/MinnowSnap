use serde_json::Value;
use std::path::Path;
use std::process::Command;

fn workspace_packages() -> Vec<Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--offline", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .expect("read workspace metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["packages"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn core_packages_do_not_depend_on_frameworks_or_native_backends() {
    let core = [
        "minnow-imaging",
        "minnow-capture",
        "minnow-editor",
        "minnow-settings",
        "minnow-application",
    ];
    let forbidden = [
        "gpui",
        "gpui-pre",
        "gpui-pre-platform",
        "gpui-component",
        "gpui-kit-assets",
        "xcap",
        "ort",
        "windows",
        "font-kit",
        "arboard",
        "notify-rust",
        "global-hotkey",
    ];
    for package in workspace_packages() {
        let name = package["name"].as_str().unwrap();
        if !core.contains(&name) {
            continue;
        }
        for dependency in package["dependencies"].as_array().unwrap() {
            let dependency_name = dependency["name"].as_str().unwrap();
            assert!(!forbidden.contains(&dependency_name), "{name} must not depend on {dependency_name}");
        }
    }
}

#[test]
fn workspace_dependencies_follow_capability_boundaries() {
    let rules: &[(&str, &[&str])] = &[
        ("minnow-imaging", &[]),
        ("minnow-capture", &["minnow-imaging"]),
        ("minnow-editor", &["minnow-imaging"]),
        ("minnow-settings", &[]),
        (
            "minnow-application",
            &["minnow-imaging", "minnow-capture", "minnow-editor", "minnow-settings"],
        ),
        ("minnow-recognition", &["minnow-application", "minnow-imaging"]),
        ("minnow-platform", &["minnow-application", "minnow-imaging", "minnow-capture"]),
        ("minnow-ui", &["minnow-application", "minnow-editor", "minnow-imaging"]),
    ];
    for package in workspace_packages() {
        let name = package["name"].as_str().unwrap();
        if name == "minnow-app" {
            continue;
        }
        let allowed = rules
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .expect("new package needs an explicit dependency policy")
            .1;
        for dependency in package["dependencies"].as_array().unwrap() {
            let dependency_name = dependency["name"].as_str().unwrap();
            if dependency_name.starts_with("minnow-") {
                assert!(allowed.contains(&dependency_name), "{name} may not depend on {dependency_name}");
            }
            if name != "minnow-ui" {
                assert!(
                    !matches!(dependency_name, "gpui" | "gpui-pre" | "gpui-pre-platform" | "gpui-component"),
                    "GPUI belongs to the UI and composition root, not {name}"
                );
            }
        }
    }
}

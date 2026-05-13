use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_repo_file(path: &str) -> String {
    fs::read_to_string(repo_root().join(path)).unwrap_or_else(|err| panic!("read {path}: {err}"))
}

fn rs_files_under(path: &str) -> Vec<PathBuf> {
    fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("read dir {}: {err}", dir.display())) {
            let entry = entry.expect("read dir entry");
            let path = entry.path();
            if path.is_dir() {
                collect(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }

    let mut files = Vec::new();
    collect(&repo_root().join(path), &mut files);
    files.sort();
    files
}

fn repo_relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .expect("repo relative path")
        .to_string_lossy()
        .replace('\\', "/")
}

fn manifest_array(manifest: &str, key: &str) -> Vec<String> {
    let mut lines = manifest.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.split('#').next().unwrap_or(line).trim();
        let Some((entry_key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if entry_key.trim() != key {
            continue;
        }

        let mut array_text = value.trim().to_string();
        while !array_text.contains(']') {
            let Some(next) = lines.next() else {
                break;
            };
            array_text.push('\n');
            array_text.push_str(next.split('#').next().unwrap_or(next).trim());
        }

        let Some((_, entries)) = array_text.split_once('[') else {
            return Vec::new();
        };
        let entries = entries.split(']').next().unwrap_or(entries);
        return entries
            .split(',')
            .filter_map(|entry| {
                let entry = entry.trim();
                if entry.is_empty() {
                    None
                } else {
                    Some(entry.trim_matches('"').to_string())
                }
            })
            .collect();
    }

    Vec::new()
}

fn code_before_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn use_statements(source: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();

    for line in source.lines() {
        let code = code_before_comment(line).trim();
        if code.is_empty() {
            continue;
        }

        if current.is_empty() {
            if !(code.starts_with("use ") || code.starts_with("pub use ")) {
                continue;
            }
        } else {
            current.push(' ');
        }

        current.push_str(code);
        if code.ends_with(';') {
            statements.push(std::mem::take(&mut current));
        }
    }

    statements
}

fn tokenize_use_path(use_statement: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = use_statement.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            ':' if chars.peek() == Some(&':') => {
                chars.next();
                tokens.push("::".to_string());
            }
            '{' | '}' | ',' | ';' | '*' => tokens.push(ch.to_string()),
            ch if ch.is_ascii_alphabetic() || ch == '_' => {
                let mut ident = String::from(ch);
                while let Some(next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || *next == '_' {
                        ident.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(ident);
            }
            _ => {}
        }
    }

    tokens
}

fn parse_use_entries(tokens: &[String], mut index: usize, prefix: &[String]) -> (Vec<Vec<String>>, usize) {
    let mut paths = Vec::new();

    while index < tokens.len() && tokens[index] != "}" && tokens[index] != ";" {
        let (mut parsed, next_index) = parse_use_path(tokens, index, prefix);
        paths.append(&mut parsed);
        index = next_index;

        if index < tokens.len() && tokens[index] == "," {
            index += 1;
        }
    }

    if index < tokens.len() && tokens[index] == "}" {
        index += 1;
    }

    (paths, index)
}

fn parse_use_path(tokens: &[String], mut index: usize, prefix: &[String]) -> (Vec<Vec<String>>, usize) {
    let mut path = prefix.to_vec();

    while index < tokens.len() {
        match tokens[index].as_str() {
            "," | "}" | ";" => break,
            "::" => {
                index += 1;
            }
            "{" => {
                return parse_use_entries(tokens, index + 1, &path);
            }
            "*" => {
                path.push("*".to_string());
                index += 1;
            }
            segment => {
                path.push(segment.to_string());
                index += 1;
            }
        }
    }

    (vec![path], index)
}

fn expanded_use_paths(use_statement: &str) -> Vec<Vec<String>> {
    let trimmed = use_statement.trim();
    let statement = trimmed.strip_prefix("pub ").unwrap_or(trimmed).strip_prefix("use ").unwrap_or(trimmed);
    let tokens = tokenize_use_path(statement);
    let (paths, _) = parse_use_entries(&tokens, 0, &[]);
    paths
}

fn imports_private_feature_module(use_statement: &str, target_feature: &str, private_module: &str) -> bool {
    let forbidden = ["crate", "ui", "features", target_feature, private_module];

    expanded_use_paths(use_statement).iter().any(|path| {
        path.windows(forbidden.len())
            .any(|window| window.iter().map(String::as_str).eq(forbidden))
    })
}

fn code_line_mentions_shell_window_api(line: &str) -> bool {
    let code = code_before_comment(line).trim();
    if code.is_empty() {
        return false;
    }

    [
        "WindowOptions",
        "Bounds<Pixels>",
        "set_window_level",
        "activate_window",
        "window_handle",
        "raw_window_handle",
    ]
    .iter()
    .any(|needle| code.contains(needle))
}

#[test]
fn private_feature_import_matcher_handles_direct_and_grouped_paths() {
    for import in [
        "use crate::ui::features::overlay::render::layout::OverlayPanelLayout;",
        "use crate::ui::features::overlay::{render::layout::OverlayPanelLayout};",
        "use crate::ui::features::{overlay::state::OverlayFrame};",
        "use crate::ui::features::{overlay::{state::OverlayFrame}};",
        "use crate::ui::{features::{overlay::{render::toolbar::ToolbarIcon}}};",
        "use crate::ui::features::{overlay::stateful::Thing, overlay::state::OverlayFrame};",
    ] {
        assert!(
            imports_private_feature_module(import, "overlay", "render") || imports_private_feature_module(import, "overlay", "state"),
            "matcher should flag {import}"
        );
    }
}

#[test]
fn private_feature_import_matcher_rejects_prefix_and_other_feature_paths() {
    for import in [
        "use crate::ui::features::overlay::stateful::Thing;",
        "use crate::ui::features::overlay_render::layout::Thing;",
        "use crate::ui::features::pin::state::PinSession;",
        "use crate::ui::features::{pin::{render::Thing}};",
    ] {
        assert!(
            !imports_private_feature_module(import, "overlay", "render") && !imports_private_feature_module(import, "overlay", "state"),
            "matcher should not flag {import}"
        );
    }
}

#[test]
fn workspace_stays_single_crate() {
    let workspace_manifest = read_repo_file("Cargo.toml");
    assert_eq!(
        manifest_array(&workspace_manifest, "members"),
        ["crates/minnow-app"],
        "workspace manifest should keep minnow-app as the only member"
    );
    assert_eq!(
        manifest_array(&workspace_manifest, "default-members"),
        ["crates/minnow-app"],
        "workspace default member should stay minnow-app"
    );

    let crates_dir = repo_root().join("crates");
    let mut crate_dirs = fs::read_dir(&crates_dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", crates_dir.display()))
        .map(|entry| entry.expect("crate entry"))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    crate_dirs.sort();

    assert_eq!(crate_dirs, ["minnow-app"]);

    let app_manifest = read_repo_file("crates/minnow-app/Cargo.toml");
    assert!(
        !app_manifest.contains("path = \"../"),
        "minnow-app should not depend on sibling workspace crates"
    );
}

#[test]
fn main_rs_stays_thin_runtime_entrypoint() {
    let main_rs = read_repo_file("crates/minnow-app/src/main.rs");
    let non_empty_lines = main_rs.lines().filter(|line| !line.trim().is_empty()).count();

    assert!(non_empty_lines <= 22, "main.rs should stay thin; found {non_empty_lines} non-empty lines");
    assert!(
        main_rs.contains("minnow_app::app::parse_command()") && main_rs.contains("minnow_app::app::run_command(command)"),
        "main.rs should delegate command parsing and execution to app"
    );

    for forbidden in [
        "mod ",
        "pub mod ",
        "gpui::",
        "tokio::",
        "minnow_app::platform",
        "minnow_app::services",
        "minnow_app::ui",
    ] {
        assert!(
            !main_rs.contains(forbidden),
            "main.rs should not own app internals or UI/platform wiring: {forbidden}"
        );
    }
}

#[test]
fn app_composition_is_the_top_composition_root() {
    let app_mod = read_repo_file("crates/minnow-app/src/app/mod.rs");
    let runtime_rs = read_repo_file("crates/minnow-app/src/app/runtime.rs");
    let composition_rs = read_repo_file("crates/minnow-app/src/app/composition.rs");

    assert!(app_mod.contains("pub mod composition;"), "app module should expose the composition root");
    assert!(
        runtime_rs.contains("use super::composition::run_application;"),
        "runtime should enter the app through composition"
    );
    assert!(
        composition_rs.contains("use crate::platform") && composition_rs.contains("use crate::services") && composition_rs.contains("use crate::ui"),
        "composition root should be the place where top-level owners are wired together"
    );
}

#[test]
fn shell_window_helpers_do_not_live_under_ui_support_or_services() {
    let mut misplaced = Vec::new();

    for file in rs_files_under("crates/minnow-app/src/ui/support")
        .into_iter()
        .chain(rs_files_under("crates/minnow-app/src/services"))
    {
        let rel = repo_relative(&file);
        let file_stem = file.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));

        let looks_like_shell_window_helper =
            matches!(file_stem, "windowing" | "native_window" | "window_drag") || source.lines().any(code_line_mentions_shell_window_api);

        if looks_like_shell_window_helper {
            misplaced.push(rel);
        }
    }

    assert!(
        misplaced.is_empty(),
        "shell/window helpers belong at the platform edge, not under ui/support or services: {misplaced:#?}"
    );
}

#[test]
fn features_do_not_import_other_features_private_render_or_state_modules() {
    let mut violations = Vec::new();

    for file in rs_files_under("crates/minnow-app/src/ui/features") {
        let rel = repo_relative(&file);
        let Some(source_feature) = rel
            .strip_prefix("crates/minnow-app/src/ui/features/")
            .and_then(|path| path.split('/').next())
        else {
            continue;
        };

        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        let imports = use_statements(&source);
        for target_feature in ["long_capture", "overlay", "pin", "preferences"] {
            if target_feature == source_feature {
                continue;
            }

            for private_module in ["render", "state"] {
                if imports
                    .iter()
                    .any(|import| imports_private_feature_module(import, target_feature, private_module))
                {
                    let forbidden = format!("crate::ui::features::{target_feature}::{private_module}");
                    violations.push(format!("{rel} imports {forbidden}"));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "features should use other features through public feature APIs only: {violations:#?}"
    );
}

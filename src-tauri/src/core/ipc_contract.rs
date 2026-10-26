//! Keeps the hand-written TS mirrors in `src/constants/` identical to what Rust
//! registers and emits.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path:?}: {err}"))
}

/// Parses `KEY: "value",` entries of a TS `as const` object. The formatter may
/// wrap a long value onto the next line, so whitespace is collapsed first.
fn ts_constants(relative: &str) -> BTreeMap<String, String> {
    let source = read(relative);
    let collapsed = source.split_whitespace().collect::<Vec<_>>().join(" ");

    collapsed
        .split(',')
        .filter_map(|entry| {
            let entry = entry.rsplit('{').next()?.trim();
            let (key, value) = entry.split_once(": \"")?;
            let value = value.strip_suffix('"')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

/// Last path segment of every handler in `generate_handler![...]`.
fn registered_commands() -> Vec<String> {
    let lib = read("src-tauri/src/lib.rs");
    let start = lib
        .find("generate_handler![")
        .expect("lib.rs registers handlers");
    let body = &lib[start + "generate_handler![".len()..];
    let body = &body[..body.find(']').expect("handler list closes")];

    body.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.rsplit("::").next().unwrap_or(entry).to_owned())
        .collect()
}

fn frontend_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read frontend dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            frontend_sources(&path, out);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("ts" | "tsx")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn every_registered_command_has_exactly_one_ts_constant() {
    let mut rust = registered_commands();
    rust.sort();
    let duplicates: Vec<_> = rust.windows(2).filter(|w| w[0] == w[1]).collect();
    assert!(duplicates.is_empty(), "duplicate handlers: {duplicates:?}");

    let mut ts: Vec<String> = ts_constants("src/constants/commands.ts")
        .into_values()
        .collect();
    ts.sort();

    let rust_only: Vec<_> = rust.iter().filter(|name| !ts.contains(name)).collect();
    let ts_only: Vec<_> = ts.iter().filter(|name| !rust.contains(name)).collect();
    assert!(
        rust_only.is_empty() && ts_only.is_empty(),
        "registered without TS constant: {rust_only:?}; TS constant without handler: {ts_only:?}"
    );
}

#[test]
fn every_ts_command_constant_is_used() {
    let mut sources = Vec::new();
    frontend_sources(&repo_root().join("src"), &mut sources);
    let code: String = sources
        .iter()
        .filter(|path| !path.ends_with(Path::new("constants").join("commands.ts")))
        .map(|path| fs::read_to_string(path).expect("read source"))
        .collect();

    let unused: Vec<_> = ts_constants("src/constants/commands.ts")
        .into_keys()
        .filter(|key| !references_constant(&code, key))
        .collect();
    assert!(
        unused.is_empty(),
        "TS command constants never called: {unused:?}"
    );
}

/// `TAURI_COMMAND.KEY` followed by a non-identifier character, so
/// `GET_IMAGE` is not satisfied by `GET_IMAGE_INFO`.
fn references_constant(code: &str, key: &str) -> bool {
    let needle = format!("TAURI_COMMAND.{key}");
    code.match_indices(&needle).any(|(at, _)| {
        code[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|next| !(next.is_ascii_alphanumeric() || next == '_'))
    })
}

/// Commands are invoked only through the wrappers in `src/commands/index.ts`,
/// so no other file may import `invoke` from `@tauri-apps/api/core`.
#[test]
fn only_the_command_layer_imports_invoke() {
    let mut sources = Vec::new();
    frontend_sources(&repo_root().join("src"), &mut sources);
    let command_layer = Path::new("src").join("commands").join("index.ts");

    let offenders: Vec<_> = sources
        .iter()
        .filter(|path| !path.ends_with(&command_layer))
        .filter(|path| {
            let code = fs::read_to_string(path).expect("read source");
            let collapsed = code.split_whitespace().collect::<Vec<_>>().join(" ");
            let pieces: Vec<&str> = collapsed.split("from \"@tauri-apps/api/core\"").collect();
            // Every piece except the last ends right before one such `from`.
            pieces[..pieces.len() - 1].iter().any(|before| {
                let import = before.rsplit("import").next().unwrap_or(before);
                import.contains("invoke")
            })
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "invoke imported outside src/commands/index.ts: {offenders:?}"
    );
}

#[test]
fn ts_event_constants_match_rust_events() {
    let rust: BTreeMap<String, String> = super::events::ALL
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();

    assert_eq!(ts_constants("src/constants/events.ts"), rust);
}

use context::*;
use fnv::FnvHashMap;
use itertools::Itertools;
use languageserver_types::*;
use std::io::{stderr, stdout, Write};
use std::os::unix::fs::DirBuilderExt;
use std::time::Duration;
use std::{env, fs, path, process, thread};
use types::*;

pub fn temp_dir() -> path::PathBuf {
    let mut path = env::temp_dir();
    path.push("kak-lsp");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&path)
        .unwrap();
    path
}

pub fn lsp_range_to_kakoune(range: Range) -> String {
    // LSP ranges are 0-based, but Kakoune's 1-based.
    // LSP ranges are exclusive, but Kakoune's are inclusive.
    // Also from LSP spec: If you want to specify a range that contains a line including
    // the line ending character(s) then use an end position denoting the start of the next
    // line.
    let mut end_line = range.end.line;
    let mut end_char = range.end.character;
    if end_char > 0 {
        end_line += 1;
    } else {
        end_char = 1_000_000;
    }
    format!(
        "{}.{},{}.{}",
        range.start.line + 1,
        range.start.character + 1,
        end_line,
        end_char,
    )
}

pub fn format_symbol_information(items: Vec<SymbolInformation>, ctx: &Context) -> String {
    items
        .into_iter()
        .map(|symbol| {
            let SymbolInformation {
                location,
                name,
                kind,
                ..
            } = symbol;
            let filename = location.uri.to_file_path().unwrap();
            let filename = filename
                .strip_prefix(&ctx.root_path)
                .ok()
                .and_then(|p| Some(p.to_str().unwrap()))
                .or_else(|| filename.to_str())
                .unwrap();

            let position = location.range.start;
            let description = format!("{:?} {}", kind, name);
            format!(
                "{}:{}:{}:{}",
                filename,
                position.line + 1,
                position.character + 1,
                description
            )
        }).join("\n")
}

/// Try to detect language of the file by extension.
pub fn path_to_language_id(extensions: &FnvHashMap<String, String>, path: &str) -> Option<String> {
    extensions
        .get(path::Path::new(path).extension()?.to_str()?)
        .cloned()
}

/// Convert language extensions configuration into a more lookup-friendly form.
pub fn extension_to_language_id_map(config: &Config) -> FnvHashMap<String, String> {
    let mut extensions = FnvHashMap::default();
    for (language_id, language) in &config.language {
        for extension in &language.extensions {
            extensions.insert(extension.clone(), language_id.clone());
        }
    }
    extensions
}

/// Extract extension from path falling back to the empty string.
///
/// Useful for debug messages.
pub fn ext_as_str(path: &str) -> &str {
    path::Path::new(path)
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
}

/// Escape Kakoune string wrapped into single quote
pub fn editor_escape(s: &str) -> String {
    s.replace("'", "''")
}

/// Convert to Kakoune string by wrapping into quotes and escaping
pub fn editor_quote(s: &str) -> String {
    format!("'{}'", editor_escape(s))
}

// Cleanup and gracefully exit
pub fn goodbye(config: &Config, code: i32) {
    if code == 0 {
        if let Some(ref session) = config.server.session {
            let path = temp_dir();
            let sock_path = path.join(session);
            let pid_path = path.join(format!("{}.pid", session));
            if fs::remove_file(sock_path).is_err() {
                warn!("Failed to remove socket file");
            };
            if pid_path.exists() && fs::remove_file(pid_path).is_err() {
                warn!("Failed to remove pid file");
            };
        }
    }
    stderr().flush().unwrap();
    stdout().flush().unwrap();
    // give stdio a chance to actually flush
    thread::sleep(Duration::from_secs(1));
    process::exit(code);
}

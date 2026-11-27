use std::time::Instant;

use crate::models::{CompileResult, Diagnostic};

pub fn check_rust(code: &str) -> (CompileResult, Option<syn::File>) {
    let started = Instant::now();
    match syn::parse_file(code) {
        Err(err) => {
            let start = err.span().start();
            let line = start.line;
            let col = start.column + 1;
            let rendered = render_error(code, line, col, &err.to_string());
            let diag = Diagnostic {
                level: "error".to_string(),
                message: err.to_string(),
                line: Some(line),
                column: Some(col),
                rendered: Some(rendered.clone()),
            };
            let duration_ms = started.elapsed().as_millis() as u64;
            (
                CompileResult {
                    success: false,
                    diagnostics: vec![diag],
                    raw_output: rendered,
                    duration_ms,
                    truncated: false,
                },
                None,
            )
        }
        Ok(file) => {
            let mut diagnostics = Vec::new();
            let has_main = file.items.iter().any(|item| {
                matches!(item, syn::Item::Fn(f) if f.sig.ident == "main")
            });

            if !has_main {
                let msg = "fonction `main` introuvable (`fn main() { ... }`)".to_string();
                let rendered = format!("error[E0601]: {msg}");
                diagnostics.push(Diagnostic {
                    level: "error".to_string(),
                    message: msg,
                    line: Some(1),
                    column: Some(1),
                    rendered: Some(rendered),
                });
            }

            // Check for duplicate top-level function names.
            let mut seen_fns = std::collections::HashSet::new();
            for item in &file.items {
                if let syn::Item::Fn(f) = item {
                    let name = f.sig.ident.to_string();
                    if !seen_fns.insert(name.clone()) {
                        let start = f.sig.ident.span().start();
                        let msg = format!("le nom `{name}` est défini plusieurs fois");
                        let rendered = render_error(code, start.line, start.column + 1, &msg);
                        diagnostics.push(Diagnostic {
                            level: "error".to_string(),
                            message: msg,
                            line: Some(start.line),
                            column: Some(start.column + 1),
                            rendered: Some(rendered),
                        });
                    }
                }
            }

            let success = diagnostics.iter().all(|d| d.level != "error");
            let raw_output = diagnostics
                .iter()
                .filter_map(|d| d.rendered.clone())
                .collect::<Vec<_>>()
                .join("\n\n");
            let duration_ms = started.elapsed().as_millis() as u64;
            (
                CompileResult {
                    success,
                    diagnostics,
                    raw_output,
                    duration_ms,
                    truncated: false,
                },
                if success { Some(file) } else { None },
            )
        }
    }
}

pub fn render_error(code: &str, line: usize, col: usize, message: &str) -> String {
    let source_line = code.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let line_str = line.to_string();
    let pad = " ".repeat(line_str.len());
    let caret_pad = " ".repeat(col.saturating_sub(1));
    format!(
        "error: {message}\n {pad}--> ligne {line}:{col}\n {pad} |\n{line_str} | {source_line}\n {pad} | {caret_pad}^"
    )
}

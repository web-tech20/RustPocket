use crate::models::FormatResult;

/// Validates Rust syntax with `syn` and formats the source code cleanly while
/// preserving comments, docstrings, and string literals.
pub fn format_rust(code: &str) -> FormatResult {
    if let Err(err) = syn::parse_file(code) {
        let start = err.span().start();
        return FormatResult {
            ok: false,
            code: code.to_string(),
            error: Some(format!(
                "Erreur de syntaxe (ligne {}, colonne {}) : {}",
                start.line,
                start.column + 1,
                err
            )),
        };
    }

    let formatted = format_indentation(code);
    FormatResult {
        ok: true,
        code: formatted,
        error: None,
    }
}

fn format_indentation(code: &str) -> String {
    let mut out = String::with_capacity(code.len() + 16);
    let mut indent: usize = 0;
    let mut in_block_comment = false;
    let mut prev_blank = false;

    for raw_line in code.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if !out.is_empty() && !prev_blank {
                out.push('\n');
                prev_blank = true;
            }
            continue;
        }
        prev_blank = false;

        let leading_closers = count_leading_closers(trimmed, in_block_comment);
        let line_indent = indent.saturating_sub(leading_closers);

        for _ in 0..line_indent {
            out.push_str("    ");
        }
        out.push_str(trimmed);
        out.push('\n');

        let (delta, next_block_comment) = scan_delimiters(trimmed, in_block_comment);
        in_block_comment = next_block_comment;
        let net = (indent as isize) + delta;
        indent = if net < 0 { 0 } else { net as usize };
    }

    while out.ends_with("\n\n") {
        out.pop();
    }
    if out.is_empty() || !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn count_leading_closers(line: &str, in_block_comment: bool) -> usize {
    if in_block_comment {
        return 0;
    }
    let mut count = 0;
    for ch in line.chars() {
        if matches!(ch, '}' | ')' | ']') {
            count += 1;
        } else {
            break;
        }
    }
    count
}

fn scan_delimiters(line: &str, mut in_block_comment: bool) -> (isize, bool) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut delta: isize = 0;
    let mut in_string = false;
    let mut in_char = false;

    while i < len {
        if in_block_comment {
            if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        if in_string {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if in_char {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'\'' {
                in_char = false;
            }
            i += 1;
            continue;
        }

        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            break;
        }
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            in_block_comment = true;
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        // Only treat `'` as char literal if it's not a lifetime `'a`
        if bytes[i] == b'\'' && i + 2 < len && (bytes[i + 1] == b'\\' || bytes[i + 2] == b'\'') {
            in_char = true;
            i += 1;
            continue;
        }

        match bytes[i] {
            b'{' | b'(' | b'[' => delta += 1,
            b'}' | b')' | b']' => delta -= 1,
            _ => {}
        }
        i += 1;
    }

    (delta, in_block_comment)
}

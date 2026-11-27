mod checker;
mod formatter;
mod interpreter;
mod models;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub use checker::check_rust;
pub use formatter::format_rust;
pub use interpreter::run_rust;
pub use models::{CompileResult, Diagnostic, FormatResult, RunResult};

fn to_c_json<T: serde::Serialize>(value: &T) -> *mut c_char {
    let json = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    CString::new(json)
        .unwrap_or_else(|_| CString::new("{}").unwrap())
        .into_raw()
}

unsafe fn read_c_str(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// Allocates `len` bytes so Dart FFI can pass UTF-8 strings without `package:ffi`.
#[no_mangle]
pub extern "C" fn rustpocket_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Frees a buffer previously allocated by `rustpocket_alloc`.
#[no_mangle]
pub unsafe extern "C" fn rustpocket_free_bytes(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
}

/// Checks Rust syntax and structure, returning a JSON-encoded `CompileResult`.
#[no_mangle]
pub unsafe extern "C" fn rustpocket_check(code_ptr: *const c_char) -> *mut c_char {
    let code = unsafe { read_c_str(code_ptr) };
    let (result, _) = check_rust(&code);
    to_c_json(&result)
}

/// Parses and executes Rust code, returning a JSON-encoded `RunResult`.
#[no_mangle]
pub unsafe extern "C" fn rustpocket_run(code_ptr: *const c_char) -> *mut c_char {
    let code = unsafe { read_c_str(code_ptr) };
    let result = run_rust(&code);
    to_c_json(&result)
}

/// Formats Rust code, returning a JSON-encoded `FormatResult`.
#[no_mangle]
pub unsafe extern "C" fn rustpocket_format(code_ptr: *const c_char) -> *mut c_char {
    let code = unsafe { read_c_str(code_ptr) };
    let result = format_rust(&code);
    to_c_json(&result)
}

/// Frees a C string returned by `rustpocket_check`, `rustpocket_run`, or `rustpocket_format`.
#[no_mangle]
pub unsafe extern "C" fn rustpocket_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    drop(unsafe { CString::from_raw(ptr) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_world_runs() {
        let res = run_rust(
            r#"
fn main() {
    println!("Bonjour, RustPocket !");
}
"#,
        );
        assert!(res.success, "stderr: {}", res.stderr);
        assert_eq!(res.stdout, "Bonjour, RustPocket !\n");
    }

    #[test]
    fn recursion_and_structs_work() {
        let res = run_rust(
            r#"
struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn new(x: i64, y: i64) -> Self {
        Point { x, y }
    }

    fn sum(&self) -> i64 {
        self.x + self.y
    }
}

fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}

fn main() {
    let p = Point::new(3, 4);
    println!("sum={} fib={}", p.sum(), fib(7));
}
"#,
        );
        assert!(res.success, "stderr: {}", res.stderr);
        assert_eq!(res.stdout, "sum=7 fib=13\n");
    }

    #[test]
    fn vec_closures_and_match_work() {
        let res = run_rust(
            r#"
fn main() {
    let mut nums = vec![1, 2, 3, 4];
    nums.push(5);
    let doubled = nums.iter().map(|x| x * 2).collect();
    println!("{:?}", doubled);
    match nums.get(2) {
        Some(v) => println!("got {v}"),
        None => println!("none"),
    }
}
"#,
        );
        assert!(res.success, "stderr: {}", res.stderr);
        assert_eq!(res.stdout, "[2, 4, 6, 8, 10]\ngot 3\n");
    }

    #[test]
    fn infinite_loop_times_out_safely() {
        let res = run_rust("fn main() { loop {} }");
        assert!(!res.success);
        assert!(res.timed_out);
    }

    #[test]
    fn syntax_error_reports_line_and_column() {
        let (res, _) = check_rust("fn main() {\n    let x = ;\n}\n");
        assert!(!res.success);
        assert!(!res.diagnostics.is_empty());
        assert_eq!(res.diagnostics[0].line, Some(2));
    }

    #[test]
    fn formatter_indents_cleanly() {
        let res = format_rust("fn main() {\nprintln!(\"hi\");\n}\n");
        assert!(res.ok);
        assert_eq!(res.code, "fn main() {\n    println!(\"hi\");\n}\n");
    }
}

mod checker;
mod formatter;
mod interpreter;
mod models;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub use checker::check_rust;
pub use formatter::format_rust;
pub use interpreter::{run_rust, run_rust_with_stdin};
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

/// Parses and executes Rust code with standard input, returning a JSON-encoded `RunResult`.
#[no_mangle]
pub unsafe extern "C" fn rustpocket_run_with_stdin(
    code_ptr: *const c_char,
    stdin_ptr: *const c_char,
) -> *mut c_char {
    let code = unsafe { read_c_str(code_ptr) };
    let stdin = unsafe { read_c_str(stdin_ptr) };
    let result = run_rust_with_stdin(&code, &stdin);
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

    #[test]
    fn stdin_read_line_works() {
        use interpreter::run_rust_with_stdin;
        let code = r#"
use std::io;
fn main() {
    println!("=== TEST STDIN ===");
    let mut nom = String::new();
    println!("Quel est ton nom ?");
    io::stdin().read_line(&mut nom).expect("Erreur de lecture");
    let nom = nom.trim();
    println!("Bonjour {}, Rust fonctionne !", nom);
    let a = 15;
    let b = 25;
    println!("{} + {} = {}", a, b, a + b);
    println!("=== FIN DU TEST ===");
}
"#;
        let res = run_rust_with_stdin(code, "Alice\n");
        assert!(res.success, "stderr: {}", res.stderr);
        assert!(res.stdout.contains("Bonjour Alice, Rust fonctionne !"), "stdout: {}", res.stdout);
        assert!(res.stdout.contains("15 + 25 = 40"), "stdout: {}", res.stdout);
    }

    #[test]
    fn hashmap_works() {
        let code = r#"
use std::collections::HashMap;
fn main() {
    let mut scores: HashMap<String, i32> = HashMap::new();
    scores.insert(String::from("Alice"), 100);
    scores.insert(String::from("Bob"), 85);
    println!("Alice: {}", scores.get("Alice").unwrap_or(&0));
    println!("Bob: {}", scores.get("Bob").unwrap_or(&0));
    println!("contient Alice: {}", scores.contains_key("Alice"));
    scores.remove("Bob");
    println!("contient Bob après remove: {}", scores.contains_key("Bob"));
}
"#;
        let res = run_rust(code);
        assert!(res.success, "stderr: {}", res.stderr);
        assert!(res.stdout.contains("Alice: 100"), "stdout: {}", res.stdout);
        assert!(res.stdout.contains("contient Alice: true"), "stdout: {}", res.stdout);
        assert!(res.stdout.contains("contient Bob après remove: false"), "stdout: {}", res.stdout);
    }

    #[test]
    fn needs_input_signal_works() {
        let code = r#"
use std::io;
fn main() {
    println!("=== TEST RUST ===");
    let mut nom = String::new();
    println!("Quel est ton nom ?");
    io::stdin().read_line(&mut nom).expect("erreur");
    println!("Bonjour {}, Rust fonctionne correctement !", nom.trim());
}
"#;
        let res1 = run_rust_with_stdin(code, "");
        assert!(res1.needs_input, "devrait signaler needs_input");
        assert!(res1.stdout.contains("Quel est ton nom ?"));

        let res2 = run_rust_with_stdin(code, "noe\n");
        assert!(!res2.needs_input);
        assert!(res2.success, "stderr: {}", res2.stderr);
        assert!(res2.stdout.contains("Bonjour noe, Rust fonctionne correctement !"), "stdout: {}", res2.stdout);
    }

    #[test]
    fn iterator_methods_work() {
        let res = run_rust(
            r#"
fn main() {
    let nums = vec![1, 2, 3, 4, 5];
    let sum = nums.iter().fold(0, |acc, x| acc + x);
    println!("fold sum: {}", sum);
    let evens = nums.iter().filter(|x| x % 2 == 0).collect();
    println!("pairs: {:?}", evens);
    println!("contient 3: {}", nums.iter().any(|x| x == 3));
    println!("tous positifs: {}", nums.iter().all(|x| x > 0));
    println!("premier > 3: {:?}", nums.iter().find(|x| x > 3));
    println!("count: {}", nums.iter().count());
    let s = "un deux trois";
    println!("mots: {:?}", s.split_whitespace());
}
"#,
        );
        assert!(res.success, "stderr: {}", res.stderr);
        assert!(res.stdout.contains("fold sum: 15"), "stdout: {}", res.stdout);
        assert!(res.stdout.contains("contient 3: true"), "stdout: {}", res.stdout);
        assert!(res.stdout.contains("tous positifs: true"), "stdout: {}", res.stdout);
        assert!(res.stdout.contains("count: 5"), "stdout: {}", res.stdout);
    }

    #[test]
    fn async_and_command_work() {
        let res = run_rust(
            r#"
use std::process::Command;

async fn calcul() -> i32 {
    21 * 2
}

fn main() {
    let v = calcul().await;
    println!("async: {}", v);
    let out = Command::new("bluetoothctl").arg("scan").output();
    println!("cmd is_err: {}", out.is_err());
}
"#,
        );
        assert!(res.success, "stderr: {}", res.stderr);
        assert!(res.stdout.contains("async: 42"), "stdout: {}", res.stdout);
        assert!(res.stdout.contains("cmd is_err: true"), "stdout: {}", res.stdout);
    }
}

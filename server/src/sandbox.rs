//! Execution of untrusted Rust code.
//!
//! Two modes:
//! - `Local` spawns `rustc`/the program directly, with rlimits, a wall-clock
//!   timeout, a process group and a capped output. Fine for a personal
//!   machine, never enough for a public server.
//! - `Docker` runs each command in a throw-away container with no network,
//!   limited memory/CPU/PIDs and all capabilities dropped. This is the mode the
//!   cahier des charges asks for on the VPS.

use std::{
    path::PathBuf,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

use crate::config::{Config, SandboxMode};

#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("impossible de préparer le dossier de compilation : {0}")]
    Workspace(String),
    #[error("le service de compilation est indisponible : {0}")]
    Unavailable(String),
    #[error("rustfmt a échoué : {0}")]
    Format(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub rendered: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompileResult {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub raw_output: String,
    pub duration_ms: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    pub success: bool,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub truncated: bool,
    pub compile: CompileResult,
}

#[derive(Debug, Clone, Copy)]
pub enum BuildMode {
    /// Emit a runnable binary.
    Binary,
    /// Type-check only (`--emit=metadata`), used by /check.
    Metadata,
}

pub struct Sandbox {
    config: Arc<Config>,
    /// True when `unshare -n` works on this host. Containers always have their
    /// own network namespace, so this only matters in local mode.
    local_network_isolation: bool,
}

struct ProcessOutput {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    timed_out: bool,
    truncated: bool,
    duration_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct Rlimits {
    address_space: u64,
    cpu_secs: u64,
    file_size: u64,
    open_files: u64,
}

impl Sandbox {
    pub fn new(config: Arc<Config>) -> Self {
        let local_network_isolation = match config.sandbox_mode {
            SandboxMode::Docker => true,
            SandboxMode::Local => detect_unshare(),
        };
        if config.sandbox_mode == SandboxMode::Local {
            tracing::warn!(
                "sandbox local actif : isolation réseau {}. Utilisez RUSTPOCKET_SANDBOX=docker en production.",
                if local_network_isolation {
                    "disponible"
                } else {
                    "INDISPONIBLE (installez Docker)"
                }
            );
        }
        Self {
            config,
            local_network_isolation,
        }
    }

    pub async fn compile(
        &self,
        code: &str,
        mode: BuildMode,
    ) -> Result<CompileResult, SandboxError> {
        let workspace = Workspace::new()?;
        self.write_source(&workspace, code)?;

        let started = Instant::now();
        let output = self
            .compile_in(&workspace, mode)
            .await
            .map_err(|e| SandboxError::Unavailable(e.to_string()))?;
        let diagnostics = parse_diagnostics(&output.stderr);
        let raw_output = render_raw(&output.stderr, &diagnostics);

        Ok(CompileResult {
            success: output.exit_code == Some(0),
            diagnostics,
            raw_output,
            duration_ms: started.elapsed().as_millis() as u64,
            truncated: output.truncated,
        })
    }

    pub async fn run(&self, code: &str) -> Result<RunResult, SandboxError> {
        let workspace = Workspace::new()?;
        self.write_source(&workspace, code)?;

        let compile = self
            .compile_in(&workspace, BuildMode::Binary)
            .await
            .map_err(|e| SandboxError::Unavailable(e.to_string()))?;
        let diagnostics = parse_diagnostics(&compile.stderr);
        let compile_result = CompileResult {
            success: compile.exit_code == Some(0),
            diagnostics,
            raw_output: render_raw(&compile.stderr, &parse_diagnostics(&compile.stderr)),
            duration_ms: compile.duration_ms,
            truncated: compile.truncated,
        };

        if !compile_result.success {
            return Ok(RunResult {
                success: false,
                timed_out: false,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: compile_result.duration_ms,
                truncated: compile_result.truncated,
                compile: compile_result,
            });
        }

        let started = Instant::now();
        let output = self
            .execute_binary(&workspace)
            .await
            .map_err(|e| SandboxError::Unavailable(format!("exécution du programme : {e}")))?;
        let duration_ms = started.elapsed().as_millis() as u64;

        Ok(RunResult {
            success: !output.timed_out && output.exit_code == Some(0),
            timed_out: output.timed_out,
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
            duration_ms,
            truncated: output.truncated,
            compile: compile_result,
        })
    }

    pub async fn format(&self, code: &str) -> Result<String, SandboxError> {
        let mut cmd = match self.config.sandbox_mode {
            SandboxMode::Local => {
                let mut c = Command::new("rustfmt");
                c.args(["--edition", "2021", "--emit", "stdout"]);
                c
            }
            SandboxMode::Docker => {
                let mut c = self.docker_command(256, true);
                c.arg(&self.config.sandbox_image);
                c.args(["rustfmt", "--edition", "2021", "--emit", "stdout"]);
                c
            }
        };
        cmd.env("RUSTFMT_TERM", "plain");

        let output = run_process(
            cmd,
            self.config.format_timeout,
            self.config.max_output_bytes,
            Some(code),
            None,
        )
        .await
        .map_err(|e| SandboxError::Unavailable(e.to_string()))?;

        if output.exit_code != Some(0) {
            let details = if output.stderr.trim().is_empty() {
                output.stdout
            } else {
                output.stderr
            };
            return Err(SandboxError::Format(details.trim().to_string()));
        }
        Ok(output.stdout)
    }

    fn write_source(&self, workspace: &Workspace, code: &str) -> Result<(), SandboxError> {
        std::fs::write(workspace.path.join("main.rs"), code)
            .map_err(|e| SandboxError::Workspace(e.to_string()))?;
        Ok(())
    }

    /// Compiles `main.rs` in `workspace` and times the whole command.
    async fn compile_in(
        &self,
        workspace: &Workspace,
        mode: BuildMode,
    ) -> std::io::Result<ProcessOutput> {
        let mut cmd = match self.config.sandbox_mode {
            SandboxMode::Local => {
                let mut c = Command::new("rustc");
                c.current_dir(&workspace.path);
                c
            }
            SandboxMode::Docker => {
                // `nobody` must be able to write the output inside the mount.
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(
                        &workspace.path,
                        std::fs::Permissions::from_mode(0o777),
                    );
                }
                let mut c = self.docker_command(512, false);
                c.args(["--user", "65534:65534", "-w", "/work"]);
                c.arg(&self.config.sandbox_image);
                c.arg("rustc");
                c
            }
        };

        cmd.args([
            "--edition",
            "2021",
            "--error-format=json",
            "--color",
            "never",
        ])
        .args(["--crate-name", "rustpocket_program"]);

        match mode {
            BuildMode::Binary => {
                cmd.args(["-o", self.output_binary_name()]);
            }
            BuildMode::Metadata => {
                cmd.args(["--emit=metadata", "-o", "main.rmeta"]);
            }
        }
        cmd.arg("main.rs");

        let limits = Some(Rlimits {
            address_space: 2 * 1024 * 1024 * 1024,
            cpu_secs: self.config.compile_timeout.as_secs() + 5,
            file_size: 64 * 1024 * 1024,
            open_files: 256,
        });
        run_process(
            cmd,
            self.config.compile_timeout,
            self.config.max_output_bytes,
            None,
            limits,
        )
        .await
    }

    async fn execute_binary(&self, workspace: &Workspace) -> std::io::Result<ProcessOutput> {
        let cmd = match self.config.sandbox_mode {
            SandboxMode::Local => {
                let mut c = Command::new(workspace.path.join(self.output_binary_name()));
                c.current_dir(&workspace.path);
                c.env_clear()
                    .env("PATH", "/usr/local/bin:/usr/bin:/bin")
                    .env("HOME", &workspace.path)
                    .env("TMPDIR", &workspace.path)
                    .env("LANG", "C.UTF-8");
                if self.local_network_isolation {
                    // Prefix the program with `unshare -n`, which cannot be
                    // expressed through Command once it is built.
                    let mut isolated = Command::new("unshare");
                    isolated.arg("-n").arg(c.as_std().get_program());
                    isolated.current_dir(&workspace.path);
                    isolated
                        .env_clear()
                        .env("PATH", "/usr/local/bin:/usr/bin:/bin");
                    isolated
                } else {
                    c
                }
            }
            SandboxMode::Docker => {
                let mut c = self.docker_command(256, false);
                c.args(["--user", "65534:65534", "-w", "/work"]);
                c.arg(&self.config.sandbox_image);
                c.arg(format!("./{}", self.output_binary_name()));
                c
            }
        };

        let limits = Some(Rlimits {
            address_space: 256 * 1024 * 1024,
            cpu_secs: self.config.run_timeout.as_secs() + 1,
            file_size: 1024 * 1024,
            open_files: 64,
        });
        run_process(
            cmd,
            self.config.run_timeout,
            self.config.max_output_bytes,
            None,
            limits,
        )
        .await
    }

    /// Docker options common to every execution. The image name and the
    /// container command must be appended by the caller, after these flags.
    fn docker_command(&self, memory_mb: u32, interactive: bool) -> Command {
        let mut c = Command::new("docker");
        c.args([
            "run",
            "--rm",
            "--network",
            "none",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--pids-limit",
            "128",
            "--memory",
            &format!("{memory_mb}m"),
            "--memory-swap",
            &format!("{memory_mb}m"),
            "--cpus",
            "1",
            "--tmpfs",
            "/tmp:rw,noexec,nosuid,size=8m",
        ]);
        if interactive {
            c.arg("-i");
        }
        c
    }

    fn output_binary_name(&self) -> &'static str {
        "rustpocket_program"
    }
}

struct Workspace {
    path: PathBuf,
}

impl Workspace {
    fn new() -> Result<Self, SandboxError> {
        let path = std::env::temp_dir().join(format!("rustpocket-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).map_err(|e| SandboxError::Workspace(e.to_string()))?;
        Ok(Self { path })
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Extracts rustc JSON diagnostics into something the mobile client can render
/// and link back to a source line.
pub fn parse_diagnostics(stderr: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in stderr.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(level) = value.get("level").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(level, "error" | "warning" | "note" | "help") {
            continue;
        }
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let primary = value
            .get("spans")
            .and_then(Value::as_array)
            .and_then(|spans| {
                spans
                    .iter()
                    .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
                    .or_else(|| spans.first())
            });

        let line_no = primary
            .and_then(|span| span.get("line_start"))
            .and_then(Value::as_u64)
            .map(|n| n as u32);
        let column = primary
            .and_then(|span| span.get("column_start"))
            .and_then(Value::as_u64)
            .map(|n| n as u32);

        diagnostics.push(Diagnostic {
            level: level.to_string(),
            message,
            line: line_no,
            column,
            rendered: value
                .get("rendered")
                .and_then(Value::as_str)
                .map(|s| strip_ansi(s).trim_end().to_string()),
        });
    }

    diagnostics.sort_by_key(|d| (d.line.unwrap_or(u32::MAX), level_rank(&d.level)));
    diagnostics
}

fn level_rank(level: &str) -> u8 {
    match level {
        "error" => 0,
        "warning" => 1,
        "help" => 2,
        _ => 3,
    }
}

fn render_raw(stderr: &str, diagnostics: &[Diagnostic]) -> String {
    if !diagnostics.is_empty() {
        return diagnostics
            .iter()
            .filter(|d| matches!(d.level.as_str(), "error" | "warning"))
            .filter_map(|d| d.rendered.as_deref())
            .collect::<Vec<_>>()
            .join("\n\n");
    }
    // No JSON diagnostic: surface whatever the toolchain said (linker errors,
    // internal compiler errors...).
    stderr
        .lines()
        .filter(|line| !line.trim_start().starts_with('{'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// rustc is run with `--color never`, but bodies coming from a cached JSON
/// stream can still carry escapes. Keep the output clean for a mobile screen.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.next() != Some('[') {
                continue;
            }
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn detect_unshare() -> bool {
    std::process::Command::new("unshare")
        .args(["-n", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Runs a command with a wall-clock timeout, capped output and optional
/// rlimits. The whole process group is killed when the timeout expires.
async fn run_process(
    mut cmd: Command,
    timeout: Duration,
    output_limit: usize,
    stdin_data: Option<&str>,
    limits: Option<Rlimits>,
) -> std::io::Result<ProcessOutput> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .kill_on_drop(true);

    #[cfg(unix)]
    {
        cmd.process_group(0);
        if let Some(limits) = limits {
            unsafe {
                cmd.pre_exec(move || apply_rlimits(limits));
            }
        }
    }
    #[cfg(not(unix))]
    let _ = limits;

    let started = Instant::now();
    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_task = tokio::spawn(read_limited(stdout, output_limit));
    let stderr_task = tokio::spawn(read_limited(stderr, output_limit));

    if let Some(data) = stdin_data
        && let Some(mut stdin) = child.stdin.take()
    {
        let owned = data.to_string();
        tokio::spawn(async move {
            let _ = stdin.write_all(owned.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });
    }

    let mut timed_out = false;
    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(result) => Some(result?),
        Err(_) => {
            timed_out = true;
            kill_process_group(&mut child);
            let _ = child.wait().await;
            None
        }
    };

    let (stdout, stdout_truncated) = stdout_task.await.unwrap_or_default();
    let (stderr, stderr_truncated) = stderr_task.await.unwrap_or_default();

    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        exit_code: status.and_then(|s| s.code()),
        timed_out,
        truncated: stdout_truncated || stderr_truncated,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn kill_process_group(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            // Negative PID targets the whole process group created above.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
    let _ = child.start_kill();
}

async fn read_limited<R>(reader: Option<R>, limit: usize) -> (Vec<u8>, bool)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return (Vec::new(), false);
    };
    let mut kept = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 8192];

    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                if kept.len() < limit {
                    let take = n.min(limit - kept.len());
                    kept.extend_from_slice(&buffer[..take]);
                    if take < n {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }

    (kept, truncated)
}

#[cfg(unix)]
unsafe fn apply_rlimits(limits: Rlimits) -> std::io::Result<()> {
    fn set(resource: libc::__rlimit_resource_t, soft: u64, hard: u64) -> std::io::Result<()> {
        let limit = libc::rlimit {
            rlim_cur: soft as libc::rlim_t,
            rlim_max: hard as libc::rlim_t,
        };
        // SAFETY: `limit` is a valid rlimit for the current process.
        if unsafe { libc::setrlimit(resource, &limit) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    set(libc::RLIMIT_AS, limits.address_space, limits.address_space)?;
    set(libc::RLIMIT_CPU, limits.cpu_secs, limits.cpu_secs + 1)?;
    set(libc::RLIMIT_FSIZE, limits.file_size, limits.file_size)?;
    set(libc::RLIMIT_NOFILE, limits.open_files, limits.open_files)?;
    set(libc::RLIMIT_CORE, 0, 0)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rustc_json_diagnostics() {
        let stderr = concat!(
            r#"{"message":"cannot find value `x` in this scope","code":{"code":"E0425","explanation":null},"level":"error","spans":[{"file_name":"main.rs","line_start":2,"line_end":2,"column_start":5,"column_end":6,"is_primary":true}],"rendered":"error[E0425]: cannot find value `x` in this scope\n"}"#,
            "\n",
            r#"{"message":"unused variable: `y`","code":{"code":"unused_variables","explanation":null},"level":"warning","spans":[{"file_name":"main.rs","line_start":4,"line_end":4,"column_start":9,"column_end":10,"is_primary":true}],"rendered":"warning: unused variable: `y`\n"}"#
        );
        let diagnostics = parse_diagnostics(stderr);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].line, Some(2));
        assert_eq!(diagnostics[0].column, Some(5));
        assert_eq!(diagnostics[0].level, "error");
        assert_eq!(diagnostics[1].level, "warning");
    }

    #[test]
    fn output_is_kept_below_the_cap() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (kept, truncated) = runtime.block_on(async {
            let data = vec![b'a'; 50_000];
            read_limited(Some(std::io::Cursor::new(data)), 1024).await
        });
        assert_eq!(kept.len(), 1024);
        assert!(truncated);
    }

    #[tokio::test]
    async fn runs_a_hello_world_program() {
        let config = Arc::new(crate::config::Config::from_env());
        let sandbox = Sandbox::new(config);
        let result = sandbox
            .run("fn main() { println!(\"bonjour\"); }")
            .await
            .expect("sandbox execution");
        assert!(result.success, "{:?}", result);
        assert_eq!(result.stdout.trim(), "bonjour");
    }

    #[tokio::test]
    async fn stops_an_infinite_loop() {
        let mut config = crate::config::Config::from_env();
        config.run_timeout = Duration::from_secs(1);
        let sandbox = Sandbox::new(Arc::new(config));
        let result = sandbox
            .run("fn main() { loop { std::hint::spin_loop(); } }")
            .await
            .expect("sandbox execution");
        assert!(result.timed_out, "{:?}", result);
        assert!(result.duration_ms >= 900);
    }
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
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

#[derive(Debug, Clone, Serialize)]
pub struct FormatResult {
    pub ok: bool,
    pub code: String,
    pub error: Option<String>,
}

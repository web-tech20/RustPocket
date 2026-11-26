/// Wire models shared with the RustPocket backend.
library;

class Diagnostic {
  const Diagnostic({
    required this.level,
    required this.message,
    this.line,
    this.column,
    this.rendered,
  });

  final String level;
  final String message;
  final int? line;
  final int? column;
  final String? rendered;

  bool get isError => level == 'error';

  factory Diagnostic.fromJson(Map<String, dynamic> json) {
    return Diagnostic(
      level: json['level'] as String? ?? 'error',
      message: json['message'] as String? ?? '',
      line: (json['line'] as num?)?.toInt(),
      column: (json['column'] as num?)?.toInt(),
      rendered: json['rendered'] as String?,
    );
  }
}

class CompileResult {
  const CompileResult({
    required this.success,
    required this.diagnostics,
    required this.rawOutput,
    required this.durationMs,
    required this.truncated,
  });

  final bool success;
  final List<Diagnostic> diagnostics;
  final String rawOutput;
  final int durationMs;
  final bool truncated;

  int get errorCount => diagnostics.where((d) => d.isError).length;
  int get warningCount => diagnostics.where((d) => d.level == 'warning').length;

  factory CompileResult.fromJson(Map<String, dynamic> json) {
    final diagnostics = (json['diagnostics'] as List<dynamic>? ?? [])
        .whereType<Map<String, dynamic>>()
        .map(Diagnostic.fromJson)
        .toList();
    return CompileResult(
      success: json['success'] as bool? ?? false,
      diagnostics: diagnostics,
      rawOutput: json['raw_output'] as String? ?? '',
      durationMs: (json['duration_ms'] as num?)?.toInt() ?? 0,
      truncated: json['truncated'] as bool? ?? false,
    );
  }
}

class RunResult {
  const RunResult({
    required this.success,
    required this.timedOut,
    required this.exitCode,
    required this.stdout,
    required this.stderr,
    required this.durationMs,
    required this.truncated,
    required this.compile,
  });

  final bool success;
  final bool timedOut;
  final int? exitCode;
  final String stdout;
  final String stderr;
  final int durationMs;
  final bool truncated;
  final CompileResult compile;

  factory RunResult.fromJson(Map<String, dynamic> json) {
    return RunResult(
      success: json['success'] as bool? ?? false,
      timedOut: json['timed_out'] as bool? ?? false,
      exitCode: (json['exit_code'] as num?)?.toInt(),
      stdout: json['stdout'] as String? ?? '',
      stderr: json['stderr'] as String? ?? '',
      durationMs: (json['duration_ms'] as num?)?.toInt() ?? 0,
      truncated: json['truncated'] as bool? ?? false,
      compile: CompileResult.fromJson(
        (json['compile'] as Map<String, dynamic>?) ?? const {},
      ),
    );
  }
}

class AuthSession {
  const AuthSession({required this.token, required this.email});

  final String token;
  final String email;

  factory AuthSession.fromJson(Map<String, dynamic> json) {
    final user = (json['user'] as Map<String, dynamic>?) ?? const {};
    return AuthSession(
      token: json['token'] as String? ?? '',
      email: user['email'] as String? ?? '',
    );
  }
}

class QuotaStatus {
  const QuotaStatus({
    required this.limitPerMinute,
    required this.remainingMinute,
    required this.limitPerDay,
    required this.remainingDay,
  });

  final int limitPerMinute;
  final int remainingMinute;
  final int limitPerDay;
  final int remainingDay;

  factory QuotaStatus.fromJson(Map<String, dynamic> json) {
    return QuotaStatus(
      limitPerMinute: (json['limit_per_minute'] as num?)?.toInt() ?? 0,
      remainingMinute: (json['remaining_minute'] as num?)?.toInt() ?? 0,
      limitPerDay: (json['limit_per_day'] as num?)?.toInt() ?? 0,
      remainingDay: (json['remaining_day'] as num?)?.toInt() ?? 0,
    );
  }
}

class AccountInfo {
  const AccountInfo({
    required this.email,
    required this.quotas,
    required this.maxCodeBytes,
    required this.runTimeoutSecs,
  });

  final String email;
  final QuotaStatus quotas;
  final int maxCodeBytes;
  final int runTimeoutSecs;

  factory AccountInfo.fromJson(Map<String, dynamic> json) {
    final limits = (json['limits'] as Map<String, dynamic>?) ?? const {};
    return AccountInfo(
      email: json['email'] as String? ?? '',
      quotas: QuotaStatus.fromJson(
        (json['quotas'] as Map<String, dynamic>?) ?? const {},
      ),
      maxCodeBytes: (limits['max_code_bytes'] as num?)?.toInt() ?? 65536,
      runTimeoutSecs: (limits['run_timeout_secs'] as num?)?.toInt() ?? 10,
    );
  }
}

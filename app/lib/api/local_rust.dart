import 'dart:async';
import 'dart:convert';
import 'dart:io';

import '../api/models.dart';

/// Runs the Rust toolchain directly on the host machine.
///
/// This is the offline counterpart of the RustPocket server sandbox: it
/// shells out to `rustc` / `rustfmt` and shapes their output into the same
/// models the UI already consumes. It only works where a Rust toolchain is
/// installed (development machines and tests).
class LocalRust {
  LocalRust({this.timeout = const Duration(seconds: 10)});

  final Duration timeout;

  Future<CompileResult> check(String code) => _compile(code, metadataOnly: true);
  Future<CompileResult> compile(String code) => _compile(code, metadataOnly: false);

  Future<CompileResult> _compile(String code, {required bool metadataOnly}) async {
    final dir = await Directory.systemTemp.createTemp('rustpocket_');
    try {
      final source = File('${dir.path}/main.rs');
      await source.writeAsString(code);
      final binary = '${dir.path}/rustpocket_program';
      final args = <String>[
        '--edition',
        '2021',
        '--error-format=json',
        '--color',
        'never',
        '--crate-name',
        'rustpocket_program',
      ];
      if (metadataOnly) {
        args.addAll(['--emit=metadata', '-o', '${dir.path}/main.rmeta']);
      } else {
        args.addAll(['-o', binary]);
      }
      args.add('main.rs');

      final started = DateTime.now();
      final result = await _run(
        'rustc',
        args,
        workingDirectory: dir.path,
        timeout: timeout,
      );
      final duration = DateTime.now().difference(started).inMilliseconds;
      final diagnostics = _parseDiagnostics(result.stderr);
      final raw = diagnostics
          .where((d) => d.level == 'error' || d.level == 'warning')
          .map((d) => d.rendered ?? '')
          .where((s) => s.trim().isNotEmpty)
          .join('\n\n');
      return CompileResult(
        success: result.exitCode == 0,
        diagnostics: diagnostics,
        rawOutput: raw.isNotEmpty ? raw : result.stderr.trim(),
        durationMs: duration < 0 ? 0 : duration,
        truncated: false,
      );
    } finally {
      try {
        await dir.delete(recursive: true);
      } catch (_) {}
    }
  }

  Future<RunResult> run(String code) async {
    final compileResult = await compile(code);
    if (!compileResult.success) {
      return RunResult(
        success: false,
        timedOut: false,
        exitCode: null,
        stdout: '',
        stderr: '',
        durationMs: compileResult.durationMs,
        truncated: false,
        compile: compileResult,
      );
    }
    final dir = await Directory.systemTemp.createTemp('rustpocket_');
    try {
      final source = File('${dir.path}/main.rs');
      await source.writeAsString(code);
      final binary = '${dir.path}/rustpocket_program';
      final c = await _run(
        'rustc',
        [
          '--edition',
          '2021',
          '--error-format=json',
          '--color',
          'never',
          '--crate-name',
          'rustpocket_program',
          '-o',
          binary,
          'main.rs',
        ],
        workingDirectory: dir.path,
        timeout: timeout,
      );
      if (c.exitCode != 0) {
        return RunResult(
          success: false,
          timedOut: false,
          exitCode: null,
          stdout: '',
          stderr: '',
          durationMs: compileResult.durationMs,
          truncated: false,
          compile: compileResult,
        );
      }

      final started = DateTime.now();
      final exec = await _runBinary(binary, dir.path);
      final duration = DateTime.now().difference(started).inMilliseconds;
      return RunResult(
        success: !exec.timedOut && exec.exitCode == 0,
        timedOut: exec.timedOut,
        exitCode: exec.exitCode,
        stdout: exec.stdout,
        stderr: exec.stderr,
        durationMs: duration < 0 ? 0 : duration,
        truncated: exec.truncated,
        compile: compileResult,
      );
    } finally {
      try {
        await dir.delete(recursive: true);
      } catch (_) {}
    }
  }

  Future<String> format(String code) async {
    final result = await _runWithStdin(
      'rustfmt',
      ['--edition', '2021', '--emit', 'stdout'],
      null,
      code,
      const Duration(seconds: 30),
    );
    if (result.exitCode != 0) {
      final details = result.stderr.trim().isNotEmpty
          ? result.stderr.trim()
          : result.stdout.trim();
      throw Exception(details);
    }
    return result.stdout;
  }

  List<Diagnostic> _parseDiagnostics(String stderr) {
    final diagnostics = <Diagnostic>[];
    for (final line in stderr.split('\n')) {
      final trimmed = line.trim();
      if (trimmed.isEmpty || !trimmed.startsWith('{')) continue;
      Map<String, dynamic>? value;
      try {
        value = jsonDecode(trimmed) as Map<String, dynamic>?;
      } catch (_) {
        continue;
      }
      if (value == null) continue;
      final level = value['level'] as String?;
      if (level == null ||
          !['error', 'warning', 'note', 'help'].contains(level)) {
        continue;
      }
      final message = value['message'] as String? ?? '';
      final spans = value['spans'] as List<dynamic>?;
      Map<String, dynamic>? primary;
      if (spans != null && spans.isNotEmpty) {
        primary = spans.firstWhere(
          (s) => s is Map && s['is_primary'] == true,
          orElse: () => spans.first,
        ) as Map<String, dynamic>?;
      }
      final lineNo = (primary?['line_start'] as num?)?.toInt();
      final column = (primary?['column_start'] as num?)?.toInt();
      final rendered = value['rendered'] as String?;
      diagnostics.add(
        Diagnostic(
          level: level,
          message: message,
          line: lineNo,
          column: column,
          rendered: rendered?.trimRight(),
        ),
      );
    }
    diagnostics.sort((a, b) {
      final la = a.line ?? 1 << 30;
      final lb = b.line ?? 1 << 30;
      if (la != lb) return la.compareTo(lb);
      return 0;
    });
    return diagnostics;
  }

  Future<_ProcessResult> _run(
    String executable,
    List<String> args, {
    String? workingDirectory,
    Duration? timeout,
  }) async {
    final t = timeout ?? this.timeout;
    try {
      final result = await Process.run(
        executable,
        args,
        workingDirectory: workingDirectory,
      ).timeout(t);
      return _ProcessResult(
        exitCode: result.exitCode,
        stdout: result.stdout is String
            ? result.stdout as String
            : utf8.decode(result.stdout as List<int>),
        stderr: result.stderr is String
            ? result.stderr as String
            : utf8.decode(result.stderr as List<int>),
        timedOut: false,
        truncated: false,
      );
    } on TimeoutException {
      return _ProcessResult(
        exitCode: null,
        stdout: '',
        stderr: '',
        timedOut: true,
        truncated: false,
      );
    } catch (_) {
      return _ProcessResult(
        exitCode: -1,
        stdout: '',
        stderr: 'outils Rust introuvables (rustc/rustfmt).',
        timedOut: false,
        truncated: false,
      );
    }
  }

  Future<_ProcessResult> _runWithStdin(
    String executable,
    List<String> args,
    String? workingDirectory,
    String stdin,
    Duration t,
  ) async {
    try {
      final proc = await Process.start(
        executable,
        args,
        workingDirectory: workingDirectory,
      );
      proc.stdin.write(stdin);
      await proc.stdin.close();
      final stdoutFuture = proc.stdout.transform(utf8.decoder).join();
      final stderrFuture = proc.stderr.transform(utf8.decoder).join();
      int? exitCode;
      bool timedOut = false;
      try {
        exitCode = await proc.exitCode.timeout(t);
      } on TimeoutException {
        timedOut = true;
        proc.kill(ProcessSignal.sigkill);
        try {
          await proc.exitCode.timeout(const Duration(seconds: 2));
        } catch (_) {}
      }
      final stdout = await stdoutFuture.catchError((_) => '');
      final stderr = await stderrFuture.catchError((_) => '');
      return _ProcessResult(
        exitCode: timedOut ? null : exitCode,
        stdout: stdout,
        stderr: stderr,
        timedOut: timedOut,
        truncated: false,
      );
    } catch (_) {
      return _ProcessResult(
        exitCode: -1,
        stdout: '',
        stderr: 'outils Rust introuvables (rustfmt).',
        timedOut: false,
        truncated: false,
      );
    }
  }

  Future<_ProcessResult> _runBinary(String binary, String workdir) async {
    final t = timeout;
    try {
      final proc = await Process.start(
        binary,
        [],
        workingDirectory: workdir,
        environment: const {
          'PATH': '/usr/local/bin:/usr/bin:/bin',
          'HOME': '/tmp',
          'TMPDIR': '/tmp',
          'LANG': 'C.UTF-8',
        },
      );
      final stdoutFuture = proc.stdout.transform(utf8.decoder).join();
      final stderrFuture = proc.stderr.transform(utf8.decoder).join();
      int? exitCode;
      bool timedOut = false;
      try {
        exitCode = await proc.exitCode.timeout(t);
      } on TimeoutException {
        timedOut = true;
        proc.kill(ProcessSignal.sigkill);
        try {
          await proc.exitCode.timeout(const Duration(seconds: 2));
        } catch (_) {}
      }
      final stdout = await stdoutFuture.catchError((_) => '');
      final stderr = await stderrFuture.catchError((_) => '');
      return _ProcessResult(
        exitCode: exitCode,
        stdout: stdout,
        stderr: stderr,
        timedOut: timedOut,
        truncated: false,
      );
    } catch (_) {
      return _ProcessResult(
        exitCode: -1,
        stdout: '',
        stderr: 'exécution impossible.',
        timedOut: false,
        truncated: false,
      );
    }
  }
}

class _ProcessResult {
  _ProcessResult({
    required this.exitCode,
    required this.stdout,
    required this.stderr,
    required this.timedOut,
    required this.truncated,
  });

  final int? exitCode;
  final String stdout;
  final String stderr;
  final bool timedOut;
  final bool truncated;
}

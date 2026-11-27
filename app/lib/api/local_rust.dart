import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../api/models.dart';
import 'native_rust_engine.dart';

/// Runs Rust code locally on the device.
///
/// - On Android and iOS, uses the embedded native engine (`librustpocket_engine`)
///   via FFI, working 100% offline.
/// - On Desktop (Linux, macOS, Windows), uses `rustc` / `rustfmt` if installed
///   on the host, or falls back to the embedded native engine.
class LocalRust {
  LocalRust({this.timeout = const Duration(seconds: 10)});

  final Duration timeout;
  final Map<String, String> _resolvedBinaries = {};
  final NativeRustEngine _native = NativeRustEngine();

  bool get _preferNativeEngine =>
      !kIsWeb && (Platform.isAndroid || Platform.isIOS);

  static const _webUnsupportedMessage =
      'Exécution locale indisponible dans un navigateur Web. '
      'Lancez l’application en natif (Linux, macOS, Windows ou mobile).';

  /// Resolves a toolchain binary (`rustc` or `rustfmt`) on PATH or common
  /// install locations such as `~/.cargo/bin`.
  Future<String?> _resolveBinary(String name) async {
    if (kIsWeb) return null;
    final cached = _resolvedBinaries[name];
    if (cached != null) return cached;

    final candidates = <String>[name];
    final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
    final ext = Platform.isWindows ? '.exe' : '';
    if (home != null && home.isNotEmpty) {
      candidates.add('$home${Platform.pathSeparator}.cargo${Platform.pathSeparator}bin${Platform.pathSeparator}$name$ext');
    }
    if (!Platform.isWindows) {
      candidates.add('/usr/local/bin/$name');
      candidates.add('/usr/bin/$name');
    }

    for (final candidate in candidates) {
      try {
        final result = await Process.run(
          candidate,
          ['--version'],
        ).timeout(const Duration(seconds: 3));
        if (result.exitCode == 0) {
          _resolvedBinaries[name] = candidate;
          return candidate;
        }
      } catch (_) {
        // Try next candidate.
      }
    }
    return null;
  }

  Future<CompileResult> check(String code) => _compile(code, metadataOnly: true);
  Future<CompileResult> compile(String code) => _compile(code, metadataOnly: false);

  /// Core compilation logic. Returns a failure result if rustc is not available.
  Future<CompileResult> _compile(String code, {required bool metadataOnly}) async {
    if (kIsWeb) {
      return const CompileResult(
        success: false,
        diagnostics: [],
        rawOutput: _webUnsupportedMessage,
        durationMs: 0,
        truncated: false,
      );
    }

    if (_preferNativeEngine) {
      return _native.check(code);
    }

    final rustc = await _resolveBinary('rustc');
    if (rustc == null) {
      if (_native.isAvailable) {
        return _native.check(code);
      }
      return const CompileResult(
        success: false,
        diagnostics: [],
        rawOutput: 'Outils Rust (rustc) introuvables sur cet appareil.',
        durationMs: 0,
        truncated: false,
      );
    }

    final dir = await Directory.systemTemp.createTemp('rustpocket_');
    try {
      final source = File('${dir.path}${Platform.pathSeparator}main.rs');
      await source.writeAsString(code);
      final binary = '${dir.path}${Platform.pathSeparator}rustpocket_program${Platform.isWindows ? '.exe' : ''}';
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
        args.addAll(['--emit=metadata', '-o', '${dir.path}${Platform.pathSeparator}main.rmeta']);
      } else {
        args.addAll(['-o', binary]);
      }
      args.add('main.rs');

      final started = DateTime.now();
      final result = await _run(
        rustc,
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

  /// Runs the Rust compiler to produce an executable, then executes it.
  Future<RunResult> run(String code, {String stdin = ''}) async {
    if (kIsWeb) {
      return const RunResult(
        success: false,
        timedOut: false,
        exitCode: null,
        stdout: '',
        stderr: '',
        durationMs: 0,
        truncated: false,
        compile: CompileResult(
          success: false,
          diagnostics: [],
          rawOutput: _webUnsupportedMessage,
          durationMs: 0,
          truncated: false,
        ),
      );
    }

    if (_preferNativeEngine) {
      return _native.run(code, stdin: stdin);
    }

    final rustc = await _resolveBinary('rustc');
    if (rustc == null) {
      if (_native.isAvailable) {
        return _native.run(code, stdin: stdin);
      }
      return const RunResult(
        success: false,
        timedOut: false,
        exitCode: null,
        stdout: '',
        stderr: '',
        durationMs: 0,
        truncated: false,
        compile: CompileResult(
          success: false,
          diagnostics: [],
          rawOutput: 'Outils Rust (rustc) introuvables sur cet appareil.',
          durationMs: 0,
          truncated: false,
        ),
      );
    }

    final dir = await Directory.systemTemp.createTemp('rustpocket_');
    try {
      final source = File('${dir.path}${Platform.pathSeparator}main.rs');
      await source.writeAsString(code);
      final binary = '${dir.path}${Platform.pathSeparator}rustpocket_program${Platform.isWindows ? '.exe' : ''}';
      final compileStarted = DateTime.now();
      final c = await _run(
        rustc,
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
      final compileDuration =
          DateTime.now().difference(compileStarted).inMilliseconds;
      final diagnostics = _parseDiagnostics(c.stderr);
      final raw = diagnostics
          .where((d) => d.level == 'error' || d.level == 'warning')
          .map((d) => d.rendered ?? '')
          .where((s) => s.trim().isNotEmpty)
          .join('\n\n');
      final compileResult = CompileResult(
        success: c.exitCode == 0,
        diagnostics: diagnostics,
        rawOutput: raw.isNotEmpty ? raw : c.stderr.trim(),
        durationMs: compileDuration < 0 ? 0 : compileDuration,
        truncated: false,
      );
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

      final started = DateTime.now();
      final exec = await _runBinary(binary, dir.path, stdin: stdin);
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

  /// Attempts to format code using rustfmt. If rustfmt is unavailable,
  /// throws an informative exception.
  Future<String> format(String code) async {
    if (kIsWeb) {
      throw Exception(_webUnsupportedMessage);
    }
    if (_preferNativeEngine) {
      return _native.format(code);
    }
    final rustfmt = await _resolveBinary('rustfmt');
    if (rustfmt == null) {
      if (_native.isAvailable) {
        return _native.format(code);
      }
      throw Exception('Outils Rust introuvables (rustfmt).');
    }
    final result = await _runWithStdin(
      rustfmt,
      ['--edition', '2021', '--emit', 'stdout'],
      null,
      code,
      const Duration(seconds: 30),
    );
    if (result.exitCode != 0) {
      final details = result.stderr.trim().isNotEmpty
          ? result.stderr.trim()
          : result.stdout.trim();
      throw Exception('Formatage échoué: $details');
    }
    return result.stdout;
  }

  /// Safe formatting that doesn't throw: returns (formattedCode, errorMessageOrNull)
  /// Callers can choose to show the error or keep original code.
  Future<(String formattedCode, String? errorMessage)> formatSafe(String code) async {
    try {
      final formatted = await format(code);
      return (formatted, null);
    } catch (e) {
      return (code, e.toString().replaceFirst('Exception: ', ''));
    }
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

  Future<_ProcessResult> _runBinary(
    String binary,
    String workdir, {
    String stdin = '',
  }) async {
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
      if (stdin.isNotEmpty) {
        proc.stdin.write(stdin);
      }
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
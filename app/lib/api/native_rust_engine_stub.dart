import 'models.dart';

class NativeRustEngine {
  bool get isAvailable => false;

  Future<CompileResult> check(String code) async {
    return const CompileResult(
      success: false,
      diagnostics: [],
      rawOutput: 'Moteur natif indisponible sur cette plateforme.',
      durationMs: 0,
      truncated: false,
    );
  }

  Future<RunResult> run(String code) async {
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
        rawOutput: 'Moteur natif indisponible sur cette plateforme.',
        durationMs: 0,
        truncated: false,
      ),
    );
  }

  Future<String> format(String code) async {
    throw Exception('Moteur natif indisponible sur cette plateforme.');
  }
}

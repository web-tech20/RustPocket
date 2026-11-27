import 'local_rust.dart';
import 'models.dart';

class ApiException implements Exception {
  ApiException(this.message, {this.statusCode});

  final String message;
  final int? statusCode;

  @override
  String toString() => message;
}

/// Local Rust execution client.
///
/// Runs `rustc` and `rustfmt` directly on the device without any remote server.
class ApiClient {
  ApiClient({LocalRust? local}) : _local = local ?? LocalRust();

  final LocalRust _local;

  Future<CompileResult> compile(String code) => _local.compile(code);

  Future<CompileResult> check(String code) => _local.check(code);

  Future<RunResult> run(String code, {String stdin = ''}) =>
      _local.run(code, stdin: stdin);

  Future<String> format(String code) async {
    try {
      return await _local.format(code);
    } catch (e) {
      throw ApiException(e.toString().replaceFirst('Exception: ', ''));
    }
  }
}

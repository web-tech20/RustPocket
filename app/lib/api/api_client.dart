import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;

import 'models.dart';

class ApiException implements Exception {
  ApiException(this.message, {this.statusCode});

  final String message;
  final int? statusCode;

  bool get isUnauthorized => statusCode == 401;

  @override
  String toString() => message;
}

/// Thin client for the RustPocket API.
///
/// The base URL and the token are read through callbacks so the settings
/// screen can change the server without rebuilding the client.
class ApiClient {
  ApiClient({
    required this.baseUrlProvider,
    required this.tokenProvider,
    http.Client? client,
  }) : _client = client ?? http.Client();

  final String Function() baseUrlProvider;
  final String? Function() tokenProvider;
  final http.Client _client;

  Uri _uri(String path) {
    var base = baseUrlProvider().trim();
    while (base.endsWith('/')) {
      base = base.substring(0, base.length - 1);
    }
    return Uri.parse('$base$path');
  }

  Map<String, String> get _headers {
    final token = tokenProvider();
    return {
      'content-type': 'application/json',
      if (token != null && token.isNotEmpty) 'authorization': 'Bearer $token',
    };
  }

  Future<AuthSession> login(String email, String password) async {
    final json = await _post('/auth/login', {
      'email': email,
      'password': password,
    });
    return AuthSession.fromJson(json as Map<String, dynamic>);
  }

  Future<AuthSession> register(String email, String password) async {
    final json = await _post('/auth/register', {
      'email': email,
      'password': password,
    });
    return AuthSession.fromJson(json as Map<String, dynamic>);
  }

  Future<AccountInfo> me() async {
    final json = await _get('/me');
    return AccountInfo.fromJson(json as Map<String, dynamic>);
  }

  Future<CompileResult> compile(String code) async {
    final json = await _post('/compile', {'code': code});
    return CompileResult.fromJson(json as Map<String, dynamic>);
  }

  Future<CompileResult> check(String code) async {
    final json = await _post('/check', {'code': code});
    return CompileResult.fromJson(json as Map<String, dynamic>);
  }

  Future<RunResult> run(String code) async {
    final json = await _post('/run', {'code': code});
    return RunResult.fromJson(json as Map<String, dynamic>);
  }

  Future<String> format(String code) async {
    final json = await _post(
      '/format',
      {'code': code},
      timeout: const Duration(seconds: 30),
    );
    return (json as Map<String, dynamic>)['code'] as String? ?? code;
  }

  Future<dynamic> _get(String path) async {
    try {
      final response = await _client
          .get(_uri(path), headers: _headers)
          .timeout(const Duration(seconds: 20));
      return _decode(response);
    } on TimeoutException {
      throw ApiException('Le serveur met trop de temps à répondre.');
    } on ApiException {
      rethrow;
    } catch (_) {
      throw _unreachable();
    }
  }

  Future<dynamic> _post(
    String path,
    Map<String, dynamic> body, {
    Duration timeout = const Duration(seconds: 60),
  }) async {
    try {
      final response = await _client
          .post(_uri(path), headers: _headers, body: jsonEncode(body))
          .timeout(timeout);
      return _decode(response);
    } on TimeoutException {
      throw ApiException('Le serveur met trop de temps à répondre.');
    } on ApiException {
      rethrow;
    } catch (_) {
      throw _unreachable();
    }
  }

  ApiException _unreachable() => ApiException(
    'Serveur injoignable. Vérifiez l’adresse du serveur et votre connexion.',
  );

  dynamic _decode(http.Response response) {
    dynamic json;
    if (response.body.trim().isNotEmpty) {
      try {
        json = jsonDecode(utf8.decode(response.bodyBytes));
      } catch (_) {
        json = null;
      }
    }

    if (response.statusCode >= 400) {
      String? message;
      if (json is Map<String, dynamic>) {
        final error = json['error'];
        if (error is Map<String, dynamic>) {
          message = error['message'] as String?;
        }
      }
      throw ApiException(
        message ?? 'Erreur du serveur (${response.statusCode}).',
        statusCode: response.statusCode,
      );
    }

    return json;
  }
}

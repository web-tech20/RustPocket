import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:rustpocket/api/api_client.dart';

ApiClient _client({
  required String? token,
  required void Function() onUnauthorized,
}) {
  return ApiClient(
    baseUrlProvider: () => 'http://localhost:8080',
    tokenProvider: () => token,
    onUnauthorized: onUnauthorized,
    client: MockClient(
      (_) async => http.Response(
        jsonEncode({
          'error': {'message': 'session expirée'},
        }),
        401,
        headers: {'content-type': 'application/json'},
      ),
    ),
  );
}

void main() {
  test('un 401 avec un jeton déclenche la déconnexion', () async {
    var signOuts = 0;
    final api = _client(token: 'jeton', onUnauthorized: () => signOuts++);

    await expectLater(
      api.me(),
      throwsA(
        isA<ApiException>()
            .having((e) => e.message, 'message', 'session expirée')
            .having((e) => e.isUnauthorized, 'isUnauthorized', isTrue),
      ),
    );
    expect(signOuts, 1);
  });

  test('un 401 sans jeton ne déclenche pas de déconnexion', () async {
    var signOuts = 0;
    final api = _client(token: null, onUnauthorized: () => signOuts++);

    await expectLater(api.me(), throwsA(isA<ApiException>()));
    expect(signOuts, 0);
  });
}

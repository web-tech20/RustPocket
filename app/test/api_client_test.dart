import 'package:flutter_test/flutter_test.dart';
import 'package:rustpocket/storage/auth_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  test('AuthStore crée un compte local et connecte l’utilisateur', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final auth = AuthStore.load(prefs);

    expect(auth.isAuthenticated, isFalse);
    expect(auth.hasLocalAccounts, isFalse);

    final registered = await auth.registerLocal(
      email: 'Test@Example.com',
      password: 'password123',
    );
    expect(registered, isTrue);
    expect(auth.isAuthenticated, isTrue);
    expect(auth.email, 'test@example.com');
    expect(auth.hasLocalAccounts, isTrue);

    final duplicate = await auth.registerLocal(
      email: 'test@example.com',
      password: 'password123',
    );
    expect(duplicate, isFalse);

    await auth.signOut();
    expect(auth.isAuthenticated, isFalse);

    final badLogin = await auth.loginLocal(
      email: 'test@example.com',
      password: 'wrong',
    );
    expect(badLogin, isFalse);

    final goodLogin = await auth.loginLocal(
      email: 'test@example.com',
      password: 'password123',
    );
    expect(goodLogin, isTrue);
    expect(auth.isAuthenticated, isTrue);
  });
}

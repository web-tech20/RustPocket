import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

class AuthStore extends ChangeNotifier {
  AuthStore._(this._prefs, this._token, this._email);

  static const _tokenKey = 'rustpocket.token';
  static const _emailKey = 'rustpocket.email';

  final SharedPreferences _prefs;
  String? _token;
  String? _email;

  static AuthStore load(SharedPreferences prefs) {
    return AuthStore._(
      prefs,
      prefs.getString(_tokenKey),
      prefs.getString(_emailKey),
    );
  }

  String? get token => _token;
  String? get email => _email;
  bool get isAuthenticated => _token != null && _token!.isNotEmpty;

  Future<void> signIn({required String token, required String email}) async {
    _token = token;
    _email = email;
    await _prefs.setString(_tokenKey, token);
    await _prefs.setString(_emailKey, email);
    notifyListeners();
  }

  Future<void> signOut() async {
    _token = null;
    _email = null;
    await _prefs.remove(_tokenKey);
    await _prefs.remove(_emailKey);
    notifyListeners();
  }
}

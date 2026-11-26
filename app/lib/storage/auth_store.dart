import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

class AuthStore extends ChangeNotifier {
  AuthStore._(this._prefs, this._token, this._email, this._offline);

  static const _tokenKey = 'rustpocket.token';
  static const _emailKey = 'rustpocket.email';
  static const _offlineKey = 'rustpocket.offline';

  final SharedPreferences _prefs;
  String? _token;
  String? _email;
  bool _offline;

  static AuthStore load(SharedPreferences prefs) {
    return AuthStore._(
      prefs,
      prefs.getString(_tokenKey),
      prefs.getString(_emailKey),
      prefs.getBool(_offlineKey) ?? false,
    );
  }

  String? get token => _token;
  String? get email => _email;
  bool get isOffline => _offline;
  bool get isAuthenticated =>
      _offline || (_token != null && _token!.isNotEmpty);

  /// Opens the app without an account so the editor can be used while the
  /// server is unreachable. Compilation still requires a real session.
  Future<void> continueOffline() async {
    _offline = true;
    _token = null;
    _email = null;
    await _prefs.setBool(_offlineKey, true);
    await _prefs.remove(_tokenKey);
    await _prefs.remove(_emailKey);
    notifyListeners();
  }

  Future<void> signIn({required String token, required String email}) async {
    _offline = false;
    _token = token;
    _email = email;
    await _prefs.remove(_offlineKey);
    await _prefs.setString(_tokenKey, token);
    await _prefs.setString(_emailKey, email);
    notifyListeners();
  }

  Future<void> signOut() async {
    _offline = false;
    _token = null;
    _email = null;
    await _prefs.remove(_offlineKey);
    await _prefs.remove(_tokenKey);
    await _prefs.remove(_emailKey);
    notifyListeners();
  }
}

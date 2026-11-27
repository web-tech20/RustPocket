import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

class AuthStore extends ChangeNotifier {
  AuthStore._(this._prefs, this._token, this._email, this._offline);

  static const _tokenKey = 'rustpocket.token';
  static const _emailKey = 'rustpocket.email';
  static const _offlineKey = 'rustpocket.offline';
  static const _accountsKey = 'rustpocket.local_accounts';

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

  /// Local accounts stored on this device (email -> password).
  Map<String, String> _accounts() {
    final raw = _prefs.getString(_accountsKey);
    if (raw == null || raw.isEmpty) return <String, String>{};
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return <String, String>{};
      return decoded.map(
        (key, value) => MapEntry(key.toString(), value.toString()),
      );
    } catch (_) {
      return <String, String>{};
    }
  }

  Future<void> _saveAccounts(Map<String, String> accounts) async {
    await _prefs.setString(_accountsKey, jsonEncode(accounts));
  }

  /// Creates a local account on this device and signs in.
  /// Returns false if the email is already registered.
  Future<bool> registerLocal({
    required String email,
    required String password,
  }) async {
    final normalized = email.trim().toLowerCase();
    final accounts = _accounts();
    if (accounts.containsKey(normalized)) return false;
    accounts[normalized] = password;
    await _saveAccounts(accounts);
    await signIn(token: 'local', email: normalized);
    return true;
  }

  /// Signs in with a previously created local account.
  /// Returns false if the credentials do not match.
  Future<bool> loginLocal({
    required String email,
    required String password,
  }) async {
    final normalized = email.trim().toLowerCase();
    final accounts = _accounts();
    if (accounts[normalized] != password) return false;
    await signIn(token: 'local', email: normalized);
    return true;
  }

  /// True when a local account exists for [email].
  bool hasLocalAccount(String email) =>
      _accounts().containsKey(email.trim().toLowerCase());

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

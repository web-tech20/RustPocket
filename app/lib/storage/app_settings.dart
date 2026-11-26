import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:shared_preferences/shared_preferences.dart';

class AppSettings extends ChangeNotifier {
  AppSettings._(
    this._prefs, {
    required this._themeMode,
    required this._editorFontSize,
    required this._apiBaseUrl,
  });

  static const _themeKey = 'rustpocket.theme';
  static const _fontKey = 'rustpocket.fontSize';
  static const _serverKey = 'rustpocket.apiBaseUrl';

  static const defaultFontSize = 14.0;

  final SharedPreferences _prefs;
  ThemeMode _themeMode;
  double _editorFontSize;
  String _apiBaseUrl;

  static AppSettings load(SharedPreferences prefs) {
    return AppSettings._(
      prefs,
      themeMode: _parseThemeMode(prefs.getString(_themeKey)),
      editorFontSize: (prefs.getDouble(_fontKey) ?? defaultFontSize).clamp(
        12.0,
        22.0,
      ),
      apiBaseUrl: prefs.getString(_serverKey) ?? defaultApiBaseUrl(),
    );
  }

  ThemeMode get themeMode => _themeMode;
  double get editorFontSize => _editorFontSize;
  String get apiBaseUrl => _apiBaseUrl;

  set themeMode(ThemeMode mode) {
    if (mode == _themeMode) return;
    _themeMode = mode;
    _prefs.setString(_themeKey, mode.name);
    notifyListeners();
  }

  set editorFontSize(double size) {
    final clamped = size.clamp(12.0, 22.0);
    if (clamped == _editorFontSize) return;
    _editorFontSize = clamped;
    _prefs.setDouble(_fontKey, clamped);
    notifyListeners();
  }

  /// Applies [url] when it is a usable address. Returns false when the value
  /// was rejected, so the caller can show [validateBaseUrl] to the user.
  bool setApiBaseUrl(String url) {
    if (validateBaseUrl(url) != null) return false;
    final normalized = normalizeBaseUrl(url);
    if (normalized == _apiBaseUrl) return true;
    _apiBaseUrl = normalized;
    _prefs.setString(_serverKey, normalized);
    notifyListeners();
    return true;
  }

  /// Null when [url] can be used as an API base, otherwise a French message.
  static String? validateBaseUrl(String url) {
    final trimmed = url.trim();
    if (trimmed.isEmpty) {
      return 'Saisissez l’adresse du serveur.';
    }
    final uri = Uri.tryParse(normalizeBaseUrl(trimmed));
    if (uri == null ||
        (uri.scheme != 'http' && uri.scheme != 'https') ||
        uri.host.isEmpty) {
      return 'Adresse du serveur invalide. Exemple : http://192.168.1.20:8080';
    }
    return null;
  }

  /// Emulator-friendly default; set `RUSTPOCKET_API_URL` at build time or
  /// change the address from the login/settings screen for a real device.
  static String defaultApiBaseUrl() {
    const defined = String.fromEnvironment('RUSTPOCKET_API_URL');
    if (defined.isNotEmpty) return defined;
    if (kIsWeb) return 'http://127.0.0.1:8080';
    if (defaultTargetPlatform == TargetPlatform.android) {
      return 'http://10.0.2.2:8080';
    }
    return 'http://127.0.0.1:8080';
  }

  static String normalizeBaseUrl(String url) {
    var value = url.trim();
    // Detect the scheme before trimming slashes: `http://` would otherwise
    // shrink to `http:` and look like a host without a scheme.
    final hasScheme = value.contains('://');
    while (value.endsWith('/')) {
      value = value.substring(0, value.length - 1);
    }
    // Only add a scheme when the user did not provide one; otherwise a
    // non-HTTP scheme would silently become `http://ftp://…`.
    if (value.isNotEmpty && !hasScheme) {
      value = 'http://$value';
    }
    return value;
  }

  static ThemeMode _parseThemeMode(String? value) {
    switch (value) {
      case 'light':
        return ThemeMode.light;
      case 'system':
        return ThemeMode.system;
      default:
        return ThemeMode.dark;
    }
  }
}

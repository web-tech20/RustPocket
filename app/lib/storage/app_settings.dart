import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
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

  void setApiBaseUrl(String url) {
    final normalized = normalizeBaseUrl(url);
    if (normalized.isEmpty || normalized == _apiBaseUrl) return;
    _apiBaseUrl = normalized;
    _prefs.setString(_serverKey, normalized);
    notifyListeners();
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
    while (value.endsWith('/')) {
      value = value.substring(0, value.length - 1);
    }
    if (value.isNotEmpty && !value.startsWith('http')) {
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

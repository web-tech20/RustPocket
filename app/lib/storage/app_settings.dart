import 'package:material_ui/material_ui.dart';
import 'package:shared_preferences/shared_preferences.dart';

class AppSettings extends ChangeNotifier {
  AppSettings._(
    this._prefs, {
    required this._themeMode,
    required this._editorFontSize,
  });

  static const _themeKey = 'rustpocket.theme';
  static const _fontKey = 'rustpocket.fontSize';

  static const defaultFontSize = 14.0;

  final SharedPreferences _prefs;
  ThemeMode _themeMode;
  double _editorFontSize;

  static AppSettings load(SharedPreferences prefs) {
    return AppSettings._(
      prefs,
      themeMode: _parseThemeMode(prefs.getString(_themeKey)),
      editorFontSize: (prefs.getDouble(_fontKey) ?? defaultFontSize).clamp(
        12.0,
        22.0,
      ),
    );
  }

  ThemeMode get themeMode => _themeMode;
  double get editorFontSize => _editorFontSize;

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

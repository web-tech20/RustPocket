import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rustpocket/storage/app_settings.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  test('AppSettings charge les valeurs par défaut et borne la police', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final settings = AppSettings.load(prefs);

    expect(settings.themeMode, ThemeMode.dark);
    expect(settings.editorFontSize, AppSettings.defaultFontSize);

    settings.editorFontSize = 30.0;
    expect(settings.editorFontSize, 22.0);

    settings.editorFontSize = 8.0;
    expect(settings.editorFontSize, 12.0);

    settings.themeMode = ThemeMode.light;
    expect(AppSettings.load(prefs).themeMode, ThemeMode.light);
  });
}

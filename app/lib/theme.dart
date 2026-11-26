import 'package:material_ui/material_ui.dart';
import 'package:flutter_code_editor/flutter_code_editor.dart';

/// Colours from section 8.1 of the specification: sober black and white with
/// the Rust orange as the single accent.
class RustColors {
  static const orange = Color(0xFFF74C00);
  static const rustDark = Color(0xFFB7410E);
  static const grey = Color(0xFF8A8A8A);
  static const black = Color(0xFF000000);
  static const white = Color(0xFFFFFFFF);
  static const offWhite = Color(0xFFF2EDE6);
  static const nightSurface = Color(0xFF121212);
  static const nightBorder = Color(0xFF2A2A2A);
}

ThemeData buildDarkTheme() {
  const scheme = ColorScheme.dark(
    primary: RustColors.orange,
    onPrimary: RustColors.black,
    secondary: RustColors.rustDark,
    onSecondary: RustColors.white,
    surface: RustColors.black,
    onSurface: RustColors.offWhite,
    outline: RustColors.nightBorder,
    error: Color(0xFFFF6B6B),
  );

  return _baseTheme(scheme, Brightness.dark);
}

ThemeData buildLightTheme() {
  const scheme = ColorScheme.light(
    primary: RustColors.orange,
    onPrimary: RustColors.white,
    secondary: RustColors.rustDark,
    onSecondary: RustColors.white,
    surface: RustColors.white,
    onSurface: RustColors.black,
    outline: Color(0xFFD6D6D6),
    error: Color(0xFFB3261E),
  );

  return _baseTheme(scheme, Brightness.light);
}

ThemeData _baseTheme(ColorScheme scheme, Brightness brightness) {
  final dark = brightness == Brightness.dark;
  return ThemeData(
    useMaterial3: true,
    brightness: brightness,
    colorScheme: scheme,
    scaffoldBackgroundColor: scheme.surface,
    appBarTheme: AppBarTheme(
      backgroundColor: scheme.surface,
      foregroundColor: scheme.onSurface,
      elevation: 0,
      centerTitle: false,
      surfaceTintColor: Colors.transparent,
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: RustColors.orange,
        foregroundColor: RustColors.black,
        textStyle: const TextStyle(fontWeight: FontWeight.w700),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: scheme.onSurface,
        side: BorderSide(color: scheme.outline),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: dark ? RustColors.nightSurface : const Color(0xFFF7F7F7),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(10),
        borderSide: BorderSide(color: scheme.outline),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(10),
        borderSide: BorderSide(color: scheme.outline),
      ),
    ),
    dividerTheme: DividerThemeData(color: scheme.outline, space: 1),
    snackBarTheme: SnackBarThemeData(
      backgroundColor: dark ? RustColors.nightSurface : RustColors.black,
      contentTextStyle: const TextStyle(color: RustColors.white),
      behavior: SnackBarBehavior.floating,
    ),
  );
}

/// Syntax colours: keywords in orange, strings muted green, comments grey.
/// The light palette uses darker variants so code stays readable on white.
CodeThemeData buildCodeTheme(Brightness brightness) {
  final dark = brightness == Brightness.dark;
  final base = dark ? RustColors.offWhite : RustColors.black;
  final keyword = dark ? RustColors.orange : const Color(0xFFC2410C);
  final builtIn = dark ? const Color(0xFFFFB07C) : const Color(0xFF7C3E00);
  final title = dark ? const Color(0xFFFFD7B5) : const Color(0xFF6B3A00);
  final string = dark ? const Color(0xFF9CCF9F) : const Color(0xFF2F7D32);
  final comment = dark ? RustColors.grey : const Color(0xFF5F6368);
  final number = dark ? const Color(0xFF8FC7F0) : const Color(0xFF1565C0);
  final type = dark ? const Color(0xFFE6C07B) : const Color(0xFF7A5B00);
  return CodeThemeData(
    styles: {
      'root': TextStyle(color: base, backgroundColor: Colors.transparent),
      'keyword': TextStyle(color: keyword, fontWeight: FontWeight.w700),
      'built_in': TextStyle(color: builtIn),
      'title': TextStyle(color: title),
      'string': TextStyle(color: string),
      'comment': TextStyle(color: comment, fontStyle: FontStyle.italic),
      'number': TextStyle(color: number),
      'literal': TextStyle(color: number),
      'type': TextStyle(color: type),
      'meta': TextStyle(color: comment),
      'operator': TextStyle(color: base),
    },
  );
}

/// Shared monospace family. Flutter falls back to the platform monospace font
/// on Android and iOS.
const codeFontFamily = 'monospace';

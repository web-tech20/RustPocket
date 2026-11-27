import 'package:flutter/cupertino.dart' show DefaultCupertinoLocalizations;
import 'package:flutter/material.dart' show DefaultMaterialLocalizations;
import 'package:material_ui/material_ui.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'api/api_client.dart';
import 'screens/home_screen.dart';
import 'screens/login_screen.dart';
import 'storage/app_settings.dart';
import 'storage/auth_store.dart';
import 'storage/project_store.dart';
import 'theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();

  final settings = AppSettings.load(prefs);
  final auth = AuthStore.load(prefs);
  final projects = ProjectStore(prefs);
  final navigatorKey = GlobalKey<NavigatorState>();
  final api = ApiClient();

  runApp(
    RustPocketApp(
      settings: settings,
      auth: auth,
      projects: projects,
      api: api,
      navigatorKey: navigatorKey,
    ),
  );
}

class RustPocketApp extends StatelessWidget {
  const RustPocketApp({
    super.key,
    required this.settings,
    required this.auth,
    required this.projects,
    required this.api,
    required this.navigatorKey,
  });

  final AppSettings settings;
  final AuthStore auth;
  final ProjectStore projects;
  final ApiClient api;
  final GlobalKey<NavigatorState> navigatorKey;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: settings,
      builder: (context, _) {
        return MaterialApp(
          title: 'RustPocket',
          navigatorKey: navigatorKey,
          debugShowCheckedModeBanner: false,
          localizationsDelegates: const [
            DefaultMaterialLocalizations.delegate,
            DefaultCupertinoLocalizations.delegate,
            DefaultWidgetsLocalizations.delegate,
          ],
          theme: buildLightTheme(),
          darkTheme: buildDarkTheme(),
          themeMode: settings.themeMode,
          home: AnimatedBuilder(
            animation: auth,
            builder: (context, _) {
              if (!auth.isAuthenticated) {
                return LoginScreen(auth: auth);
              }
              return HomeScreen(
                api: api,
                auth: auth,
                projects: projects,
                settings: settings,
              );
            },
          ),
        );
      },
    );
  }
}

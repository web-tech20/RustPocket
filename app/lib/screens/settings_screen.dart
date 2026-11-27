import 'package:material_ui/material_ui.dart';

import '../storage/app_settings.dart';
import '../storage/auth_store.dart';
import '../theme.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.auth,
    required this.settings,
  });

  final AuthStore auth;
  final AppSettings settings;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  Future<void> _signOut() async {
    await widget.auth.signOut();
    if (mounted) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(title: const Text('Paramètres')),
      body: AnimatedBuilder(
        animation: widget.settings,
        builder: (context, _) {
          return ListView(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 32),
            children: [
              _sectionTitle(theme, 'Apparence'),
              SegmentedButton<ThemeMode>(
                segments: const [
                  ButtonSegment(
                    value: ThemeMode.dark,
                    label: Text('Sombre'),
                    icon: Icon(Icons.dark_mode_outlined),
                  ),
                  ButtonSegment(
                    value: ThemeMode.light,
                    label: Text('Clair'),
                    icon: Icon(Icons.light_mode_outlined),
                  ),
                  ButtonSegment(
                    value: ThemeMode.system,
                    label: Text('Auto'),
                    icon: Icon(Icons.brightness_auto_outlined),
                  ),
                ],
                selected: {widget.settings.themeMode},
                onSelectionChanged: (selection) {
                  widget.settings.themeMode = selection.first;
                },
              ),
              const SizedBox(height: 20),
              _sectionTitle(theme, 'Éditeur'),
              Row(
                children: [
                  const Icon(Icons.format_size, size: 20),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Slider(
                      value: widget.settings.editorFontSize,
                      min: 12,
                      max: 22,
                      divisions: 10,
                      label: '${widget.settings.editorFontSize.round()} pt',
                      onChanged: (value) {
                        widget.settings.editorFontSize = value;
                      },
                    ),
                  ),
                  SizedBox(
                    width: 44,
                    child: Text(
                      '${widget.settings.editorFontSize.round()}',
                      textAlign: TextAlign.end,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 20),
              _sectionTitle(theme, 'Compte local'),
              Card(
                elevation: 0,
                color: theme.colorScheme.surfaceContainerHighest.withValues(
                  alpha: theme.brightness == Brightness.dark ? 0.35 : 0.45,
                ),
                child: Padding(
                  padding: const EdgeInsets.all(16),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          const Icon(
                            Icons.account_circle_outlined,
                            color: RustColors.orange,
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: Text(
                              widget.auth.email ?? 'Compte local',
                              style: const TextStyle(
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 8),
                      Text(
                        'Compte et projets stockés localement sur cet appareil.',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: RustColors.grey,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              const SizedBox(height: 24),
              OutlinedButton.icon(
                onPressed: _signOut,
                style: OutlinedButton.styleFrom(
                  foregroundColor: theme.colorScheme.error,
                  side: BorderSide(
                    color: theme.colorScheme.error.withValues(alpha: 0.6),
                  ),
                  minimumSize: const Size.fromHeight(48),
                ),
                icon: const Icon(Icons.logout),
                label: const Text('Se déconnecter'),
              ),
              const SizedBox(height: 24),
              Center(
                child: Text(
                  'RustPocket 1.0.0 · IDE Local',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: RustColors.grey,
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  Widget _sectionTitle(ThemeData theme, String label) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8, top: 4),
      child: Text(
        label,
        style: theme.textTheme.titleSmall?.copyWith(
          color: RustColors.grey,
          letterSpacing: 0.4,
        ),
      ),
    );
  }
}

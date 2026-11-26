import 'package:material_ui/material_ui.dart';

import '../api/api_client.dart';
import '../api/models.dart';
import '../storage/app_settings.dart';
import '../storage/auth_store.dart';
import '../theme.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.api,
    required this.auth,
    required this.settings,
  });

  final ApiClient api;
  final AuthStore auth;
  final AppSettings settings;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late final TextEditingController _server;
  AccountInfo? _account;
  bool _loadingAccount = false;
  String? _accountError;

  @override
  void initState() {
    super.initState();
    _server = TextEditingController(text: widget.settings.apiBaseUrl);
    if (!widget.auth.isOffline) _refreshAccount();
  }

  @override
  void dispose() {
    _server.dispose();
    super.dispose();
  }

  Future<void> _refreshAccount() async {
    if (widget.auth.isOffline) return;
    setState(() {
      _loadingAccount = true;
      _accountError = null;
    });
    try {
      final account = await widget.api.me();
      if (mounted) setState(() => _account = account);
    } on ApiException catch (e) {
      if (mounted) setState(() => _accountError = e.message);
    } finally {
      if (mounted) setState(() => _loadingAccount = false);
    }
  }

  void _saveServer() {
    if (!widget.settings.setApiBaseUrl(_server.text)) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            AppSettings.validateBaseUrl(_server.text) ??
                'Adresse du serveur invalide.',
          ),
        ),
      );
      return;
    }
    _server.text = widget.settings.apiBaseUrl;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text('Serveur : ${widget.settings.apiBaseUrl}'),
      ),
    );
    _refreshAccount();
  }

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
              const SizedBox(height: 12),
              _sectionTitle(theme, 'Serveur de compilation'),
              TextField(
                controller: _server,
                keyboardType: TextInputType.url,
                textInputAction: TextInputAction.done,
                onSubmitted: (_) => _saveServer(),
                decoration: const InputDecoration(
                  labelText: 'Adresse du serveur',
                  hintText: 'http://192.168.1.20:8080',
                  helperText:
                      '10.0.2.2 pour un émulateur Android ; IP locale du PC pour un téléphone.',
                  suffixIcon: Icon(Icons.dns_outlined),
                ),
              ),
              const SizedBox(height: 8),
              Align(
                alignment: Alignment.centerRight,
                child: OutlinedButton.icon(
                  onPressed: _saveServer,
                  icon: const Icon(Icons.check),
                  label: const Text('Appliquer'),
                ),
              ),
              const SizedBox(height: 20),
              _sectionTitle(theme, 'Compte'),
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
                              widget.auth.isOffline
                                  ? 'Mode local'
                                  : (widget.auth.email ?? ''),
                              style: const TextStyle(
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      if (widget.auth.isOffline)
                        Text(
                          'Aucune connexion au serveur. L’éditeur et vos '
                          'projets locaux restent disponibles.',
                          style: theme.textTheme.bodySmall,
                        )
                      else if (_loadingAccount)
                        const LinearProgressIndicator(minHeight: 2)
                      else if (_accountError != null)
                        Text(
                          _accountError!,
                          style: TextStyle(color: theme.colorScheme.error),
                        )
                      else if (_account != null) ...[
                        Text(
                          'Quota : ${_account!.quotas.remainingMinute} / '
                          '${_account!.quotas.limitPerMinute} exécutions '
                          'restantes cette minute',
                          style: theme.textTheme.bodySmall,
                        ),
                        const SizedBox(height: 4),
                        Text(
                          '${_account!.quotas.remainingDay} / '
                          '${_account!.quotas.limitPerDay} restantes aujourd’hui',
                          style: theme.textTheme.bodySmall,
                        ),
                      ],
                      if (!widget.auth.isOffline) ...[
                        const SizedBox(height: 8),
                        TextButton.icon(
                          onPressed: _loadingAccount ? null : _refreshAccount,
                          icon: const Icon(Icons.refresh),
                          label: const Text('Actualiser'),
                        ),
                      ],
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
                label: Text(
                  widget.auth.isOffline
                      ? 'Quitter le mode local'
                      : 'Se déconnecter',
                ),
              ),
              const SizedBox(height: 24),
              Center(
                child: Text(
                  'RustPocket 1.0.0 · MVP',
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

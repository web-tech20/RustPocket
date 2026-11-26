import 'package:flutter/material.dart';

import '../api/api_client.dart';
import '../storage/app_settings.dart';
import '../storage/auth_store.dart';
import '../theme.dart';
import '../widgets/rust_logo.dart';

class LoginScreen extends StatefulWidget {
  const LoginScreen({
    super.key,
    required this.api,
    required this.auth,
    required this.settings,
  });

  final ApiClient api;
  final AuthStore auth;
  final AppSettings settings;

  @override
  State<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends State<LoginScreen> {
  final _email = TextEditingController();
  final _password = TextEditingController();
  late final TextEditingController _server;
  bool _registering = false;
  bool _busy = false;
  bool _obscure = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _server = TextEditingController(text: widget.settings.apiBaseUrl);
  }

  @override
  void dispose() {
    _email.dispose();
    _password.dispose();
    _server.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final email = _email.text.trim();
    final password = _password.text;

    if (!email.contains('@')) {
      setState(() => _error = 'Saisissez une adresse e-mail valide.');
      return;
    }
    if (password.length < 8) {
      setState(() => _error = 'Le mot de passe doit contenir 8 caractères.');
      return;
    }

    setState(() {
      _busy = true;
      _error = null;
    });
    widget.settings.setApiBaseUrl(_server.text);

    try {
      final session = _registering
          ? await widget.api.register(email, password)
          : await widget.api.login(email, password);
      await widget.auth.signIn(token: session.token, email: session.email);
    } on ApiException catch (e) {
      if (mounted) setState(() => _error = e.message);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 32),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 430),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Center(child: RustLogo(size: 104)),
                  const SizedBox(height: 20),
                  Text(
                    'RustPocket',
                    textAlign: TextAlign.center,
                    style: theme.textTheme.headlineMedium?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Écrivez, compilez et exécutez du Rust depuis votre poche.',
                    textAlign: TextAlign.center,
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: RustColors.grey,
                    ),
                  ),
                  const SizedBox(height: 32),
                  TextField(
                    controller: _email,
                    keyboardType: TextInputType.emailAddress,
                    autofillHints: const [AutofillHints.email],
                    textInputAction: TextInputAction.next,
                    decoration: const InputDecoration(
                      labelText: 'Adresse e-mail',
                      prefixIcon: Icon(Icons.alternate_email),
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _password,
                    obscureText: _obscure,
                    autofillHints: const [AutofillHints.password],
                    textInputAction: TextInputAction.done,
                    onSubmitted: (_) => _busy ? null : _submit(),
                    decoration: InputDecoration(
                      labelText: 'Mot de passe',
                      prefixIcon: const Icon(Icons.lock_outline),
                      helperText: _registering
                          ? '8 caractères minimum'
                          : null,
                      suffixIcon: IconButton(
                        tooltip: _obscure ? 'Afficher' : 'Masquer',
                        icon: Icon(
                          _obscure ? Icons.visibility : Icons.visibility_off,
                        ),
                        onPressed: () =>
                            setState(() => _obscure = !_obscure),
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                  ExpansionTile(
                    tilePadding: EdgeInsets.zero,
                    title: Text(
                      'Serveur',
                      style: theme.textTheme.bodyMedium?.copyWith(
                        color: RustColors.grey,
                      ),
                    ),
                    subtitle: Text(
                      widget.settings.apiBaseUrl,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: RustColors.grey,
                      ),
                    ),
                    children: [
                      TextField(
                        controller: _server,
                        keyboardType: TextInputType.url,
                        decoration: const InputDecoration(
                          labelText: 'Adresse du serveur',
                          hintText: 'http://192.168.1.20:8080',
                          helperText:
                              '10.0.2.2 sur un émulateur Android, IP du PC sur un téléphone.',
                        ),
                      ),
                      const SizedBox(height: 8),
                    ],
                  ),
                  if (_error != null) ...[
                    const SizedBox(height: 8),
                    Container(
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: theme.colorScheme.error.withValues(alpha: 0.12),
                        borderRadius: BorderRadius.circular(10),
                      ),
                      child: Text(
                        _error!,
                        style: TextStyle(color: theme.colorScheme.error),
                      ),
                    ),
                  ],
                  const SizedBox(height: 20),
                  FilledButton(
                    onPressed: _busy ? null : _submit,
                    style: FilledButton.styleFrom(
                      minimumSize: const Size.fromHeight(52),
                    ),
                    child: _busy
                        ? const SizedBox(
                            width: 22,
                            height: 22,
                            child: CircularProgressIndicator(
                              strokeWidth: 2,
                              color: RustColors.black,
                            ),
                          )
                        : Text(
                            _registering ? 'Créer mon compte' : 'Se connecter',
                          ),
                  ),
                  const SizedBox(height: 10),
                  TextButton(
                    onPressed: _busy
                        ? null
                        : () => setState(() {
                            _registering = !_registering;
                            _error = null;
                          }),
                    child: Text(
                      _registering
                          ? 'Déjà un compte ? Se connecter'
                          : 'Pas encore de compte ? S’inscrire',
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

import 'package:material_ui/material_ui.dart';

import '../api/models.dart';
import '../theme.dart';

class OutputPanel extends StatelessWidget {
  const OutputPanel({
    super.key,
    required this.busy,
    this.busyLabel = 'Exécution en cours…',
    required this.message,
    required this.compile,
    required this.run,
    required this.onLineTap,
    required this.onClose,
    this.waitingForInput = false,
    this.stdinController,
    this.onSubmitStdin,
  });

  final bool busy;
  final String busyLabel;
  final String? message;
  final CompileResult? compile;
  final RunResult? run;
  final ValueChanged<int> onLineTap;
  final VoidCallback onClose;
  final bool waitingForInput;
  final TextEditingController? stdinController;
  final VoidCallback? onSubmitStdin;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final diagnostics = run?.compile.diagnostics ?? compile?.diagnostics ?? [];
    final compileResult = run?.compile ?? compile;

    return Container(
      color: theme.brightness == Brightness.dark
          ? RustColors.nightSurface
          : const Color(0xFFF5F5F5),
      child: Column(
        children: [
          _header(theme, compileResult),
          Divider(height: 1, color: theme.colorScheme.outline),
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(12, 12, 12, 20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (message != null) ...[
                    _messageBanner(theme),
                    const SizedBox(height: 10),
                  ],
                  if (busy)
                    const Padding(
                      padding: EdgeInsets.symmetric(vertical: 16),
                      child: Center(
                        child: SizedBox(
                          width: 24,
                          height: 24,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                      ),
                    )
                  else ...[
                    for (final diagnostic in diagnostics)
                      _DiagnosticTile(
                        diagnostic: diagnostic,
                        onTap: diagnostic.line == null
                            ? null
                            : () => onLineTap(diagnostic.line!),
                      ),
                    if (diagnostics.isEmpty &&
                        compileResult != null &&
                        !compileResult.success &&
                        compileResult.rawOutput.trim().isNotEmpty)
                      _outputSection(
                        theme,
                        title: 'Détails',
                        body: compileResult.rawOutput,
                        error: true,
                      )
                    else if (diagnostics.isEmpty && compileResult != null)
                      _quietLine(theme, 'Aucun diagnostic.'),
                    if (run != null) ...[
                      if (diagnostics.isNotEmpty || compileResult != null)
                        const SizedBox(height: 12),
                      _outputSection(
                        theme,
                        title: 'Sortie standard',
                        body: run!.stdout.isEmpty ? '(pas de sortie)' : run!.stdout,
                        muted: run!.stdout.isEmpty,
                      ),
                      if (run!.stderr.isNotEmpty) ...[
                        const SizedBox(height: 12),
                        _outputSection(
                          theme,
                          title: 'Erreur standard',
                          body: run!.stderr,
                          error: true,
                        ),
                      ],
                      if (run!.truncated || compileResult?.truncated == true)
                        Padding(
                          padding: const EdgeInsets.only(top: 10),
                          child: Text(
                            'Sortie tronquée : le programme a produit trop de texte.',
                            style: TextStyle(
                              color: theme.colorScheme.error,
                              fontSize: 12,
                            ),
                          ),
                        ),
                    ],
                    if (!busy &&
                        message == null &&
                        compileResult == null &&
                        run == null)
                      _quietLine(
                        theme,
                        'Lancez le code avec Run pour voir sa sortie ici.',
                      ),
                  ],
                ],
              ),
            ),
          ),
          if (waitingForInput && stdinController != null) ...[
            Divider(height: 1, color: theme.colorScheme.outline),
            Container(
              color: theme.brightness == Brightness.dark
                  ? const Color(0xFF0E1014)
                  : const Color(0xFFEAEAEA),
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
              child: Row(
                children: [
                  const Text(
                    '> ',
                    style: TextStyle(
                      color: Color(0xFF4CAF50),
                      fontFamily: 'monospace',
                      fontSize: 14,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  Expanded(
                    child: TextField(
                      controller: stdinController,
                      autofocus: true,
                      style: TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 13.5,
                        color: theme.colorScheme.onSurface,
                      ),
                      decoration: InputDecoration(
                        border: InputBorder.none,
                        isDense: true,
                        contentPadding: const EdgeInsets.symmetric(vertical: 8),
                        hintText: 'Entrez une valeur…',
                        hintStyle: TextStyle(
                          color: RustColors.grey,
                          fontFamily: 'monospace',
                          fontSize: 13,
                        ),
                      ),
                      textInputAction: TextInputAction.send,
                      onSubmitted: (_) => onSubmitStdin?.call(),
                    ),
                  ),
                  IconButton(
                    tooltip: 'Envoyer',
                    icon: const Icon(
                      Icons.send_rounded,
                      color: Color(0xFF4CAF50),
                      size: 20,
                    ),
                    onPressed: onSubmitStdin,
                    visualDensity: VisualDensity.compact,
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _header(ThemeData theme, CompileResult? compileResult) {
    final (icon, color, text) = _status(theme, compileResult);
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 6, 4, 6),
      child: Row(
        children: [
          Icon(icon, size: 18, color: color),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.bodyMedium?.copyWith(
                color: color,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
          IconButton(
            tooltip: 'Fermer la sortie',
            onPressed: onClose,
            icon: const Icon(Icons.close, size: 18),
            visualDensity: VisualDensity.compact,
          ),
        ],
      ),
    );
  }

  (IconData, Color, String) _status(
    ThemeData theme,
    CompileResult? compileResult,
  ) {
    if (busy) {
      return (Icons.sync, RustColors.orange, busyLabel);
    }
    if (message != null) {
      return (Icons.error_outline, theme.colorScheme.error, message!);
    }
    if (run != null) {
      if (waitingForInput || run!.needsInput) {
        return (
          Icons.keyboard_alt_outlined,
          RustColors.orange,
          'En attente d’entrée clavier…',
        );
      }
      if (run!.timedOut) {
        return (
          Icons.timer_off_outlined,
          theme.colorScheme.error,
          'Arrêté : délai d’exécution dépassé',
        );
      }
      if (run!.compile.success && run!.success) {
        return (
          Icons.check_circle_outline,
          const Color(0xFF4CAF50),
          'Exécution réussie · ${run!.durationMs} ms',
        );
      }
      if (!run!.compile.success) {
        return (
          Icons.error_outline,
          theme.colorScheme.error,
          'Compilation échouée · ${run!.compile.errorCount} erreur(s)',
        );
      }
      return (
        Icons.warning_amber_outlined,
        const Color(0xFFFFB300),
        'Terminé avec le code ${run!.exitCode ?? '?'}',
      );
    }
    if (compileResult != null) {
      if (compileResult.success) {
        return (
          Icons.check_circle_outline,
          const Color(0xFF4CAF50),
          'Aucune erreur · ${compileResult.durationMs} ms'
              '${compileResult.warningCount > 0 ? ' · ${compileResult.warningCount} avertissement(s)' : ''}',
        );
      }
      return (
        Icons.error_outline,
        theme.colorScheme.error,
        '${compileResult.errorCount} erreur(s) de compilation',
      );
    }
    return (Icons.terminal, RustColors.grey, 'Sortie');
  }

  Widget _messageBanner(ThemeData theme) {
    return Container(
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: theme.colorScheme.error.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(
        message!,
        style: TextStyle(color: theme.colorScheme.error, fontSize: 13),
      ),
    );
  }

  Widget _quietLine(ThemeData theme, String text) {
    return Text(
      text,
      style: theme.textTheme.bodySmall?.copyWith(color: RustColors.grey),
    );
  }

  Widget _outputSection(
    ThemeData theme, {
    required String title,
    required String body,
    bool error = false,
    bool muted = false,
  }) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title.toUpperCase(),
          style: theme.textTheme.labelSmall?.copyWith(
            color: muted ? RustColors.grey : RustColors.grey,
            letterSpacing: 0.6,
          ),
        ),
        const SizedBox(height: 4),
        SelectableText(
          body,
          style: TextStyle(
            fontFamily: 'monospace',
            fontSize: 13,
            height: 1.35,
            color: error ? theme.colorScheme.error : theme.colorScheme.onSurface,
          ),
        ),
      ],
    );
  }
}

class _DiagnosticTile extends StatelessWidget {
  const _DiagnosticTile({required this.diagnostic, this.onTap});

  final Diagnostic diagnostic;
  final VoidCallback? onTap;

  Color _color(ThemeData theme) {
    switch (diagnostic.level) {
      case 'error':
        return theme.colorScheme.error;
      case 'warning':
        return const Color(0xFFFFB300);
      default:
        return RustColors.grey;
    }
  }

  IconData get _icon {
    switch (diagnostic.level) {
      case 'error':
        return Icons.error_outline;
      case 'warning':
        return Icons.warning_amber_outlined;
      default:
        return Icons.info_outline;
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = _color(theme);

    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Container(
        margin: const EdgeInsets.only(bottom: 8),
        padding: const EdgeInsets.fromLTRB(10, 8, 10, 8),
        decoration: BoxDecoration(
          border: Border(left: BorderSide(color: color, width: 3)),
          color: color.withValues(alpha: 0.06),
          borderRadius: const BorderRadius.only(
            topRight: Radius.circular(8),
            bottomRight: Radius.circular(8),
          ),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(_icon, size: 16, color: color),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Wrap(
                    spacing: 8,
                    runSpacing: 4,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      if (diagnostic.line != null)
                        Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 6,
                            vertical: 1,
                          ),
                          decoration: BoxDecoration(
                            color: color.withValues(alpha: 0.16),
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: Text(
                            'ligne ${diagnostic.line}',
                            style: TextStyle(fontSize: 11, color: color),
                          ),
                        ),
                      Text(
                        diagnostic.message,
                        style: const TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ),
                  if (diagnostic.rendered != null &&
                      diagnostic.rendered!.trim().isNotEmpty) ...[
                    const SizedBox(height: 4),
                    Text(
                      diagnostic.rendered!,
                      style: TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 11.5,
                        height: 1.3,
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.7,
                        ),
                      ),
                    ),
                  ],
                ],
              ),
            ),
            if (onTap != null)
              Icon(
                Icons.arrow_forward_ios,
                size: 12,
                color: RustColors.grey,
              ),
          ],
        ),
      ),
    );
  }
}

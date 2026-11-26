import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_code_editor/flutter_code_editor.dart';
import 'package:highlight/languages/rust.dart';

import '../api/api_client.dart';
import '../api/models.dart';
import '../storage/app_settings.dart';
import '../storage/project_store.dart';
import '../theme.dart';
import '../widgets/output_panel.dart';

class EditorScreen extends StatefulWidget {
  const EditorScreen({
    super.key,
    required this.project,
    required this.api,
    required this.projects,
    required this.settings,
  });

  final Project project;
  final ApiClient api;
  final ProjectStore projects;
  final AppSettings settings;

  @override
  State<EditorScreen> createState() => _EditorScreenState();
}

class _EditorScreenState extends State<EditorScreen> {
  late Project _project;
  late CodeController _controller;
  final FocusNode _focusNode = FocusNode();

  Timer? _saveTimer;
  bool _busy = false;
  bool _panelOpen = false;
  String? _message;
  CompileResult? _checkResult;
  RunResult? _runResult;

  @override
  void initState() {
    super.initState();
    _project = widget.project;
    _controller = CodeController(text: _project.code, language: rust)
      ..addListener(_onCodeChanged);
  }

  @override
  void dispose() {
    _saveTimer?.cancel();
    _saveNow();
    _controller.removeListener(_onCodeChanged);
    _controller.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  void _onCodeChanged() {
    if (_controller.fullText == _project.code) return;
    _saveTimer?.cancel();
    _saveTimer = Timer(const Duration(milliseconds: 600), _saveNow);
  }

  void _saveNow() {
    _saveTimer?.cancel();
    final code = _controller.fullText;
    if (code == _project.code) return;
    _project = _project.copyWith(code: code, updatedAt: DateTime.now());
    widget.projects.save(_project);
  }

  Future<void> _run() async {
    if (_busy) return;
    _saveNow();
    FocusScope.of(context).unfocus();
    setState(() {
      _busy = true;
      _panelOpen = true;
      _message = null;
      _runResult = null;
      _checkResult = null;
    });

    try {
      final result = await widget.api.run(_controller.fullText);
      if (!mounted) return;
      setState(() {
        _runResult = result;
        _busy = false;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _message = e.message;
        _busy = false;
      });
    }
  }

  Future<void> _check() async {
    if (_busy) return;
    _saveNow();
    FocusScope.of(context).unfocus();
    setState(() {
      _busy = true;
      _panelOpen = true;
      _message = null;
      _runResult = null;
      _checkResult = null;
    });

    try {
      final result = await widget.api.check(_controller.fullText);
      if (!mounted) return;
      setState(() {
        _checkResult = result;
        _busy = false;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _message = e.message;
        _busy = false;
      });
    }
  }

  Future<void> _format() async {
    if (_busy) return;
    final offset = _controller.selection.baseOffset;
    setState(() {
      _busy = true;
      _message = null;
    });

    try {
      final formatted = await widget.api.format(_controller.fullText);
      if (!mounted) return;
      _controller.fullText = formatted;
      _controller.selection = TextSelection.collapsed(
        offset: offset < 0 ? 0 : offset.clamp(0, formatted.length),
      );
      _saveNow();
      setState(() => _busy = false);
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Code formaté avec rustfmt')),
      );
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _message = e.message;
        _panelOpen = true;
      });
    }
  }

  void _goToLine(int line) {
    final text = _controller.fullText;
    var offset = 0;
    for (var current = 1; current < line; current++) {
      final newline = text.indexOf('\n', offset);
      if (newline < 0) break;
      offset = newline + 1;
    }
    _controller.selection = TextSelection.collapsed(offset: offset);
    setState(() => _panelOpen = false);
    _focusNode.requestFocus();
  }

  Future<void> _rename() async {
    final controller = TextEditingController(text: _project.name);
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Renommer le projet'),
        content: TextField(
          controller: controller,
          autofocus: true,
          maxLength: 60,
          decoration: const InputDecoration(labelText: 'Nom du projet'),
          onSubmitted: (value) {
            if (value.trim().isNotEmpty) Navigator.pop(context, value.trim());
          },
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Annuler'),
          ),
          FilledButton(
            onPressed: () {
              final value = controller.text.trim();
              if (value.isNotEmpty) Navigator.pop(context, value);
            },
            child: const Text('Valider'),
          ),
        ],
      ),
    );
    controller.dispose();
    if (name == null) return;

    await widget.projects.rename(_project.id, name);
    if (!mounted) return;
    setState(() => _project = _project.copyWith(name: name));
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.settings,
      builder: (context, _) {
        final theme = Theme.of(context);
        final brightness = theme.brightness;
        final editorBackground = brightness == Brightness.dark
            ? RustColors.black
            : RustColors.white;
        final fontSize = widget.settings.editorFontSize;

        return Scaffold(
          resizeToAvoidBottomInset: true,
          appBar: AppBar(
            titleSpacing: 0,
            title: InkWell(
              onTap: _rename,
              borderRadius: BorderRadius.circular(6),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Flexible(
                      child: Text(
                        _project.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    const SizedBox(width: 6),
                    const Icon(Icons.edit_outlined, size: 14),
                  ],
                ),
              ),
            ),
            actions: [
              IconButton(
                tooltip: 'Enregistrer',
                onPressed: () {
                  _saveNow();
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(content: Text('Projet enregistré')),
                  );
                },
                icon: const Icon(Icons.save_outlined),
              ),
            ],
          ),
          body: SafeArea(
            top: false,
            child: Column(
              children: [
                Expanded(
                  child: CodeTheme(
                    data: buildCodeTheme(brightness),
                    child: CodeField(
                      controller: _controller,
                      focusNode: _focusNode,
                      expands: true,
                      maxLines: null,
                      minLines: null,
                      wrap: false,
                      background: editorBackground,
                      padding: const EdgeInsets.fromLTRB(8, 12, 12, 12),
                      textStyle: TextStyle(
                        fontFamily: codeFontFamily,
                        fontSize: fontSize,
                        height: 1.45,
                        color: theme.colorScheme.onSurface,
                      ),
                      cursorColor: RustColors.orange,
                      gutterStyle: GutterStyle(
                        width: 48,
                        margin: 8,
                        showFoldingHandles: false,
                        showErrors: false,
                        background: editorBackground,
                        textStyle: TextStyle(
                          fontFamily: codeFontFamily,
                          fontSize: fontSize - 2,
                          color: RustColors.grey,
                        ),
                      ),
                    ),
                  ),
                ),
                if (_panelOpen) ...[
                  Divider(height: 1, color: theme.colorScheme.outline),
                  Flexible(
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxHeight: 300),
                      child: OutputPanel(
                        busy: _busy,
                        message: _message,
                        compile: _checkResult,
                        run: _runResult,
                        onLineTap: _goToLine,
                        onClose: () => setState(() => _panelOpen = false),
                      ),
                    ),
                  ),
                ],
                _toolbar(theme),
              ],
            ),
          ),
        );
      },
    );
  }

  Widget _toolbar(ThemeData theme) {
    return Container(
      decoration: BoxDecoration(
        color: theme.colorScheme.surface,
        border: Border(top: BorderSide(color: theme.colorScheme.outline)),
      ),
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
      child: Row(
        children: [
          Expanded(
            child: FilledButton.icon(
              onPressed: _busy ? null : _run,
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(46),
              ),
              icon: _busy
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        color: RustColors.black,
                      ),
                    )
                  : const Icon(Icons.play_arrow_rounded),
              label: Text(_busy ? 'Exécution…' : 'Run'),
            ),
          ),
          const SizedBox(width: 6),
          _ToolAction(
            icon: Icons.format_align_left,
            label: 'Format',
            onPressed: _busy ? null : _format,
          ),
          const SizedBox(width: 2),
          _ToolAction(
            icon: Icons.fact_check_outlined,
            label: 'Check',
            onPressed: _busy ? null : _check,
          ),
          IconButton(
            tooltip: _panelOpen ? 'Masquer la sortie' : 'Afficher la sortie',
            onPressed: () => setState(() => _panelOpen = !_panelOpen),
            icon: Icon(
              _panelOpen
                  ? Icons.keyboard_arrow_down
                  : Icons.keyboard_arrow_up,
            ),
          ),
        ],
      ),
    );
  }
}

class _ToolAction extends StatelessWidget {
  const _ToolAction({
    required this.icon,
    required this.label,
    required this.onPressed,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 58,
      height: 46,
      child: TextButton(
        onPressed: onPressed,
        style: TextButton.styleFrom(
          padding: EdgeInsets.zero,
          foregroundColor: Theme.of(context).colorScheme.onSurface,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(8),
          ),
        ),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 20),
            const SizedBox(height: 2),
            Text(label, style: const TextStyle(fontSize: 11)),
          ],
        ),
      ),
    );
  }
}

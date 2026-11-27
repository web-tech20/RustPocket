import 'dart:async';

// ignore: deprecated_member_use
import 'package:flutter/material.dart' as flutter_material;
import 'package:material_ui/material_ui.dart';
import 'package:flutter_code_editor/flutter_code_editor.dart';
import 'package:highlight/languages/rust.dart';

import '../api/api_client.dart';
import '../api/models.dart';
import '../storage/app_settings.dart';
import '../storage/external_file_service.dart';
import '../storage/project_store.dart';
import '../theme.dart';
import '../widgets/output_panel.dart';

enum _BusyAction { run, check, format }

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
  _BusyAction? _busy;
  bool _panelOpen = false;
  String? _message;
  CompileResult? _checkResult;
  RunResult? _runResult;
  String _stdinBuffer = '';
  bool _waitingForInput = false;
  final TextEditingController _stdinController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _project = widget.project;
    _controller = CodeController(text: _project.code, language: rust)
      ..addListener(_onCodeChanged);
    _focusNode.addListener(_onFocusChanged);
  }

  @override
  void dispose() {
    _saveTimer?.cancel();
    _saveNow();
    _controller.removeListener(_onCodeChanged);
    _focusNode.removeListener(_onFocusChanged);
    _controller.dispose();
    _focusNode.dispose();
    _stdinController.dispose();
    super.dispose();
  }

  void _onCodeChanged() {
    if (_controller.fullText == _project.code) return;
    _saveTimer?.cancel();
    _saveTimer = Timer(const Duration(milliseconds: 600), _saveNow);
  }

  void _onFocusChanged() {
    if (mounted) setState(() {});
  }

  String get _busyLabel => switch (_busy) {
        _BusyAction.check => 'Vérification en cours…',
        _BusyAction.format => 'Formatage en cours…',
        _ => 'Exécution en cours…',
      };

  void _saveNow() {
    _saveTimer?.cancel();
    final code = _controller.fullText;
    if (code == _project.code) return;
    _project = _project.copyWith(code: code, updatedAt: DateTime.now());
    widget.projects.save(_project);
  }

  Future<void> _run() async {
    if (_busy != null) return;
    _saveNow();
    FocusScope.of(context).unfocus();
    setState(() {
      _busy = _BusyAction.run;
      _panelOpen = true;
      _message = null;
      _runResult = null;
      _checkResult = null;
      _stdinBuffer = '';
      _waitingForInput = false;
      _stdinController.clear();
    });

    await _executeWithCurrentStdin();
  }

  Future<void> _executeWithCurrentStdin() async {
    try {
      final result = await widget.api.run(
        _controller.fullText,
        stdin: _stdinBuffer,
      );
      if (!mounted) return;
      setState(() {
        _runResult = result;
        _waitingForInput = result.needsInput;
        _busy = null;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _message = e.message;
        _waitingForInput = false;
        _busy = null;
      });
    }
  }

  Future<void> _submitStdinLine() async {
    if (_busy != null) return;
    final line = _stdinController.text;
    _stdinController.clear();
    setState(() {
      _stdinBuffer += '$line\n';
      _waitingForInput = false;
      _busy = _BusyAction.run;
    });
    await _executeWithCurrentStdin();
  }

  Future<void> _check() async {
    if (_busy != null) return;
    _saveNow();
    FocusScope.of(context).unfocus();
    setState(() {
      _busy = _BusyAction.check;
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
        _busy = null;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _message = e.message;
        _busy = null;
      });
    }
  }

  Future<void> _format() async {
    if (_busy != null) return;
    final offset = _controller.selection.baseOffset;
    setState(() {
      _busy = _BusyAction.format;
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
      setState(() => _busy = null);
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Code formaté avec rustfmt')),
      );
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = null;
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

  void _insertSnippet(String left, [String right = '']) {
    final text = _controller.fullText;
    final sel = _controller.selection;
    final start = sel.start < 0 ? text.length : sel.start.clamp(0, text.length);
    final end = sel.end < 0 ? start : sel.end.clamp(0, text.length);
    final selected = text.substring(start, end);
    final replacement = '$left$selected$right';
    _controller.fullText = text.replaceRange(start, end, replacement);
    final cursor = right.isNotEmpty && selected.isEmpty
        ? start + left.length
        : start + replacement.length;
    _controller.selection = TextSelection.collapsed(offset: cursor);
    _focusNode.requestFocus();
  }

  void _indent() {
    final text = _controller.fullText;
    final sel = _controller.selection;
    final start = sel.start < 0 ? text.length : sel.start.clamp(0, text.length);
    final end = sel.end < 0 ? start : sel.end.clamp(0, text.length);
    if (start == end) {
      _insertSnippet('    ');
      return;
    }
    final lineStart = text.lastIndexOf('\n', start > 0 ? start - 1 : 0) + 1;
    final selectedBlock = text.substring(lineStart, end);
    final lines = selectedBlock.split('\n');
    final indented = lines.map((l) => '    $l').join('\n');
    _controller.fullText = text.replaceRange(lineStart, end, indented);
    _controller.selection = TextSelection(
      baseOffset: start + 4,
      extentOffset: end + (lines.length * 4),
    );
    _focusNode.requestFocus();
  }

  void _unindent() {
    final text = _controller.fullText;
    final sel = _controller.selection;
    final start = sel.start < 0 ? text.length : sel.start.clamp(0, text.length);
    final end = sel.end < 0 ? start : sel.end.clamp(0, text.length);
    final lineStart = start == 0
        ? 0
        : text.lastIndexOf('\n', start - 1) + 1;
    final lineEndIdx = text.indexOf('\n', end);
    final lineEnd = lineEndIdx < 0 ? text.length : lineEndIdx;
    final block = text.substring(lineStart, lineEnd);
    final lines = block.split('\n');
    var removedFirst = 0;
    var removedTotal = 0;
    final updatedLines = <String>[];
    for (var i = 0; i < lines.length; i++) {
      final line = lines[i];
      var remove = 0;
      while (remove < 4 && remove < line.length && line[remove] == ' ') {
        remove++;
      }
      if (remove == 0 && line.startsWith('\t')) {
        remove = 1;
      }
      if (i == 0) removedFirst = remove;
      removedTotal += remove;
      updatedLines.add(line.substring(remove));
    }
    if (removedTotal == 0) {
      _focusNode.requestFocus();
      return;
    }
    final replacement = updatedLines.join('\n');
    _controller.fullText = text.replaceRange(lineStart, lineEnd, replacement);
    _controller.selection = TextSelection(
      baseOffset: (start - removedFirst).clamp(lineStart, _controller.fullText.length),
      extentOffset: (end - removedTotal).clamp(lineStart, _controller.fullText.length),
    );
    _focusNode.requestFocus();
  }

  Future<void> _openExternalFile() async {
    try {
      final file = await ExternalFileService.openFile();
      if (file == null || !mounted) return;
      _saveNow();
      final created = await widget.projects.create(
        name: file.name,
        code: file.content,
      );
      if (!mounted) return;
      setState(() {
        _project = created;
        _controller.fullText = created.code;
        _checkResult = null;
        _runResult = null;
        _message = null;
      });
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Fichier ouvert : ${file.name}')),
      );
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Impossible d’ouvrir le fichier : $e')),
      );
    }
  }

  Future<void> _saveFileAs() async {
    try {
      _saveNow();
      final savedName = await ExternalFileService.saveFileAs(
        suggestedName: _project.name,
        content: _controller.fullText,
      );
      if (savedName == null || !mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Enregistré sous : $savedName')),
      );
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Impossible d’enregistrer : $e')),
      );
    }
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
    WidgetsBinding.instance.addPostFrameCallback((_) => controller.dispose());
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
        final panelMaxHeight =
            (MediaQuery.sizeOf(context).height * 0.42).clamp(140.0, 320.0);

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
              if (_focusNode.hasFocus)
                IconButton(
                  tooltip: 'Masquer le clavier',
                  onPressed: () => _focusNode.unfocus(),
                  icon: const Icon(Icons.keyboard_hide_outlined),
                ),
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
              PopupMenuButton<String>(
                tooltip: 'Fichier',
                onSelected: (value) {
                  if (value == 'open') _openExternalFile();
                  if (value == 'save_as') _saveFileAs();
                },
                itemBuilder: (context) => const [
                  PopupMenuItem(
                    value: 'open',
                    child: ListTile(
                      leading: Icon(Icons.folder_open_outlined),
                      title: Text('Ouvrir un fichier…'),
                      contentPadding: EdgeInsets.zero,
                    ),
                  ),
                  PopupMenuItem(
                    value: 'save_as',
                    child: ListTile(
                      leading: Icon(Icons.save_as_outlined),
                      title: Text('Enregistrer sous…'),
                      contentPadding: EdgeInsets.zero,
                    ),
                  ),
                ],
              ),
            ],
          ),
          body: SafeArea(
            top: false,
            child: Column(
              children: [
                Expanded(
                  // CodeField embeds a TextField, which requires a Material
                  // ancestor for its ink/selection rendering.
                  child: Localizations(
                    locale: const Locale('en', 'US'),
                    delegates: const [
                      flutter_material.DefaultMaterialLocalizations.delegate,
                      DefaultWidgetsLocalizations.delegate,
                    ],
                    child: flutter_material.Material(
                      color: Colors.transparent,
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
                  ),
                ),
                if (_panelOpen) ...[
                  Divider(height: 1, color: theme.colorScheme.outline),
                  Flexible(
                    child: ConstrainedBox(
                      constraints: BoxConstraints(maxHeight: panelMaxHeight),
                      child: OutputPanel(
                        busy: _busy != null,
                        busyLabel: _busyLabel,
                        message: _message,
                        compile: _checkResult,
                        run: _runResult,
                        waitingForInput: _waitingForInput,
                        stdinController: _stdinController,
                        onSubmitStdin: _submitStdinLine,
                        onLineTap: _goToLine,
                        onClose: () => setState(() {
                          _panelOpen = false;
                          _waitingForInput = false;
                        }),
                      ),
                    ),
                  ),
                ],
                _quickKeysBar(theme),
                _toolbar(theme),
              ],
            ),
          ),
        );
      },
    );
  }

  Widget _quickKeysBar(ThemeData theme) {
    return Container(
      height: 40,
      decoration: BoxDecoration(
        color: theme.colorScheme.surface,
        border: Border(top: BorderSide(color: theme.colorScheme.outline)),
      ),
      child: ListView(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        children: [
          _QuickKey(label: '⇥ Tab', onTap: _indent),
          _QuickKey(label: '⇤', onTap: _unindent),
          _QuickKey(label: '{ }', onTap: () => _insertSnippet('{', '}')),
          _QuickKey(label: '( )', onTap: () => _insertSnippet('(', ')')),
          _QuickKey(label: '[ ]', onTap: () => _insertSnippet('[', ']')),
          _QuickKey(label: ';', onTap: () => _insertSnippet(';')),
          _QuickKey(label: '->', onTap: () => _insertSnippet(' -> ')),
          _QuickKey(label: '::', onTap: () => _insertSnippet('::')),
          _QuickKey(label: '"', onTap: () => _insertSnippet('"', '"')),
          _QuickKey(label: '&', onTap: () => _insertSnippet('&')),
          _QuickKey(label: '//', onTap: () => _insertSnippet('// ')),
          _QuickKey(label: '< >', onTap: () => _insertSnippet('<', '>')),
          _QuickKey(label: '_', onTap: () => _insertSnippet('_')),
          _QuickKey(label: '!', onTap: () => _insertSnippet('!')),
        ],
      ),
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
              onPressed: _busy != null ? null : _run,
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(46),
              ),
              icon: _busy == _BusyAction.run
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        color: RustColors.black,
                      ),
                    )
                  : const Icon(Icons.play_arrow_rounded),
              label: Text(_busy == _BusyAction.run ? 'Exécution…' : 'Run'),
            ),
          ),
          const SizedBox(width: 6),
          _ToolAction(
            icon: Icons.format_align_left,
            label: 'Format',
            busy: _busy == _BusyAction.format,
            onPressed: _busy != null ? null : _format,
          ),
          const SizedBox(width: 2),
          _ToolAction(
            icon: Icons.fact_check_outlined,
            label: 'Check',
            busy: _busy == _BusyAction.check,
            onPressed: _busy != null ? null : _check,
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
    this.busy = false,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onPressed;
  final bool busy;

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
            if (busy)
              const SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            else
              Icon(icon, size: 20),
            const SizedBox(height: 2),
            Text(label, style: const TextStyle(fontSize: 11)),
          ],
        ),
      ),
    );
  }
}

class _QuickKey extends StatelessWidget {
  const _QuickKey({required this.label, required this.onTap});

  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(right: 6),
      child: Material(
        color: theme.colorScheme.surfaceContainerHighest.withValues(
          alpha: theme.brightness == Brightness.dark ? 0.5 : 0.7,
        ),
        borderRadius: BorderRadius.circular(6),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(6),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            child: Center(
              child: Text(
                label,
                style: TextStyle(
                  fontFamily: codeFontFamily,
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: theme.colorScheme.onSurface,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

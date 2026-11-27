import 'package:material_ui/material_ui.dart';

import '../api/api_client.dart';
import '../storage/app_settings.dart';
import '../storage/auth_store.dart';
import '../storage/project_store.dart';
import '../theme.dart';
import '../widgets/rust_logo.dart';
import 'editor_screen.dart';
import 'settings_screen.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({
    super.key,
    required this.api,
    required this.auth,
    required this.projects,
    required this.settings,
  });

  final ApiClient api;
  final AuthStore auth;
  final ProjectStore projects;
  final AppSettings settings;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  late List<Project> _projects;

  @override
  void initState() {
    super.initState();
    _reload();
  }

  void _reload() {
    _projects = widget.projects.all();
  }

  Future<void> _open(Project project) async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => EditorScreen(
          project: project,
          api: widget.api,
          projects: widget.projects,
          settings: widget.settings,
        ),
      ),
    );
    if (mounted) setState(_reload);
  }

  Future<void> _createProject() async {
    final name = await _promptName(title: 'Nouveau projet');
    if (name == null) return;
    final project = await widget.projects.create(name: name);
    if (!mounted) return;
    setState(_reload);
    await _open(project);
  }

  Future<void> _rename(Project project) async {
    final name = await _promptName(
      title: 'Renommer le projet',
      initial: project.name,
    );
    if (name == null) return;
    await widget.projects.rename(project.id, name);
    if (mounted) setState(_reload);
  }

  Future<void> _delete(Project project) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Supprimer le projet ?'),
        content: Text(
          '« ${project.name} » et son code seront supprimés de cet appareil.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Annuler'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Supprimer'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await widget.projects.delete(project.id);
    if (mounted) setState(_reload);
  }

  Future<String?> _promptName({
    required String title,
    String? initial,
  }) async {
    final controller = TextEditingController(text: initial ?? '');
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(title),
        content: TextField(
          controller: controller,
          autofocus: true,
          textCapitalization: TextCapitalization.sentences,
          maxLength: 60,
          decoration: const InputDecoration(
            labelText: 'Nom du projet',
            hintText: 'ex. exercice_boucles',
          ),
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
    return result;
  }

  Future<void> _openSettings() async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => SettingsScreen(
          api: widget.api,
          auth: widget.auth,
          settings: widget.settings,
        ),
      ),
    );
    if (mounted) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: Row(
          children: [
            const RustLogo(size: 30),
            const SizedBox(width: 10),
            const Text(
              'RustPocket',
              style: TextStyle(fontWeight: FontWeight.w800),
            ),
          ],
        ),
        actions: [
          IconButton(
            tooltip: 'Paramètres',
            onPressed: _openSettings,
            icon: const Icon(Icons.settings_outlined),
          ),
          PopupMenuButton<String>(
            tooltip: 'Compte',
            icon: const Icon(Icons.account_circle_outlined),
            onSelected: (value) async {
              switch (value) {
                case 'settings':
                  await _openSettings();
                case 'logout':
                  await widget.auth.signOut();
              }
            },
            itemBuilder: (context) => [
              PopupMenuItem<String>(
                enabled: false,
                child: Text(
                  widget.auth.isOffline
                      ? 'Mode local (hors ligne)'
                      : (widget.auth.email ?? ''),
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: RustColors.grey,
                  ),
                ),
              ),
              const PopupMenuItem(value: 'settings', child: Text('Paramètres')),
              PopupMenuItem(
                value: 'logout',
                child: Text(
                  widget.auth.isOffline
                      ? 'Quitter le mode local'
                      : 'Se déconnecter',
                ),
              ),
            ],
          ),
          const SizedBox(width: 4),
        ],
      ),
      body: _projects.isEmpty ? _emptyState(theme) : _projectList(theme),
      floatingActionButton: _projects.isEmpty
          ? null
          : FloatingActionButton.extended(
              onPressed: _createProject,
              icon: const Icon(Icons.add),
              label: const Text('Nouveau projet'),
            ),
    );
  }

  Widget _emptyState(ThemeData theme) {
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const RustLogo(size: 120),
            const SizedBox(height: 24),
            Text(
              'Aucun projet pour l’instant',
              style: theme.textTheme.titleLarge?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              'Créez un projet, écrivez du Rust et lancez-le en quelques secondes.',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodyMedium?.copyWith(
                color: RustColors.grey,
              ),
            ),
            const SizedBox(height: 24),
            FilledButton.icon(
              onPressed: _createProject,
              icon: const Icon(Icons.add),
              label: const Text('Nouveau projet'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _projectList(ThemeData theme) {
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 96),
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 8),
          child: Text(
            'Projets récents',
            style: theme.textTheme.titleSmall?.copyWith(
              color: RustColors.grey,
              letterSpacing: 0.4,
            ),
          ),
        ),
        for (final project in _projects)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Material(
              color: theme.colorScheme.surfaceContainerHighest.withValues(
                alpha: theme.brightness == Brightness.dark ? 0.35 : 0.45,
              ),
              borderRadius: BorderRadius.circular(12),
              child: ListTile(
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(12),
                ),
                onTap: () => _open(project),
                leading: Container(
                  width: 40,
                  height: 40,
                  decoration: BoxDecoration(
                    color: RustColors.orange.withValues(alpha: 0.14),
                    borderRadius: BorderRadius.circular(9),
                  ),
                  child: const Icon(
                    Icons.code,
                    color: RustColors.orange,
                    size: 22,
                  ),
                ),
                title: Text(
                  project.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontWeight: FontWeight.w600),
                ),
                subtitle: Text(
                  'Modifié ${formatRelativeDate(project.updatedAt)}',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: RustColors.grey,
                  ),
                ),
                trailing: PopupMenuButton<String>(
                  tooltip: 'Options',
                  onSelected: (value) {
                    if (value == 'rename') _rename(project);
                    if (value == 'delete') _delete(project);
                  },
                  itemBuilder: (context) => const [
                    PopupMenuItem(value: 'rename', child: Text('Renommer')),
                    PopupMenuItem(value: 'delete', child: Text('Supprimer')),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

String formatRelativeDate(DateTime date) {
  final now = DateTime.now();
  final day = DateTime(date.year, date.month, date.day);
  final today = DateTime(now.year, now.month, now.day);
  final difference = today.difference(day).inDays;
  final time = '${date.hour.toString().padLeft(2, '0')}:'
      '${date.minute.toString().padLeft(2, '0')}';

  if (difference == 0) return 'aujourd’hui à $time';
  if (difference == 1) return 'hier à $time';
  return 'le ${date.day.toString().padLeft(2, '0')}/'
      '${date.month.toString().padLeft(2, '0')}/${date.year}';
}

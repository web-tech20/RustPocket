import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

const defaultProjectCode = '''fn main() {
    println!("Bonjour, RustPocket !");
}
''';

class Project {
  const Project({
    required this.id,
    required this.name,
    required this.code,
    required this.updatedAt,
  });

  final String id;
  final String name;
  final String code;
  final DateTime updatedAt;

  Project copyWith({String? name, String? code, DateTime? updatedAt}) {
    return Project(
      id: id,
      name: name ?? this.name,
      code: code ?? this.code,
      updatedAt: updatedAt ?? this.updatedAt,
    );
  }

  Map<String, dynamic> toJson() => {
    'id': id,
    'name': name,
    'code': code,
    'updatedAt': updatedAt.toIso8601String(),
  };

  factory Project.fromJson(Map<String, dynamic> json) {
    return Project(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? 'sans_titre',
      code: json['code'] as String? ?? '',
      updatedAt:
          DateTime.tryParse(json['updatedAt'] as String? ?? '') ??
          DateTime.fromMillisecondsSinceEpoch(0),
    );
  }
}

/// Projects are stored locally, which keeps the editor usable without a
/// connection (section 6.1 of the specification).
class ProjectStore {
  ProjectStore(this._prefs);

  static const _key = 'rustpocket.projects.v1';

  final SharedPreferences _prefs;

  List<Project> all() {
    final raw = _prefs.getString(_key);
    if (raw == null || raw.isEmpty) return <Project>[];
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! List) return <Project>[];
      final projects = decoded
          .whereType<Map<String, dynamic>>()
          .map(Project.fromJson)
          .where((p) => p.id.isNotEmpty)
          .toList();
      projects.sort((a, b) => b.updatedAt.compareTo(a.updatedAt));
      return projects;
    } catch (_) {
      return <Project>[];
    }
  }

  Project? byId(String id) {
    for (final project in all()) {
      if (project.id == id) return project;
    }
    return null;
  }

  Future<Project> create({String? name, String? code}) async {
    final now = DateTime.now();
    final project = Project(
      id: now.microsecondsSinceEpoch.toString(),
      name: _cleanName(name) ?? _uniqueName(),
      code: code ?? defaultProjectCode,
      updatedAt: now,
    );
    await save(project);
    return project;
  }

  Future<void> save(Project project) async {
    final projects = all();
    final index = projects.indexWhere((p) => p.id == project.id);
    if (index >= 0) {
      projects[index] = project;
    } else {
      projects.add(project);
    }
    projects.sort((a, b) => b.updatedAt.compareTo(a.updatedAt));
    await _write(projects);
  }

  Future<void> rename(String id, String name) async {
    final project = byId(id);
    final cleaned = _cleanName(name);
    if (project == null || cleaned == null) return;
    await save(project.copyWith(name: cleaned, updatedAt: DateTime.now()));
  }

  Future<void> delete(String id) async {
    final projects = all()..removeWhere((p) => p.id == id);
    await _write(projects);
  }

  Future<void> _write(List<Project> projects) async {
    await _prefs.setString(
      _key,
      jsonEncode(projects.map((p) => p.toJson()).toList()),
    );
  }

  String? _cleanName(String? name) {
    final cleaned = name?.trim().replaceAll(RegExp(r'\s+'), ' ');
    if (cleaned == null || cleaned.isEmpty) return null;
    return cleaned.length > 60 ? cleaned.substring(0, 60) : cleaned;
  }

  String _uniqueName() {
    final existing = all().map((p) => p.name).toSet();
    var index = 1;
    while (existing.contains('projet_$index')) {
      index++;
    }
    return 'projet_$index';
  }
}

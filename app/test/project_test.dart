import 'package:flutter_test/flutter_test.dart';
import 'package:rustpocket/storage/project_store.dart';

void main() {
  test('un projet survit à un aller-retour JSON', () {
    final project = Project(
      id: '42',
      name: 'exercice',
      code: defaultProjectCode,
      updatedAt: DateTime(2026, 11, 26, 10, 30),
    );

    final restored = Project.fromJson(project.toJson());

    expect(restored.id, project.id);
    expect(restored.name, project.name);
    expect(restored.code, project.code);
    expect(restored.updatedAt, project.updatedAt);
  });

  test('le modèle par défaut compile un Hello World', () {
    expect(defaultProjectCode, contains('fn main()'));
    expect(defaultProjectCode, contains('println!'));
  });
}

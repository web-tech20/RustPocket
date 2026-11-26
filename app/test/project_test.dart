import 'package:flutter_test/flutter_test.dart';
import 'package:rustpocket/storage/project_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('le premier projet est ajouté à une liste vide', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final store = ProjectStore(prefs);

    final project = await store.create(name: 'test_local');

    expect(store.all().single.id, project.id);
    expect(store.all().single.name, 'test_local');
  });

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

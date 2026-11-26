import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rustpocket/api/models.dart';
import 'package:rustpocket/widgets/output_panel.dart';

Widget _wrap(Widget child) {
  return MaterialApp(
    home: Scaffold(body: SizedBox(height: 420, child: child)),
  );
}

void main() {
  testWidgets('une erreur de compilation mène à sa ligne', (tester) async {
    int? tappedLine;
    const compile = CompileResult(
      success: false,
      diagnostics: [
        Diagnostic(
          level: 'error',
          message: 'cannot find value `x` in this scope',
          line: 3,
          column: 5,
          rendered: 'error[E0425]: cannot find value `x` in this scope',
        ),
      ],
      rawOutput: '',
      durationMs: 12,
      truncated: false,
    );

    await tester.pumpWidget(
      _wrap(
        OutputPanel(
          busy: false,
          message: null,
          compile: compile,
          run: null,
          onLineTap: (line) => tappedLine = line,
          onClose: () {},
        ),
      ),
    );

    expect(find.text('1 erreur(s) de compilation'), findsOneWidget);
    expect(find.text('ligne 3'), findsOneWidget);

    await tester.tap(find.text('ligne 3'));
    expect(tappedLine, 3);
  });

  testWidgets('une exécution réussie affiche la sortie standard', (
    tester,
  ) async {
    const compile = CompileResult(
      success: true,
      diagnostics: [],
      rawOutput: '',
      durationMs: 18,
      truncated: false,
    );
    const run = RunResult(
      success: true,
      timedOut: false,
      exitCode: 0,
      stdout: 'bonjour\n',
      stderr: '',
      durationMs: 90,
      truncated: false,
      compile: compile,
    );

    await tester.pumpWidget(
      _wrap(
        OutputPanel(
          busy: false,
          message: null,
          compile: null,
          run: run,
          onLineTap: (_) {},
          onClose: () {},
        ),
      ),
    );

    expect(find.text('Exécution réussie · 90 ms'), findsOneWidget);
    expect(find.text('bonjour\n'), findsOneWidget);
  });

  testWidgets('le libellé d’activité suit l’action en cours', (tester) async {
    await tester.pumpWidget(
      _wrap(
        OutputPanel(
          busy: true,
          busyLabel: 'Vérification en cours…',
          message: null,
          compile: null,
          run: null,
          onLineTap: (_) {},
          onClose: () {},
        ),
      ),
    );

    expect(find.text('Vérification en cours…'), findsOneWidget);
  });
}

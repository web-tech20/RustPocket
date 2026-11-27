import 'dart:io' show File, Platform, Process;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

class ExternalFile {
  const ExternalFile({required this.name, required this.content});

  final String name;
  final String content;
}

class ExternalFileService {
  static const _channel = MethodChannel('com.rustpocket/files');

  /// Opens an external `.rs` / text file via the platform's native file picker.
  /// Returns `null` if the user cancels.
  static Future<ExternalFile?> openFile() async {
    if (kIsWeb) {
      throw UnsupportedError('Ouverture de fichier externe non supportée sur le Web.');
    }
    if (Platform.isAndroid) {
      final raw = await _channel.invokeMethod<Map<Object?, Object?>>('openFile');
      if (raw == null) return null;
      return ExternalFile(
        name: raw['name']?.toString() ?? 'fichier.rs',
        content: raw['content']?.toString() ?? '',
      );
    }
    if (Platform.isLinux) {
      final res = await Process.run('zenity', [
        '--file-selection',
        '--title=Ouvrir un fichier Rust',
        '--file-filter=Fichiers Rust (*.rs) | *.rs',
        '--file-filter=Tous les fichiers | *',
      ]);
      if (res.exitCode != 0) return null;
      final path = res.stdout.toString().trim();
      if (path.isEmpty) return null;
      final file = File(path);
      return ExternalFile(
        name: file.uri.pathSegments.isNotEmpty ? file.uri.pathSegments.last : 'fichier.rs',
        content: await file.readAsString(),
      );
    }
    throw UnsupportedError('Sélecteur de fichiers natif non configuré sur cette plateforme.');
  }

  /// Saves [content] to a user-chosen external file via the platform's native
  /// "Save As" dialog. Returns the saved file name, or `null` if cancelled.
  static Future<String?> saveFileAs({
    required String suggestedName,
    required String content,
  }) async {
    if (kIsWeb) {
      throw UnsupportedError('Enregistrement externe non supporté sur le Web.');
    }
    final normalizedName = suggestedName.endsWith('.rs')
        ? suggestedName
        : '$suggestedName.rs';
    if (Platform.isAndroid) {
      return _channel.invokeMethod<String>('saveFileAs', {
        'name': normalizedName,
        'content': content,
      });
    }
    if (Platform.isLinux) {
      final res = await Process.run('zenity', [
        '--file-selection',
        '--save',
        '--confirm-overwrite',
        '--title=Enregistrer sous',
        '--filename=$normalizedName',
      ]);
      if (res.exitCode != 0) return null;
      final path = res.stdout.toString().trim();
      if (path.isEmpty) return null;
      final file = File(path);
      await file.writeAsString(content);
      return file.uri.pathSegments.isNotEmpty ? file.uri.pathSegments.last : normalizedName;
    }
    throw UnsupportedError('Enregistrement natif non configuré sur cette plateforme.');
  }
}

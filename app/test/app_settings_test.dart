import 'package:flutter_test/flutter_test.dart';
import 'package:rustpocket/storage/app_settings.dart';

void main() {
  group('normalizeBaseUrl', () {
    test('ajoute http et retire les slashs finaux', () {
      expect(
        AppSettings.normalizeBaseUrl(' 192.168.1.20:8080/ '),
        'http://192.168.1.20:8080',
      );
    });

    test('conserve une adresse https', () {
      expect(
        AppSettings.normalizeBaseUrl('https://api.example.com/'),
        'https://api.example.com',
      );
    });
  });

  group('validateBaseUrl', () {
    test('accepte une adresse locale sans schéma', () {
      expect(AppSettings.validateBaseUrl('10.0.2.2:8080'), isNull);
    });

    test('accepte une adresse https', () {
      expect(AppSettings.validateBaseUrl('https://rust.example.com'), isNull);
    });

    test('refuse une adresse vide', () {
      expect(AppSettings.validateBaseUrl('   '), isNotNull);
    });

    test('refuse un schéma non http', () {
      expect(AppSettings.validateBaseUrl('ftp://example.com'), isNotNull);
    });

    test('refuse une adresse sans hôte', () {
      expect(AppSettings.validateBaseUrl('http://'), isNotNull);
    });
  });
}

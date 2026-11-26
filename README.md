# RustPocket

![Logo RustPocket](app/assets/images/logo.jpg)

IDE mobile pour écrire, compiler et exécuter du Rust depuis un téléphone.
Ce dépôt contient le **MVP V1** décrit dans le *Cahier des charges – RustPocket*
(v1.1, 26 novembre 2026) : éditeur mobile, serveur de compilation, comptes,
quotas et exécution isolée.

| Couche | Technologie |
| --- | --- |
| Application mobile | Flutter (Android, iOS, web de test) |
| Serveur | Rust, Axum, Tokio |
| Comptes | SQLite, Argon2, JWT |
| Exécution | conteneur Docker jetable, ou processus local limité en développement |

## Ce qui fonctionne

- **Éditeur** : coloration syntaxique Rust, numéros de ligne, plissement masqué,
  police réglable, thème sombre par défaut et thème clair.
- **Run / Check / Format** : `rustc` avec diagnostics JSON, `rustfmt`, sortie
  stdout/stderr, erreurs cliquables qui ramènent à la ligne concernée.
- **Projets locaux** : création, ouverture, renommage, suppression, sauvegarde
  automatique sur l'appareil (fonctionne sans connexion).
- **Comptes** : inscription et connexion, mot de passe haché avec Argon2, jeton
  JWT de 30 jours.
- **Limites** : délai d'exécution, mémoire, taille de sortie, taille du code,
  quotas par minute et par jour.
- **Identité Rust** : noir/blanc + orange `#F74C00`, logo roue dentée et
  téléphone.

## Structure

```
.
├── app/                 Application Flutter
│   ├── lib/api/         Client HTTP et modèles
│   ├── lib/screens/     Connexion, accueil, éditeur, paramètres
│   ├── lib/storage/     Projets, session et réglages persistés
│   ├── lib/widgets/     Panneau de sortie, logo
│   └── assets/images/   Logo
├── server/              API Axum
│   └── src/             Auth, base, quotas, sandbox, routes
├── sandbox/Dockerfile   Image du sandbox d'exécution
├── docker-compose.yml   Déploiement serveur + sandbox
└── .github/workflows/   CI (fmt, clippy, tests, analyze)
```

## Dépendances

SDK Flutter épinglé à **3.50.1** (Dart 3.14.0) : c'est la version utilisée en
local **et** dans la CI (`.github/workflows/ci.yml`). Le SDK et les dépendances
se mettent à jour ensemble, puis la version épinglée dans la CI et dans ce
tableau est alignée.

Contraintes déclarées dans `app/pubspec.yaml` et `server/Cargo.toml`, versions
résolues dans `app/pubspec.lock` et `server/Cargo.lock`. Le serveur utilise Rust
stable (`edition 2024`) et la CI le laisse en `stable` via
`dtolnay/rust-toolchain`, sans épinglage.

| Projet | Dépendance | Contrainte | Résolue | Rôle |
| --- | --- | --- | --- | --- |
| `app` | `cupertino_icons` | `^2.0.0` | `2.0.0` | Icônes de style iOS |
| `app` | `http` | `^1.6.0` | `1.6.0` | Client HTTP vers l'API |
| `app` | `shared_preferences` | `^2.5.6` | `2.5.6` | Persistance locale (projets, session, réglages) |
| `app` | `flutter_code_editor` | `^0.3.5` | `0.3.5` | Éditeur avec numéros de ligne et coloration |
| `app` | `highlight` | `^0.7.0` | `0.7.0` | Coloration syntaxique Rust |
| `app` | `material_ui` | `^1.11.1` | `1.11.1` | Widgets Material ; remplace `package:flutter/material.dart`, déprécié à partir de Flutter 3.47 |
| `app` | `flutter_lints` (dev) | `^6.0.0` | `6.0.0` | Règles d'analyse statique |
| `serveur` | `anyhow` | `1.0.105` | `1.0.105` | Erreurs applicatives |
| `serveur` | `argon2` | `0.6.0` | `0.6.0` | Hachage des mots de passe |
| `serveur` | `axum` | `0.8.9` | `0.8.9` | Serveur HTTP et routage |
| `serveur` | `jsonwebtoken` | `12.0.0` | `12.0.0` | Émission et vérification des JWT (`rust_crypto`) |
| `serveur` | `libc` | `0.2.191` | `0.2.191` | Limites mémoire/CPU et gestion des processus du sandbox |
| `serveur` | `rusqlite` | `0.43.0` | `0.43.0` | SQLite embarquée (comptes, quotas), feature `bundled` |
| `serveur` | `serde` | `1.0.229` | `1.0.229` | Sérialisation des modèles |
| `serveur` | `serde_json` | `1.0.154` | `1.0.154` | Corps JSON de l'API et diagnostics `rustc` |
| `serveur` | `thiserror` | `2.0.21` | `2.0.21` | Types d'erreurs internes |
| `serveur` | `tokio` | `1.54.1` | `1.54.1` | Runtime asynchrone (features `full`) |
| `serveur` | `tower-http` | `0.7.1` | `0.7.1` | CORS et traces HTTP |
| `serveur` | `tracing` | `0.1.44` | `0.1.44` | Journalisation structurée |
| `serveur` | `tracing-subscriber` | `0.3.23` | `0.3.23` | Sortie des logs, filtre `RUST_LOG` |
| `serveur` | `uuid` | `1.29.3` | `1.29.3` | Identifiants de requête et de workspace jetable |

Les dépendances directes des deux projets sont toutes à leur dernière version.
Il reste deux paquets transitifs Flutter plus anciens que leur dernière version,
bloqués en amont : `equatable` (≤ 2.x, contrainte de `flutter_code_editor`) et
`material_color_utilities` (fourni par le SDK Flutter).

Mise à jour : `flutter upgrade`, puis `flutter pub upgrade --major-versions`
dans `app/`, puis `flutter analyze` et `flutter test` avant de reporter la
nouvelle version dans la CI.

## Démarrage

Prérequis : Rust stable avec `rustfmt`, Flutter 3.50.1 (version épinglée dans la CI), et Docker pour le mode
d'exécution isolé en production.

### 1. Serveur

```bash
cd server
cp .env.example .env          # facultatif, le serveur ne lit pas .env seul
RUSTPOCKET_JWT_SECRET=un-secret-long \
RUSTPOCKET_DATABASE=/tmp/rustpocket.db \
cargo run
```

Le serveur écoute sur `http://0.0.0.0:8080`. Vérification :

```bash
curl http://127.0.0.1:8080/health
# {"status":"ok","service":"rustpocket-server","version":"0.1.0"}
```

Toutes les variables sont documentées dans `server/.env.example`.

### 2. Application

```bash
cd app
flutter pub get
flutter run
```

L'adresse du serveur se règle depuis l'écran de connexion (section « Serveur »)
ou dans les paramètres :

- émulateur Android : `http://10.0.2.2:8080` (valeur par défaut) ;
- téléphone réel : `http://IP-DU-PC:8080`, le téléphone et le PC sur le même
  Wi-Fi ;
- iOS / web : `http://127.0.0.1:8080` par défaut.

On peut aussi figer l'adresse au build :

```bash
flutter run --dart-define=RUSTPOCKET_API_URL=http://192.168.1.20:8080
```

Créez ensuite un compte depuis l'application (mot de passe : 8 caractères
minimum). Le code et les projets restent sur l'appareil.

## API

| Méthode | Route | Auth | Rôle |
| --- | --- | --- | --- |
| GET | `/health` | non | État du service |
| POST | `/auth/register` | non | Créer un compte, renvoie un JWT |
| POST | `/auth/login` | non | Connexion, renvoie un JWT |
| GET | `/me` | oui | Compte, quotas, limites |
| POST | `/run` | oui | Compile et exécute, renvoie stdout/stderr |
| POST | `/compile` | oui | Compile seulement |
| POST | `/check` | oui | Vérifie sans produire de binaire |
| POST | `/format` | oui | Applique rustfmt |

Exemple :

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"moi@example.com","password":"motdepasse"}' \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')

curl -s -X POST http://127.0.0.1:8080/run \
  -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"code":"fn main() { println!(\"bonjour\"); }"}'
```

Le format du code est volontairement limité à un fichier `main.rs` en V1 : pas
de `Cargo.toml`, donc pas de crates externes.

## Sécurité et bac à sable

Le serveur exécute du code inconnu ; deux modes existent.

**Docker (production, exigé par le cahier des charges)**

- un conteneur par compilation/exécution, détruit après usage ;
- `--network none`, toutes les capacités retirées, `no-new-privileges` ;
- mémoire, CPU, nombre de processus et taille de `/tmp` limités ;
- utilisateur non privilégié (uid 65534) ;
- dossier de travail temporaire supprimé après l'appel ;
- délai de 10 s et sortie plafonnée à 64 Ko côté serveur.

**Local (développement uniquement)**

Exécute `rustc` et le binaire directement sur la machine, avec `rlimits`
(mémoire, CPU, taille de fichier, descripteurs), groupe de processus tué au
délai, sortie plafonnée et environnement vidé pour le programme.

> **Limite importante** : le mode local n'isole pas le système de fichiers.
> Un programme peut lire des fichiers du serveur (par exemple avec
> `include_str!`) et, si `unshare -n` n'est pas autorisé sur la machine, il peut
> accéder au réseau. Le critère d'acceptation n° 6 (« aucun accès au réseau ni
> aux fichiers du serveur ») n'est donc rempli **qu'en mode Docker**. En
> production, utilisez `RUSTPOCKET_SANDBOX=docker`.

Autres points :

- mots de passe Argon2, jetons JWT HS256, secret à changer impérativement ;
- quotas en mémoire (20 exécutions/minute, 300/jour par défaut) ;
- mettez un reverse proxy HTTPS (Caddy, Nginx) devant l'API ;
- les projets ne sont pas encore envoyés au serveur.

## Docker

```bash
# Image du sandbox
docker build -t rustpocket-sandbox:latest sandbox

# Serveur + socket Docker
RUSTPOCKET_JWT_SECRET=un-secret-long docker compose up --build
```

Le serveur monte `/var/run/docker.sock` pour lancer les conteneurs : il faut en
être conscient sur une machine partagée. En usage personnel, c'est le compromis
retenu par le cahier des charges.

Chaque exécution monte son dossier de travail dans le conteneur. Lancé sur
l'hôte, le serveur utilise un *bind mount* de `/tmp` ; lancé dans un conteneur
(cas de `docker compose`), il écrit dans le volume nommé `rustpocket-work`
(`RUSTPOCKET_WORK_DIR=/work`, `RUSTPOCKET_WORK_VOLUME=rustpocket-work`), sinon le
démon Docker chercherait le chemin dans son propre système de fichiers.

### Vérifier que le sandbox est réellement actif

```bash
# 1. le serveur doit annoncer le mode docker dans ses logs
#    sandbox=Docker
docker compose logs server | head

# 2. un run normal doit créer puis détruire un conteneur
curl -s -X POST localhost:8080/run -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"code":"fn main(){println!(\"ok\");}"}'
# {"success":true,...,"stdout":"ok\n"}

# 3. aucune fuite réseau : ce programme doit échouer
docker run --rm --network none rustpocket-sandbox:latest \
  sh -c 'getent hosts example.com || echo RESEAU-COUPE'
# RESEAU-COUPE

# 4. aucun conteneur ne doit rester derrière un appel
docker ps --filter ancestor=rustpocket-sandbox:latest
```

## Tests et vérifications

```bash
cd server && cargo test            # 4 tests, dont compilation/exécution réelle
cd app && flutter analyze && flutter test
```

Mesures effectuées sur la machine de développement, mode local : compilation +
exécution d'un Hello World en **~90 ms**, diagnostic d'erreur avec ligne et
colonne, boucle infinie arrêtée au délai de 1 s dans le test.

L'application a été vérifiée par `flutter analyze`, `flutter test` et
`flutter build web`. Elle n'a pas été lancée sur un Android ou un iPhone réel
dans cet environnement, et les images Docker n'ont pas pu être construites ici
(Docker absent).

## Critères d'acceptation (MVP)

| # | Critère | État |
| --- | --- | --- |
| 1 | `fn main` + `println!` exécuté en moins de 5 s | ✅ ~90 ms mesurés |
| 2 | Erreur de compilation avec ligne et message | ✅ |
| 3 | Format reformate le code | ✅ rustfmt |
| 4 | Boucle infinie arrêtée au délai | ✅ test automatique |
| 5 | Fonctionne sur Android réel et iPhone/simulateur | ⚠️ compile, non testé sur appareil ici |
| 6 | Aucun accès réseau/fichiers du serveur | ✅ en mode Docker, ❌ en mode local |
| 7 | Exécution hors-ligne en mode avion | ❌ V3, pack hors-ligne non inclus |
| 8 | Logo et couleurs Rust | ✅ |

## Hors périmètre du MVP

Conformément au cahier des charges, restent en V2/V3 : projets multi-fichiers et
`Cargo.toml`, crates, Clippy, `cargo test`, LSP/autocomplétion, clavier de
symboles, entrée standard, cours et exercices, pack hors-ligne (toolchain
Android / WebAssembly iOS), extensions, partage et synchronisation.

## Pistes immédiates

1. Lancer l'app sur un Android réel et un simulateur iOS, corriger les détails
   de clavier et de mise en page.
2. Construire et tester les images Docker, puis durcir le sandbox (seccomp,
   gVisor).
3. Préparer la V2 : `Cargo.toml`, crates en liste blanche, multi-fichiers,
   Clippy et tests.
# RustPocket

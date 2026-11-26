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

## Démarrage

Prérequis : Rust stable avec `rustfmt`, Flutter 3.x, et Docker pour le mode
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

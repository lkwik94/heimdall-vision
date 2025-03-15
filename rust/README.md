# Heimdall Vision - Système d'inspection visuelle de bouteilles en temps réel

Ce projet implémente un système d'inspection visuelle de bouteilles en temps réel utilisant Rust pour les composants critiques de performance.

## Architecture

Le projet est organisé en plusieurs crates Rust:

- **heimdall-core**: Bibliothèque principale de traitement d'images et de détection
- **heimdall-camera**: Interface avec les caméras GigE Vision
- **heimdall-rt**: Composants temps réel pour l'ordonnancement et la synchronisation
- **heimdall-ipc**: Communication inter-processus
- **heimdall-server**: Serveur de traitement
- **heimdall-cli**: Interface en ligne de commande
- **heimdall-py**: Bindings Python

## Prérequis

- Rust 1.70+ (stable)
- OpenCV 4.x
- Aravis 0.8+ (pour les caméras GigE Vision)
- Bibliothèques de développement (voir le script d'installation)

## Installation

Utilisez le script d'installation fourni pour configurer l'environnement de développement:

```bash
./setup_dev_env.sh
```

Ce script installe:
- Rust et les outils de développement
- Les bibliothèques natives requises
- Configure les permissions pour les caméras GigE
- Configure le système pour les performances temps réel

## Compilation

```bash
# Compiler tous les composants
cargo build --workspace

# Compiler en mode release
cargo build --release --workspace

# Compiler un composant spécifique
cargo build --package heimdall-core
```

## Exécution

```bash
# Exécuter le serveur
cargo run --package heimdall-server

# Exécuter l'interface CLI
cargo run --package heimdall-cli
```

## Tests

```bash
# Exécuter tous les tests
cargo test-all

# Exécuter les tests avec rapport de couverture
cargo test-coverage

# Exécuter les benchmarks
cargo bench-all
```

## Docker

Un Dockerfile est fourni pour faciliter le déploiement:

```bash
# Construire l'image Docker
docker build -t heimdall-vision .

# Exécuter le conteneur
docker run --privileged -p 8080:8080 -p 9090:9090 heimdall-vision
```

## Optimisations de performance

Le projet utilise plusieurs optimisations pour atteindre des performances temps réel:

- Vectorisation SIMD
- Parallélisme avec Rayon
- Primitives de synchronisation sans verrou
- Ordonnancement temps réel
- Optimisations de compilation

## Licence

Ce projet est sous licence MIT. Voir le fichier LICENSE pour plus de détails.
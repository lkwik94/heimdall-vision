# Guide d'installation rapide - Système d'inspection visuelle temps réel

Ce guide résume les étapes essentielles pour configurer l'environnement de développement Rust pour le projet Heimdall Vision.

## Prérequis

- Système Linux (Ubuntu/Debian recommandé)
- Permissions administrateur

## Installation en une commande

```bash
./setup_dev_env.sh
```

## Dépendances principales

- Rust 1.74+
- OpenCV 4.8.0
- Aravis (pour caméras GigE)
- Bibliothèques temps réel

## Structure du projet

- `heimdall-core`: Traitement d'images
- `heimdall-camera`: Interface caméras
- `heimdall-rt`: Composants temps réel
- `heimdall-ipc`: Communication inter-processus
- `heimdall-server`: Serveur de traitement
- `heimdall-cli`: Interface en ligne de commande
- `heimdall-py`: Bindings Python

## Compilation

```bash
cargo build --release --workspace
```

## Tests

```bash
cargo test --workspace
```

## Benchmarks

```bash
cargo bench --workspace
```

Pour plus de détails, consultez le fichier ENVIRONMENT_SETUP.md.
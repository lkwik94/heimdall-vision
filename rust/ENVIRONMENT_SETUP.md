# Guide de configuration d'un environnement de développement Rust pour l'inspection visuelle de bouteilles en temps réel

Ce guide détaille la configuration complète d'un environnement de développement Rust optimisé pour le projet Heimdall Vision, un système d'inspection visuelle de bouteilles en temps réel.

## Table des matières

1. [Structure du projet](#structure-du-projet)
2. [Dépendances Rust](#dépendances-rust)
3. [Configuration de compilation optimale](#configuration-de-compilation-optimale)
4. [Installation des bibliothèques natives](#installation-des-bibliothèques-natives)
5. [Outils de test et profilage](#outils-de-test-et-profilage)
6. [Intégration CI/CD](#intégration-cicd)
7. [Configuration de l'environnement de développement](#configuration-de-lenvironnement-de-développement)

## Structure du projet

La structure recommandée pour le projet Heimdall Vision est la suivante:

```
heimdall-vision/
├── rust/
│   ├── Cargo.toml                 # Workspace Cargo principal
│   ├── heimdall-core/             # Bibliothèque principale de traitement d'images
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs             # Point d'entrée de la bibliothèque
│   │       ├── acquisition.rs     # Module d'acquisition d'images
│   │       ├── processing.rs      # Module de traitement d'images
│   │       └── detection.rs       # Module de détection de défauts
│   ├── heimdall-camera/           # Interface avec les caméras GigE
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── aravis.rs          # Wrapper pour Aravis (caméras GigE)
│   │       └── simulator.rs       # Simulateur de caméra pour tests
│   ├── heimdall-rt/               # Composants temps réel
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── scheduler.rs       # Ordonnanceur temps réel
│   │       └── sync.rs            # Primitives de synchronisation
│   ├── heimdall-ipc/              # Communication inter-processus
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── shared_memory.rs   # Mémoire partagée
│   │       └── messaging.rs       # Système de messagerie
│   ├── heimdall-server/           # Serveur de traitement
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs            # Point d'entrée du serveur
│   ├── heimdall-cli/              # Interface en ligne de commande
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs            # Point d'entrée CLI
│   └── heimdall-py/               # Bindings Python
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs             # Bindings Python via PyO3
├── benches/                       # Tests de performance
│   ├── acquisition_bench.rs
│   ├── processing_bench.rs
│   └── detection_bench.rs
├── tests/                         # Tests d'intégration
│   ├── test_images/               # Images de test
│   ├── acquisition_test.rs
│   ├── processing_test.rs
│   └── detection_test.rs
└── examples/                      # Exemples d'utilisation
    ├── basic_pipeline.rs
    ├── camera_capture.rs
    └── realtime_detection.rs
```

## Dépendances Rust

Voici les dépendances Rust recommandées avec leurs versions exactes pour chaque composant du système:

### Workspace Cargo principal (rust/Cargo.toml)

```toml
[workspace]
members = [
    "heimdall-core",
    "heimdall-camera",
    "heimdall-rt",
    "heimdall-ipc",
    "heimdall-server",
    "heimdall-cli",
    "heimdall-py"
]

[profile.dev]
opt-level = 1      # Optimisation de base pour le développement

[profile.release]
opt-level = 3      # Optimisation maximale
lto = "fat"        # Link-time optimization complète
codegen-units = 1  # Optimisation maximale, compilation plus lente
panic = "abort"    # Réduire la taille du binaire en cas de panique
strip = true       # Supprimer les symboles de débogage
debug = false      # Pas d'informations de débogage

[profile.bench]
opt-level = 3
lto = "fat"
codegen-units = 1
debug = true       # Garder les informations de débogage pour le profilage
```

### Bibliothèque principale (heimdall-core/Cargo.toml)

```toml
[package]
name = "heimdall-core"
version = "0.1.0"
edition = "2021"
authors = ["Heimdall Systems Team"]
description = "High-performance core components for Heimdall Vision System"

[lib]
name = "heimdall_core"
crate-type = ["cdylib", "rlib"]

[dependencies]
# Traitement d'images
image = "0.24.7"
ndarray = { version = "0.15.6", features = ["rayon"] }
opencv = { version = "0.84.5", features = ["opencv-4", "contrib", "buildtime-bindgen"] }

# Parallélisme et performance
rayon = "1.8.0"
crossbeam = "0.8.2"
parking_lot = "0.12.1"

# Gestion des erreurs et logging
thiserror = "1.0.50"
log = "0.4.20"
env_logger = "0.10.1"
tracing = "0.1.40"
tracing-subscriber = { version = "0.3.18", features = ["env-filter"] }

# Sérialisation
serde = { version = "1.0.193", features = ["derive"] }
serde_json = "1.0.108"

# Python bindings (optionnel)
pyo3 = { version = "0.19.0", features = ["extension-module"], optional = true }
numpy = { version = "0.19.0", optional = true }

[dev-dependencies]
criterion = "0.5.1"
proptest = "1.3.1"
mockall = "0.11.4"

[features]
default = []
python = ["pyo3", "numpy"]
simd = ["ndarray/simd"]
```

### Interface caméras GigE (heimdall-camera/Cargo.toml)

```toml
[package]
name = "heimdall-camera"
version = "0.1.0"
edition = "2021"
authors = ["Heimdall Systems Team"]
description = "Camera interface for Heimdall Vision System"

[dependencies]
# Dépendances de base
heimdall-core = { path = "../heimdall-core" }
thiserror = "1.0.50"
log = "0.4.20"
tracing = "0.1.40"

# Interface avec Aravis (GigE Vision)
aravis-rs = "0.6.3"
aravis-sys = "0.6.3"

# Gestion asynchrone
tokio = { version = "1.34.0", features = ["full"] }
async-trait = "0.1.74"

# Sérialisation
serde = { version = "1.0.193", features = ["derive"] }

# Gestion de la configuration
config = "0.13.3"

[dev-dependencies]
mockall = "0.11.4"
tokio-test = "0.4.3"
```

### Composants temps réel (heimdall-rt/Cargo.toml)

```toml
[package]
name = "heimdall-rt"
version = "0.1.0"
edition = "2021"
authors = ["Heimdall Systems Team"]
description = "Real-time components for Heimdall Vision System"

[dependencies]
# Dépendances de base
heimdall-core = { path = "../heimdall-core" }
thiserror = "1.0.50"
log = "0.4.20"
tracing = "0.1.40"

# Primitives temps réel
crossbeam = "0.8.2"
parking_lot = "0.12.1"
lockfree = "0.5.1"

# Gestion asynchrone temps réel
tokio = { version = "1.34.0", features = ["full", "rt-multi-thread"] }
async-io = "1.13.0"
futures = "0.3.29"

# Ordonnancement temps réel (Linux uniquement)
libc = "0.2.150"
nix = { version = "0.27.1", features = ["process", "sched"] }

# Alternative: RTIC pour les systèmes embarqués
# rtic = "2.0.1"

[target.'cfg(target_os = "linux")'.dependencies]
# Priorités temps réel Linux
rtkit = "0.0.5"

[dev-dependencies]
criterion = "0.5.1"
```

### Communication inter-processus (heimdall-ipc/Cargo.toml)

```toml
[package]
name = "heimdall-ipc"
version = "0.1.0"
edition = "2021"
authors = ["Heimdall Systems Team"]
description = "Inter-process communication for Heimdall Vision System"

[dependencies]
# Dépendances de base
heimdall-core = { path = "../heimdall-core" }
thiserror = "1.0.50"
log = "0.4.20"
tracing = "0.1.40"

# Mémoire partagée
shared_memory = "0.12.4"
memmap2 = "0.7.1"

# Communication par messages
ipc-channel = "0.16.1"
zmq = "0.10.0"
tokio-zmq = "0.12.0"

# Sérialisation
serde = { version = "1.0.193", features = ["derive"] }
bincode = "1.3.3"

# Gestion asynchrone
tokio = { version = "1.34.0", features = ["full"] }
async-trait = "0.1.74"

[dev-dependencies]
tempfile = "3.8.1"
```

### Serveur de traitement (heimdall-server/Cargo.toml)

```toml
[package]
name = "heimdall-server"
version = "0.1.0"
edition = "2021"
authors = ["Heimdall Systems Team"]
description = "Processing server for Heimdall Vision System"

[dependencies]
# Composants Heimdall
heimdall-core = { path = "../heimdall-core" }
heimdall-camera = { path = "../heimdall-camera" }
heimdall-rt = { path = "../heimdall-rt" }
heimdall-ipc = { path = "../heimdall-ipc" }

# Gestion asynchrone
tokio = { version = "1.34.0", features = ["full"] }
async-trait = "0.1.74"

# API Web
axum = "0.6.20"
tower = "0.4.13"
tower-http = { version = "0.4.4", features = ["cors", "trace"] }

# Sérialisation
serde = { version = "1.0.193", features = ["derive"] }
serde_json = "1.0.108"

# Logging et métriques
tracing = "0.1.40"
tracing-subscriber = { version = "0.3.18", features = ["env-filter"] }
metrics = "0.21.1"
metrics-exporter-prometheus = "0.12.1"

# Configuration
config = "0.13.3"
clap = { version = "4.4.8", features = ["derive"] }

[dev-dependencies]
reqwest = { version = "0.11.22", features = ["json"] }
```

### Interface en ligne de commande (heimdall-cli/Cargo.toml)

```toml
[package]
name = "heimdall-cli"
version = "0.1.0"
edition = "2021"
authors = ["Heimdall Systems Team"]
description = "Command-line interface for Heimdall Vision System"

[dependencies]
# Composants Heimdall
heimdall-core = { path = "../heimdall-core" }
heimdall-camera = { path = "../heimdall-camera" }
heimdall-ipc = { path = "../heimdall-ipc" }

# Interface CLI
clap = { version = "4.4.8", features = ["derive"] }
dialoguer = "0.10.4"
indicatif = "0.17.7"
console = "0.15.7"

# Sérialisation
serde = { version = "1.0.193", features = ["derive"] }
serde_json = "1.0.108"

# Logging
tracing = "0.1.40"
tracing-subscriber = { version = "0.3.18", features = ["env-filter"] }

# Client HTTP
reqwest = { version = "0.11.22", features = ["json", "blocking"] }
```

### Bindings Python (heimdall-py/Cargo.toml)

```toml
[package]
name = "heimdall-py"
version = "0.1.0"
edition = "2021"
authors = ["Heimdall Systems Team"]
description = "Python bindings for Heimdall Vision System"

[lib]
name = "heimdall"
crate-type = ["cdylib"]

[dependencies]
# Composants Heimdall
heimdall-core = { path = "../heimdall-core", features = ["python"] }
heimdall-camera = { path = "../heimdall-camera" }

# Python bindings
pyo3 = { version = "0.19.0", features = ["extension-module"] }
numpy = "0.19.0"

# Logging
tracing = "0.1.40"
```

## Configuration de compilation optimale

Pour obtenir des performances optimales pour un système temps réel, voici les configurations recommandées:

### Flags de compilation pour la performance temps réel

Ajoutez ces flags dans un fichier `.cargo/config.toml` à la racine du projet:

```toml
[build]
rustflags = [
    # Optimisations LLVM
    "-C", "target-cpu=native",
    "-C", "opt-level=3",
    
    # Vectorisation SIMD
    "-C", "target-feature=+avx,+avx2,+fma,+sse,+sse2,+sse3,+sse4.1,+sse4.2",
    
    # Optimisations de link-time
    "-C", "lto=fat",
    "-C", "codegen-units=1",
    
    # Optimisations pour les systèmes temps réel
    "-C", "force-frame-pointers=yes",
    
    # Désactiver les vérifications de débordement en release
    "-C", "overflow-checks=no",
]

[target.'cfg(target_os = "linux")']
rustflags = [
    # Flags spécifiques à Linux
    "-C", "link-arg=-Wl,--as-needed",
]

[target.'cfg(target_os = "windows")']
rustflags = [
    # Flags spécifiques à Windows
    "-C", "link-arg=/LTCG",
]
```

### Variables d'environnement pour la compilation

```bash
# Utiliser LLD comme linker (plus rapide)
export RUSTFLAGS="-C link-arg=-fuse-ld=lld"

# Activer la compilation incrémentale
export CARGO_INCREMENTAL=1

# Utiliser tous les cœurs pour la compilation
export CARGO_BUILD_JOBS=$(nproc)
```

## Installation des bibliothèques natives

### OpenCV

#### Debian/Ubuntu

```bash
# Installer les dépendances de développement
sudo apt update
sudo apt install -y build-essential cmake pkg-config

# Installer OpenCV avec les modules supplémentaires
sudo apt install -y libopencv-dev python3-opencv

# Vérifier l'installation
pkg-config --modversion opencv4
```

#### Compilation depuis les sources (pour des performances optimales)

```bash
# Installer les dépendances
sudo apt update
sudo apt install -y build-essential cmake pkg-config \
    libgtk-3-dev libavcodec-dev libavformat-dev libswscale-dev \
    libv4l-dev libxvidcore-dev libx264-dev libjpeg-dev \
    libpng-dev libtiff-dev gfortran openexr libatlas-base-dev \
    python3-dev python3-numpy libtbb2 libtbb-dev libdc1394-22-dev

# Télécharger OpenCV
mkdir -p ~/opencv_build && cd ~/opencv_build
git clone --depth 1 --branch 4.8.0 https://github.com/opencv/opencv.git
git clone --depth 1 --branch 4.8.0 https://github.com/opencv/opencv_contrib.git

# Configurer la compilation
cd ~/opencv_build/opencv
mkdir -p build && cd build
cmake -D CMAKE_BUILD_TYPE=RELEASE \
    -D CMAKE_INSTALL_PREFIX=/usr/local \
    -D OPENCV_EXTRA_MODULES_PATH=~/opencv_build/opencv_contrib/modules \
    -D ENABLE_NEON=ON \
    -D WITH_TBB=ON \
    -D WITH_V4L=ON \
    -D WITH_QT=OFF \
    -D WITH_OPENGL=ON \
    -D WITH_CUDA=ON \
    -D BUILD_TIFF=ON \
    -D WITH_FFMPEG=ON \
    -D WITH_GSTREAMER=ON \
    -D WITH_GTK=ON \
    -D BUILD_TESTS=OFF \
    -D BUILD_PERF_TESTS=OFF \
    -D BUILD_EXAMPLES=OFF \
    -D OPENCV_ENABLE_NONFREE=ON \
    -D OPENCV_GENERATE_PKGCONFIG=ON ..

# Compiler et installer
make -j$(nproc)
sudo make install
sudo ldconfig

# Vérifier l'installation
pkg-config --modversion opencv4
```

### Aravis (pour les caméras GigE Vision)

#### Debian/Ubuntu

```bash
# Installer les dépendances
sudo apt update
sudo apt install -y libglib2.0-dev libxml2-dev libusb-1.0-0-dev \
    gobject-introspection libgirepository1.0-dev \
    gtk-doc-tools libgtk-3-dev libnotify-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev

# Installer Aravis
sudo apt install -y libaravis-dev gstreamer1.0-plugins-good

# Vérifier l'installation
pkg-config --modversion aravis-0.8
```

#### Compilation depuis les sources (version récente)

```bash
# Installer les dépendances
sudo apt update
sudo apt install -y build-essential meson ninja-build \
    libglib2.0-dev libxml2-dev libusb-1.0-0-dev \
    gobject-introspection libgirepository1.0-dev \
    gtk-doc-tools libgtk-3-dev libnotify-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev

# Télécharger Aravis
git clone https://github.com/AravisProject/aravis.git
cd aravis

# Configurer et compiler
meson build --prefix=/usr/local
cd build
ninja
sudo ninja install
sudo ldconfig

# Vérifier l'installation
pkg-config --modversion aravis-0.8
```

### Configuration des permissions pour les caméras GigE

```bash
# Créer un fichier de règles udev pour les caméras GigE
sudo tee /etc/udev/rules.d/40-aravis.rules > /dev/null << 'EOT'
# Aravis GigE Vision devices
SUBSYSTEM=="usb", ATTRS{idVendor}=="1ab2", MODE="0666"
# GigE Vision ethernet devices
SUBSYSTEM=="net", ACTION=="add", ATTR{address}=="aa:bb:cc:*", RUN+="/sbin/ip link set %k mtu 9000"
EOT

# Recharger les règles udev
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### Configuration du système pour les performances temps réel

```bash
# Ajouter l'utilisateur au groupe realtime
sudo groupadd -f realtime
sudo usermod -aG realtime $USER

# Configurer les limites de ressources pour le groupe realtime
sudo tee /etc/security/limits.d/99-realtime.conf > /dev/null << 'EOT'
@realtime soft rtprio 99
@realtime hard rtprio 99
@realtime soft memlock unlimited
@realtime hard memlock unlimited
@realtime soft nice -20
@realtime hard nice -20
EOT

# Configurer le noyau pour les performances temps réel
sudo tee /etc/sysctl.d/99-realtime.conf > /dev/null << 'EOT'
# Priorité temps réel
kernel.sched_rt_runtime_us = 980000
# Augmenter la taille maximale de la mémoire partagée
kernel.shmmax = 8589934592
kernel.shmall = 8589934592
# Désactiver le swap pour les processus temps réel
vm.swappiness = 10
EOT

# Appliquer les changements
sudo sysctl -p /etc/sysctl.d/99-realtime.conf
```

## Outils de test et profilage

### Configuration des tests unitaires et d'intégration

Créez un fichier `.cargo/config.toml` à la racine du projet:

```toml
[alias]
# Exécuter tous les tests
test-all = "test --workspace --all-features"

# Exécuter les tests avec le rapport de couverture
test-coverage = "llvm-cov --workspace --all-features --lcov --output-path lcov.info"

# Exécuter les benchmarks
bench-all = "bench --workspace"
```

### Configuration des benchmarks avec Criterion

Créez un fichier `benches/bench_main.rs`:

```rust
use criterion::{criterion_group, criterion_main, Criterion};

// Importer les fonctions de benchmark
mod acquisition_bench;
mod processing_bench;
mod detection_bench;

fn bench_acquisition(c: &mut Criterion) {
    acquisition_bench::bench_camera_acquisition(c);
}

fn bench_processing(c: &mut Criterion) {
    processing_bench::bench_image_processing(c);
}

fn bench_detection(c: &mut Criterion) {
    detection_bench::bench_contamination_detection(c);
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(std::time::Duration::from_secs(5));
    targets = bench_acquisition, bench_processing, bench_detection
);
criterion_main!(benches);
```

### Outils de profilage

#### Flamegraph

```bash
# Installer cargo-flamegraph
cargo install flamegraph

# Exécuter avec flamegraph
cargo flamegraph --bin heimdall-server

# Pour les systèmes qui nécessitent des privilèges
sudo cargo flamegraph --bin heimdall-server
```

#### Valgrind/Callgrind

```bash
# Installer Valgrind
sudo apt install -y valgrind kcachegrind

# Profiler avec Callgrind
valgrind --tool=callgrind --callgrind-out-file=callgrind.out target/release/heimdall-server

# Visualiser les résultats
kcachegrind callgrind.out
```

#### Perf (Linux)

```bash
# Installer perf
sudo apt install -y linux-tools-common linux-tools-generic linux-tools-`uname -r`

# Profiler avec perf
perf record -g target/release/heimdall-server

# Analyser les résultats
perf report
```

## Intégration CI/CD

### Configuration GitHub Actions

Créez un fichier `.github/workflows/rust.yml`:

```yaml
name: Rust CI/CD

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    
    - name: Install dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libopencv-dev libaravis-dev
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        profile: minimal
        toolchain: stable
        override: true
        components: rustfmt, clippy
    
    - name: Cache dependencies
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Check formatting
      run: cargo fmt --all -- --check
    
    - name: Lint with clippy
      run: cargo clippy --all-targets --all-features -- -D warnings
    
    - name: Build
      run: cargo build --verbose --workspace
    
    - name: Run tests
      run: cargo test --verbose --workspace
    
    - name: Run benchmarks
      run: cargo bench --verbose --workspace
    
  release:
    needs: build
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    
    - name: Install dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libopencv-dev libaravis-dev
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        profile: minimal
        toolchain: stable
        override: true
    
    - name: Build release
      run: cargo build --release --verbose --workspace
    
    - name: Create release artifacts
      run: |
        mkdir -p artifacts
        cp target/release/heimdall-server artifacts/
        cp target/release/heimdall-cli artifacts/
        cp target/release/libheimdall.so artifacts/
        tar -czvf heimdall-vision-release.tar.gz artifacts/
    
    - name: Upload artifacts
      uses: actions/upload-artifact@v3
      with:
        name: heimdall-vision-release
        path: heimdall-vision-release.tar.gz
```

### Configuration Docker

Créez un fichier `Dockerfile`:

```dockerfile
FROM rust:1.74-slim-bullseye as builder

# Installer les dépendances
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libopencv-dev \
    libaravis-dev \
    libglib2.0-dev \
    libusb-1.0-0-dev \
    && rm -rf /var/lib/apt/lists/*

# Créer un utilisateur non-root
RUN useradd -m -u 1000 -U -s /bin/bash heimdall

# Copier le code source
WORKDIR /heimdall
COPY --chown=heimdall:heimdall . .

# Compiler en mode release
USER heimdall
RUN cargo build --release --workspace

# Image finale
FROM debian:bullseye-slim

# Installer les dépendances runtime
RUN apt-get update && apt-get install -y \
    libopencv-dev \
    libaravis-0.8-0 \
    libglib2.0-0 \
    libusb-1.0-0 \
    && rm -rf /var/lib/apt/lists/*

# Créer un utilisateur non-root
RUN useradd -m -u 1000 -U -s /bin/bash heimdall

# Copier les binaires compilés
COPY --from=builder --chown=heimdall:heimdall /heimdall/target/release/heimdall-server /usr/local/bin/
COPY --from=builder --chown=heimdall:heimdall /heimdall/target/release/heimdall-cli /usr/local/bin/
COPY --from=builder --chown=heimdall:heimdall /heimdall/target/release/libheimdall.so /usr/local/lib/

# Configurer les permissions pour les caméras GigE
RUN echo 'SUBSYSTEM=="usb", ATTRS{idVendor}=="1ab2", MODE="0666"' > /etc/udev/rules.d/40-aravis.rules

# Configurer l'environnement
ENV LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH

# Définir le répertoire de travail
WORKDIR /heimdall
USER heimdall

# Exposer les ports
EXPOSE 8080 9090

# Point d'entrée
ENTRYPOINT ["heimdall-server"]
```

## Configuration de l'environnement de développement

### VSCode

Créez un fichier `.vscode/settings.json`:

```json
{
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.checkOnSave.extraArgs": ["--all-features"],
    "rust-analyzer.cargo.allFeatures": true,
    "rust-analyzer.procMacro.enable": true,
    "rust-analyzer.inlayHints.enable": true,
    "editor.formatOnSave": true,
    "editor.rulers": [100],
    "files.insertFinalNewline": true,
    "files.trimTrailingWhitespace": true
}
```

Créez un fichier `.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug heimdall-server",
            "cargo": {
                "args": [
                    "build",
                    "--bin=heimdall-server",
                    "--package=heimdall-server"
                ],
                "filter": {
                    "name": "heimdall-server",
                    "kind": "bin"
                }
            },
            "args": [],
            "cwd": "${workspaceFolder}"
        },
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug heimdall-cli",
            "cargo": {
                "args": [
                    "build",
                    "--bin=heimdall-cli",
                    "--package=heimdall-cli"
                ],
                "filter": {
                    "name": "heimdall-cli",
                    "kind": "bin"
                }
            },
            "args": [],
            "cwd": "${workspaceFolder}"
        },
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug unit tests",
            "cargo": {
                "args": [
                    "test",
                    "--no-run",
                    "--workspace"
                ],
                "filter": {
                    "kind": "lib"
                }
            },
            "args": [],
            "cwd": "${workspaceFolder}"
        }
    ]
}
```

### Script d'installation de l'environnement de développement

Créez un script `setup_dev_env.sh`:

```bash
#!/bin/bash
set -e

echo "Configuration de l'environnement de développement Heimdall Vision..."

# Installer Rust
if ! command -v rustc &> /dev/null; then
    echo "Installation de Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Installer les composants Rust
rustup component add rustfmt clippy
rustup update

# Installer les outils de développement
cargo install cargo-edit cargo-watch cargo-expand cargo-llvm-cov cargo-criterion cargo-flamegraph

# Installer les dépendances système
if [ "$(uname)" == "Linux" ]; then
    if command -v apt-get &> /dev/null; then
        echo "Installation des dépendances sur Debian/Ubuntu..."
        sudo apt-get update
        sudo apt-get install -y build-essential cmake pkg-config \
            libopencv-dev libaravis-dev libglib2.0-dev libusb-1.0-0-dev \
            libgtk-3-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
            valgrind kcachegrind linux-tools-common linux-tools-generic
    elif command -v dnf &> /dev/null; then
        echo "Installation des dépendances sur Fedora..."
        sudo dnf install -y gcc gcc-c++ cmake pkgconfig \
            opencv-devel aravis-devel glib2-devel libusb-devel \
            gtk3-devel gstreamer1-devel gstreamer1-plugins-base-devel \
            valgrind kcachegrind perf
    fi
elif [ "$(uname)" == "Darwin" ]; then
    echo "Installation des dépendances sur macOS..."
    brew install opencv aravis glib libusb gtk+3 gstreamer gst-plugins-base
fi

# Configurer les permissions pour les caméras GigE (Linux uniquement)
if [ "$(uname)" == "Linux" ]; then
    echo "Configuration des permissions pour les caméras GigE..."
    sudo groupadd -f realtime
    sudo usermod -aG realtime $USER
    
    sudo tee /etc/udev/rules.d/40-aravis.rules > /dev/null << 'EOT'
# Aravis GigE Vision devices
SUBSYSTEM=="usb", ATTRS{idVendor}=="1ab2", MODE="0666"
# GigE Vision ethernet devices
SUBSYSTEM=="net", ACTION=="add", ATTR{address}=="aa:bb:cc:*", RUN+="/sbin/ip link set %k mtu 9000"
EOT
    
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    
    sudo tee /etc/security/limits.d/99-realtime.conf > /dev/null << 'EOT'
@realtime soft rtprio 99
@realtime hard rtprio 99
@realtime soft memlock unlimited
@realtime hard memlock unlimited
@realtime soft nice -20
@realtime hard nice -20
EOT
    
    sudo tee /etc/sysctl.d/99-realtime.conf > /dev/null << 'EOT'
kernel.sched_rt_runtime_us = 980000
kernel.shmmax = 8589934592
kernel.shmall = 8589934592
vm.swappiness = 10
EOT
    
    sudo sysctl -p /etc/sysctl.d/99-realtime.conf
fi

# Créer la structure du projet
mkdir -p rust/{heimdall-core,heimdall-camera,heimdall-rt,heimdall-ipc,heimdall-server,heimdall-cli,heimdall-py}/{src,tests}
mkdir -p benches examples

echo "Configuration terminée! Veuillez vous déconnecter et vous reconnecter pour que les changements de groupe prennent effet."
```

Rendez le script exécutable:

```bash
chmod +x setup_dev_env.sh
```

---

Ce guide complet vous permet de configurer un environnement de développement Rust optimisé pour un projet d'inspection visuelle de bouteilles en temps réel. Il couvre tous les aspects nécessaires, de la structure du projet aux dépendances précises, en passant par les optimisations de compilation, l'installation des bibliothèques natives, les outils de test et profilage, et l'intégration CI/CD.
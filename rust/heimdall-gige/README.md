# Module d'acquisition d'images pour caméras GigE Vision

Ce module fournit une interface complète pour l'acquisition d'images à partir de caméras GigE Vision dans un contexte d'inspection de bouteilles à haute cadence.

## Caractéristiques

- Support pour caméras GigE Vision 2MP en niveaux de gris
- Acquisition synchronisée de 4 caméras avec latence < 5ms
- Mécanismes de synchronisation hardware/software
- Gestion robuste des erreurs et stratégies de reprise
- Optimisation des paramètres de caméra
- Métriques et diagnostics

## Architecture

Le module est organisé en plusieurs composants:

- **GigESystem**: Point d'entrée principal qui gère l'ensemble du système d'acquisition
- **GigECamera**: Représente une caméra GigE Vision individuelle
- **SyncManager**: Gère la synchronisation entre les caméras
- **Frame/FrameSet**: Structures de données pour représenter les images et métadonnées
- **Diagnostics**: Outils de diagnostic et surveillance

## Installation

Ajoutez la dépendance à votre fichier `Cargo.toml`:

```toml
[dependencies]
heimdall-gige = { path = "../heimdall-gige" }
```

## Prérequis

- Rust 1.56 ou supérieur
- Bibliothèque Aravis (pour GigE Vision)
- Jumbo Frames activés sur votre interface réseau (MTU 9000)

### Installation d'Aravis

#### Ubuntu/Debian

```bash
sudo apt-get install libaravis-dev
```

#### Fedora/CentOS

```bash
sudo dnf install aravis-devel
```

#### macOS

```bash
brew install aravis
```

## Exemple d'utilisation

```rust
use heimdall_gige::{GigESystem, SyncMode};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialiser le système GigE
    let mut gige = GigESystem::new()?;
    
    // Découvrir les caméras disponibles
    let cameras = gige.discover_cameras().await?;
    println!("Caméras découvertes: {:?}", cameras);
    
    // Configurer et initialiser les caméras
    gige.configure_cameras(SyncMode::Hardware).await?;
    
    // Démarrer l'acquisition
    gige.start_acquisition().await?;
    
    // Acquérir des images
    for _ in 0..10 {
        let frames = gige.acquire_frames().await?;
        println!("Images acquises: {}", frames.len());
        
        // Traiter les images...
    }
    
    // Arrêter l'acquisition
    gige.stop_acquisition().await?;
    
    Ok(())
}
```

## API

### GigESystem

```rust
// Création d'un nouveau système
let gige = GigESystem::new()?;

// Création avec configuration personnalisée
let config = SystemConfig { ... };
let gige = GigESystem::with_config(config)?;

// Découverte des caméras
let cameras = gige.discover_cameras().await?;

// Configuration des caméras
gige.configure_cameras(SyncMode::Hardware).await?;

// Démarrage de l'acquisition
gige.start_acquisition().await?;

// Acquisition d'images
let frames = gige.acquire_frames().await?;

// Optimisation des paramètres
gige.optimize_camera_parameters().await?;

// Diagnostic
let report = gige.run_diagnostics().await?;

// Arrêt de l'acquisition
gige.stop_acquisition().await?;
```

### Frame

```rust
// Accès aux données d'image
let data = &frame.data;
let width = frame.width;
let height = frame.height;

// Accès aux métadonnées
let camera_id = &frame.metadata.camera_id;
let timestamp = frame.metadata.timestamp;
let exposure = frame.metadata.exposure_time_us;

// Conversion en ndarray
let array = frame.to_ndarray2()?;

// Calcul de statistiques
let mean = frame.mean()?;
let std_dev = frame.std_dev()?;
let histogram = frame.histogram()?;

// Enregistrement de l'image
frame.save("image.png")?;
```

### SyncManager

```rust
// Configuration de la synchronisation
let mut sync_manager = SyncManager::new();
sync_manager.set_mode(SyncMode::Hardware);

// Démarrage de la synchronisation
sync_manager.start()?;

// Déclenchement
sync_manager.trigger()?;

// Obtention de l'état
let status = sync_manager.get_status();
```

## Performances

Le module est conçu pour des performances optimales:

- Latence d'acquisition < 5ms
- Support pour 4 caméras à 30 FPS
- Utilisation efficace de la mémoire et du CPU
- Gestion asynchrone pour éviter le blocage

## Stratégies de reprise

Le module implémente plusieurs stratégies de reprise en cas d'erreur:

- Reconnexion automatique en cas de perte de connexion
- Réinitialisation des caméras en cas de blocage
- Nouvelle tentative d'acquisition en cas d'erreur temporaire
- Journalisation détaillée pour le diagnostic

## Tests

Le module inclut des tests unitaires et d'intégration:

```bash
# Exécuter les tests unitaires
cargo test

# Exécuter les benchmarks
cargo bench
```

## Licence

Ce module est distribué sous licence MIT.

## Auteurs

Équipe Heimdall Vision
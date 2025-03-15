# Heimdall Lighting

Module Rust pour le contrôle de panneaux lumineux LED synchronisés avec l'acquisition d'images pour l'inspection de bouteilles en PET dans le système de vision Heimdall.

## Fonctionnalités

- **Contrôle précis de l'éclairage** : Interface unifiée pour différents types de contrôleurs d'éclairage (série, Ethernet, GPIO)
- **Synchronisation précise** : Synchronisation avec l'acquisition d'images avec un temps de réponse inférieur à 1ms
- **Ajustement automatique d'intensité** : Algorithmes avancés pour maintenir une intensité lumineuse optimale
- **Compensation des variations** : Détection et compensation des variations d'éclairage
- **Support multi-configurations** : Gestion de différentes configurations d'éclairage pour les différents postes d'inspection
- **Diagnostics avancés** : Surveillance en temps réel et alertes en cas de défaillance
- **Calibration automatique** : Procédures de calibration pour garantir la constance de l'éclairage

## Architecture

Le module est organisé en plusieurs composants :

### Contrôleurs d'éclairage

- `SerialLightingController` : Contrôle de l'éclairage via une interface série
- `EthernetLightingController` : Contrôle de l'éclairage via une interface réseau
- `GpioLightingController` : Contrôle de l'éclairage via les GPIO (Raspberry Pi)
- `SimulatedLightingController` : Contrôleur simulé pour les tests

### Synchronisation

- `LightingSynchronizer` : Synchronisation entre l'éclairage et l'acquisition d'images
- `CameraSynchronizer` : Synchronisation avec les caméras
- `ExternalSynchronizer` : Synchronisation avec des sources externes (encodeurs, capteurs)
- `HighPrecisionTimer` : Timer haute précision pour la synchronisation

### Ajustement d'intensité

- `AutoIntensityAdjuster` : Ajustement automatique de l'intensité lumineuse
- `AdvancedAutoIntensityAdjuster` : Ajustement avancé avec plusieurs algorithmes (PID, recherche binaire, gradient, histogramme)

### Calibration

- `UniformityCalibrator` : Calibration de l'uniformité de l'éclairage

### Diagnostics

- `LightingDiagnostics` : Diagnostics de l'éclairage
- `LightingMonitor` : Surveillance en temps réel
- `AlertManager` : Gestion des alertes

## Utilisation

### Installation

Ajoutez la dépendance à votre fichier `Cargo.toml` :

```toml
[dependencies]
heimdall-lighting = { path = "../heimdall-lighting" }
```

### Exemple de base

```rust
use heimdall_lighting::{
    LightingControllerFactory, LightingConfig, LightChannelConfig,
    SyncMode, LightingType
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Créer un contrôleur d'éclairage
    let mut controller = LightingControllerFactory::create("serial", "main_light")?;
    
    // Configurer le contrôleur
    let config = LightingConfig::default();
    controller.initialize(config).await?;
    
    // Activer un canal d'éclairage
    controller.turn_on("channel1").await?;
    
    // Régler l'intensité
    controller.set_intensity("channel1", 75.0).await?;
    
    // Utiliser le mode stroboscopique
    controller.strobe("channel1", 1000).await?;
    
    // Désactiver le canal
    controller.turn_off("channel1").await?;
    
    Ok(())
}
```

### Synchronisation avec une caméra

```rust
use heimdall_lighting::{
    LightingControllerFactory, LightingSynchronizer, SyncMode, SyncEvent
};
use heimdall_lighting::synchronization::camera_sync::{
    CameraSynchronizer, CameraSyncConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Créer un contrôleur d'éclairage
    let controller = LightingControllerFactory::create("serial", "main_light")?;
    
    // Créer un synchroniseur
    let sync_config = CameraSyncConfig::default();
    let mut synchronizer = CameraSynchronizer::new(controller, sync_config);
    
    // Démarrer la synchronisation
    synchronizer.start()?;
    
    // Déclencher l'acquisition
    synchronizer.trigger_camera()?;
    
    // Arrêter la synchronisation
    synchronizer.stop()?;
    
    Ok(())
}
```

### Ajustement automatique d'intensité

```rust
use heimdall_lighting::{
    LightingControllerFactory, AutoIntensityAdjuster
};
use heimdall_lighting::calibration::auto_intensity::{
    AdvancedAutoIntensityAdjuster, AutoIntensityConfig, IntensityAlgorithm
};
use ndarray::Array3;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Créer un contrôleur d'éclairage
    let controller = LightingControllerFactory::create("serial", "main_light")?;
    
    // Créer un ajusteur d'intensité
    let config = AutoIntensityConfig {
        algorithm: IntensityAlgorithm::PID,
        target_intensity: 128.0,
        tolerance: 5.0,
        adjustment_step: 2.0,
        min_intensity: 10.0,
        max_intensity: 100.0,
        roi: Some((100, 100, 200, 200)),
        pid_params: Some((0.5, 0.1, 0.05)),
    };
    
    let mut adjuster = AdvancedAutoIntensityAdjuster::new(
        controller,
        "channel1".to_string(),
        config
    );
    
    // Acquérir une image
    let image = Array3::<u8>::zeros((480, 640, 3));
    
    // Ajuster l'intensité
    let new_intensity = adjuster.adjust(&image.view()).await?;
    println!("Nouvelle intensité: {}%", new_intensity);
    
    Ok(())
}
```

### Diagnostics et surveillance

```rust
use heimdall_lighting::{
    LightingControllerFactory, LightingDiagnostics
};
use heimdall_lighting::diagnostics::{
    monitoring::{LightingMonitor, MonitoringConfig},
    alerts::{AlertManager, AlertLevel, Alert}
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Créer un contrôleur d'éclairage
    let controller = LightingControllerFactory::create("serial", "main_light")?;
    
    // Créer un moniteur
    let config = MonitoringConfig::default();
    let mut monitor = LightingMonitor::new(controller, config);
    
    // Démarrer la surveillance
    monitor.start()?;
    
    // Ajouter un callback d'alerte
    monitor.add_alert_callback(|measurement| {
        println!("Alerte: intensité = {}%", measurement.mean_intensity);
    });
    
    // Exécuter un diagnostic
    let anomalies = monitor.detect_anomalies();
    for anomaly in anomalies {
        println!("Anomalie: {}", anomaly);
    }
    
    // Arrêter la surveillance
    monitor.stop()?;
    
    Ok(())
}
```

## Configuration pour différents types d'éclairage

### Éclairage diffus (dôme)

Idéal pour l'inspection des surfaces et la détection des défauts de surface.

```rust
let diffuse_config = LightChannelConfig {
    id: "diffuse".to_string(),
    lighting_type: LightingType::Diffuse,
    intensity: 70.0,
    duration_us: 1000,
    delay_us: 0,
    controller_params: HashMap::new(),
};
```

### Rétro-éclairage (backlight)

Parfait pour la détection de contours et la mesure dimensionnelle.

```rust
let backlight_config = LightChannelConfig {
    id: "backlight".to_string(),
    lighting_type: LightingType::Backlight,
    intensity: 90.0,
    duration_us: 1000,
    delay_us: 0,
    controller_params: HashMap::new(),
};
```

### Éclairage directionnel

Utilisé pour mettre en évidence les reliefs et les textures.

```rust
let directional_config = LightChannelConfig {
    id: "directional".to_string(),
    lighting_type: LightingType::Directional,
    intensity: 80.0,
    duration_us: 1000,
    delay_us: 0,
    controller_params: HashMap::new(),
};
```

### Éclairage coaxial

Idéal pour les surfaces réfléchissantes.

```rust
let coaxial_config = LightChannelConfig {
    id: "coaxial".to_string(),
    lighting_type: LightingType::Coaxial,
    intensity: 75.0,
    duration_us: 1000,
    delay_us: 0,
    controller_params: HashMap::new(),
};
```

### Éclairage structuré

Utilisé pour la reconstruction 3D et l'analyse de forme.

```rust
let structured_config = LightChannelConfig {
    id: "structured".to_string(),
    lighting_type: LightingType::Structured,
    intensity: 85.0,
    duration_us: 1000,
    delay_us: 0,
    controller_params: HashMap::new(),
};
```

## Procédures de calibration

### Calibration de l'uniformité

```rust
use heimdall_lighting::calibration::uniformity::{
    UniformityCalibrator, UniformityCalibrationConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Créer un contrôleur d'éclairage
    let controller = LightingControllerFactory::create("serial", "main_light")?;
    
    // Créer un calibrateur d'uniformité
    let config = UniformityCalibrationConfig::default();
    let mut calibrator = UniformityCalibrator::new(
        controller,
        "channel1".to_string(),
        config
    );
    
    // Fonction d'acquisition d'image
    let acquire_image = || {
        // Acquérir une image réelle ou simulée
        let image = Array3::<u8>::zeros((480, 640, 3));
        Ok(image)
    };
    
    // Calibrer l'uniformité
    let result = calibrator.calibrate(acquire_image).await?;
    println!("Uniformité globale: {}%", result.global_uniformity);
    
    Ok(())
}
```

## Licence

Ce module est distribué sous licence MIT.

## Auteurs

Équipe Heimdall Systems
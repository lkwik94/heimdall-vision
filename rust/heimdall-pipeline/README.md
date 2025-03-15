# Heimdall Pipeline

Module de pipeline d'acquisition d'images haute performance pour le système de vision Heimdall.

## Caractéristiques

- **Buffer circulaire lock-free** : Permet un transfert d'images sans verrou entre les threads d'acquisition et de traitement
- **Horodatage précis** : Mécanisme de timestamping nanoseconde avec compteur monotone pour garantir l'ordre
- **Architecture multi-thread** : Séparation des tâches d'acquisition et de traitement pour maximiser le parallélisme
- **Gestion mémoire optimisée** : Préallocation des buffers et réutilisation pour éviter les allocations dynamiques
- **Détection et récupération de désynchronisation** : Mécanismes automatiques pour détecter et récupérer des problèmes de synchronisation
- **Métriques de performance** : Surveillance complète du pipeline avec métriques en temps réel
- **Gestion des débordements** : Stratégies configurables pour gérer les pics de charge
- **Interfaces claires** : API bien définie pour l'intégration avec les modules d'acquisition et de traitement

## Performances

Le pipeline est conçu pour traiter jusqu'à 100 000 images par heure (environ 28 images par seconde) avec une latence minimale et zéro perte d'image dans des conditions normales.

## Architecture

Le pipeline est composé des éléments suivants :

1. **Buffer circulaire lock-free** : Stockage intermédiaire des images entre l'acquisition et le traitement
2. **Tâches d'acquisition** : Threads haute priorité pour l'acquisition d'images depuis les caméras
3. **Tâches de traitement** : Threads pour le traitement des images acquises
4. **Tâche de surveillance** : Thread pour la collecte et l'exposition des métriques
5. **Système d'horodatage** : Mécanisme précis pour l'horodatage des acquisitions
6. **Gestionnaire de récupération** : Détection et récupération automatique des désynchronisations

## Utilisation

### Configuration du pipeline

```rust
use heimdall_pipeline::{PipelineConfig, OverflowStrategy, PipelineState};
use heimdall_rt::RtPriority;

// Créer la configuration du pipeline
let config = PipelineConfig {
    buffer_capacity: 32,
    max_image_size: 1920 * 1080 * 3, // Full HD RGB
    acquisition_threads: 1,
    processing_threads: 4,
    acquisition_priority: RtPriority::Critical,
    processing_priority: RtPriority::High,
    acquisition_cpu_affinity: vec![0],
    processing_cpu_affinity: vec![1, 2, 3, 4],
    metrics_interval_ms: 1000,
    enable_auto_recovery: true,
    max_wait_time_ms: 100,
    overflow_strategy: OverflowStrategy::DropOldest,
};
```

### Création et initialisation du pipeline

```rust
use heimdall_pipeline::pipeline::AcquisitionPipeline;

// Créer le pipeline
let pipeline = AcquisitionPipeline::new(config)?;

// Ajouter un callback de traitement
pipeline.add_processor_callback(|image| {
    // Traiter l'image
    println!("Image traitée: {}x{}", image.width, image.height);
    Ok(())
})?;

// Initialiser le pipeline
pipeline.initialize()?;

// Démarrer le pipeline
pipeline.start()?;
```

### Arrêt du pipeline

```rust
// Arrêter le pipeline
pipeline.stop()?;
```

## Exemple complet

Voir le fichier `examples/bottle_inspection.rs` pour un exemple complet d'utilisation du pipeline pour l'inspection de bouteilles.

## Benchmarks

Des benchmarks sont disponibles dans le répertoire `benches/` pour évaluer les performances du pipeline.

Pour exécuter les benchmarks :

```bash
cargo bench
```

## Tests

Des tests unitaires sont disponibles pour chaque composant du pipeline.

Pour exécuter les tests :

```bash
cargo test
```

## Licence

Ce module fait partie du système de vision Heimdall et est soumis aux mêmes conditions de licence que le projet principal.
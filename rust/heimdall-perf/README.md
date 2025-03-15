# Module d'analyse de performance pour Heimdall Vision

Ce module fournit des outils d'analyse de performance pour le système d'inspection visuelle Heimdall Vision.

## Fonctionnalités

- **Profilage** : Mesure du temps d'exécution, utilisation CPU, mémoire, etc.
- **Métriques** : Collecte et analyse de métriques de performance
- **Rapports** : Génération de rapports au format JSON, HTML, Markdown, etc.
- **Visualisation** : Génération de flamegraphs pour l'analyse de performance
- **Surveillance système** : Collecte de métriques système (CPU, mémoire, etc.)

## Architecture

Le module est organisé en plusieurs composants :

- **ProfilingSession** : Session de profilage pour collecter des métriques
- **ProfilingManager** : Gestionnaire de sessions de profilage
- **Metrics** : Compteurs, chronomètres et mesureurs de débit
- **Reports** : Génération de rapports de performance
- **System** : Collecte de métriques système

## Utilisation

### Initialisation

```rust
use heimdall_perf::{init, MetricType, Measurement};

// Initialiser le profilage
let profiling_manager = init("./performance_reports")?;

// Créer une session de profilage
let session = {
    let mut manager = profiling_manager.lock().unwrap();
    manager.start_session("my_session")?
};

// Démarrer la collecte de métriques système
{
    let mut session_guard = session.lock().unwrap();
    session_guard.start_system_metrics()?;
}
```

### Mesure du temps d'exécution

```rust
// Mesurer le temps d'exécution d'une fonction
{
    let mut session_guard = session.lock().unwrap();
    session_guard.start_timing("my_function");
}

// Exécuter la fonction
my_function();

// Arrêter la mesure
{
    let mut session_guard = session.lock().unwrap();
    let duration = session_guard.stop_timing("my_function")?;
    println!("Temps d'exécution: {:?}", duration);
}
```

### Utilisation des compteurs

```rust
use heimdall_perf::metrics::{MetricCounter, Timer, ThroughputMeter};

// Créer un compteur de FPS
let mut fps_counter = MetricCounter::new("fps", MetricType::Throughput, "fps", 100);

// Mettre à jour le compteur
fps_counter.set(30.0);

// Ajouter la mesure à la session
{
    let mut session_guard = session.lock().unwrap();
    session_guard.add_measurement(fps_counter.to_measurement());
}
```

### Génération de rapports

```rust
use heimdall_perf::reports::ReportFormat;
use std::path::Path;

// Générer un rapport HTML
{
    let session_guard = session.lock().unwrap();
    session_guard.save_report(Path::new("./report.html"), ReportFormat::Html)?;
}

// Générer un flamegraph
{
    let session_guard = session.lock().unwrap();
    session_guard.generate_flamegraph(Path::new("./flamegraph.svg"))?;
}
```

### Arrêt de la session

```rust
// Arrêter la session
{
    let mut manager = profiling_manager.lock().unwrap();
    manager.stop_session()?;
}
```

## Intégration avec Heimdall Vision

Ce module s'intègre parfaitement avec les autres composants de Heimdall Vision :

- **heimdall-camera** : Analyse de performance de l'acquisition d'images
- **heimdall-rt** : Mesure des performances temps réel
- **heimdall-core** : Profilage des algorithmes de traitement d'images

## Exemple complet

Voir le fichier `examples/performance_analysis.rs` pour un exemple complet d'utilisation du module d'analyse de performance.
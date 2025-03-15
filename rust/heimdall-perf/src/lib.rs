use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

pub mod profiler;
pub mod metrics;
pub mod reports;
pub mod system;

/// Erreur liée à l'analyse de performance
#[derive(Error, Debug)]
pub enum PerfError {
    #[error("Erreur d'initialisation du profilage: {0}")]
    InitError(String),

    #[error("Erreur de mesure: {0}")]
    MeasurementError(String),

    #[error("Erreur d'E/S: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Erreur de sérialisation: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Erreur système: {0}")]
    SystemError(String),
}

/// Type de mesure de performance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    /// Temps d'exécution
    ExecutionTime,
    
    /// Utilisation CPU
    CpuUsage,
    
    /// Utilisation mémoire
    MemoryUsage,
    
    /// Débit de traitement (images/seconde)
    Throughput,
    
    /// Latence
    Latency,
    
    /// Gigue
    Jitter,
    
    /// Dépassements de délai
    DeadlineMisses,
    
    /// Métrique personnalisée
    Custom,
}

/// Mesure de performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    /// Type de mesure
    pub metric_type: MetricType,
    
    /// Nom de la mesure
    pub name: String,
    
    /// Valeur de la mesure
    pub value: f64,
    
    /// Unité de mesure
    pub unit: String,
    
    /// Horodatage
    pub timestamp: DateTime<Utc>,
    
    /// Métadonnées supplémentaires
    pub metadata: HashMap<String, String>,
}

impl Measurement {
    /// Crée une nouvelle mesure
    pub fn new(metric_type: MetricType, name: &str, value: f64, unit: &str) -> Self {
        Self {
            metric_type,
            name: name.to_string(),
            value,
            unit: unit.to_string(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }
    
    /// Ajoute une métadonnée
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Session de profilage
pub struct ProfilingSession {
    /// Nom de la session
    name: String,
    
    /// Heure de début
    start_time: Instant,
    
    /// Mesures collectées
    measurements: Vec<Measurement>,
    
    /// Mesures en cours
    active_measurements: HashMap<String, Instant>,
    
    /// Compteurs
    counters: HashMap<String, u64>,
    
    /// Métriques système
    system_metrics: Option<system::SystemMetrics>,
}

impl ProfilingSession {
    /// Crée une nouvelle session de profilage
    pub fn new(name: &str) -> Self {
        info!("Démarrage d'une nouvelle session de profilage: {}", name);
        
        Self {
            name: name.to_string(),
            start_time: Instant::now(),
            measurements: Vec::new(),
            active_measurements: HashMap::new(),
            counters: HashMap::new(),
            system_metrics: None,
        }
    }
    
    /// Démarre la collecte de métriques système
    pub fn start_system_metrics(&mut self) -> Result<(), PerfError> {
        info!("Démarrage de la collecte de métriques système");
        
        self.system_metrics = Some(system::SystemMetrics::new()?);
        
        Ok(())
    }
    
    /// Démarre une mesure de temps
    pub fn start_timing(&mut self, name: &str) {
        debug!("Démarrage de la mesure de temps: {}", name);
        
        self.active_measurements.insert(name.to_string(), Instant::now());
    }
    
    /// Arrête une mesure de temps
    pub fn stop_timing(&mut self, name: &str) -> Result<Duration, PerfError> {
        if let Some(start) = self.active_measurements.remove(name) {
            let duration = start.elapsed();
            debug!("Arrêt de la mesure de temps: {} = {:?}", name, duration);
            
            // Enregistrer la mesure
            self.measurements.push(Measurement::new(
                MetricType::ExecutionTime,
                name,
                duration.as_secs_f64() * 1000.0, // Convertir en millisecondes
                "ms",
            ));
            
            Ok(duration)
        } else {
            Err(PerfError::MeasurementError(format!("Mesure non démarrée: {}", name)))
        }
    }
    
    /// Incrémente un compteur
    pub fn increment_counter(&mut self, name: &str, value: u64) -> u64 {
        let counter = self.counters.entry(name.to_string()).or_insert(0);
        *counter += value;
        *counter
    }
    
    /// Ajoute une mesure
    pub fn add_measurement(&mut self, measurement: Measurement) {
        debug!("Ajout d'une mesure: {:?}", measurement);
        
        self.measurements.push(measurement);
    }
    
    /// Collecte les métriques système
    pub fn collect_system_metrics(&mut self) -> Result<(), PerfError> {
        if let Some(metrics) = &mut self.system_metrics {
            debug!("Collecte des métriques système");
            
            // Collecter les métriques
            metrics.collect()?;
            
            // Ajouter les mesures
            self.measurements.push(Measurement::new(
                MetricType::CpuUsage,
                "system_cpu",
                metrics.cpu_usage()?,
                "%",
            ));
            
            self.measurements.push(Measurement::new(
                MetricType::MemoryUsage,
                "system_memory",
                metrics.memory_usage()? as f64 / 1024.0 / 1024.0, // Convertir en Mo
                "MB",
            ));
            
            Ok(())
        } else {
            Err(PerfError::MeasurementError("Métriques système non initialisées".to_string()))
        }
    }
    
    /// Génère un rapport de performance
    pub fn generate_report(&self, format: reports::ReportFormat) -> Result<String, PerfError> {
        info!("Génération d'un rapport de performance au format {:?}", format);
        
        let report = reports::Report::new(&self.name, &self.measurements);
        report.generate(format)
    }
    
    /// Enregistre un rapport de performance dans un fichier
    pub fn save_report(&self, path: &Path, format: reports::ReportFormat) -> Result<(), PerfError> {
        info!("Enregistrement d'un rapport de performance dans: {:?}", path);
        
        let report = self.generate_report(format)?;
        
        let mut file = File::create(path)?;
        file.write_all(report.as_bytes())?;
        
        Ok(())
    }
    
    /// Génère un flamegraph
    pub fn generate_flamegraph(&self, path: &Path) -> Result<(), PerfError> {
        info!("Génération d'un flamegraph dans: {:?}", path);
        
        // Utiliser pprof pour générer un flamegraph
        profiler::generate_flamegraph(path)
    }
    
    /// Obtient la durée totale de la session
    pub fn duration(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    /// Obtient les mesures collectées
    pub fn measurements(&self) -> &[Measurement] {
        &self.measurements
    }
    
    /// Obtient les compteurs
    pub fn counters(&self) -> &HashMap<String, u64> {
        &self.counters
    }
}

/// Gestionnaire de profilage
pub struct ProfilingManager {
    /// Session active
    active_session: Option<Arc<Mutex<ProfilingSession>>>,
    
    /// Historique des sessions
    session_history: Vec<String>,
    
    /// Répertoire de sortie
    output_dir: String,
}

impl ProfilingManager {
    /// Crée un nouveau gestionnaire de profilage
    pub fn new(output_dir: &str) -> Self {
        info!("Initialisation du gestionnaire de profilage avec répertoire de sortie: {}", output_dir);
        
        // Créer le répertoire de sortie s'il n'existe pas
        std::fs::create_dir_all(output_dir).unwrap_or_else(|e| {
            warn!("Impossible de créer le répertoire de sortie: {}", e);
        });
        
        Self {
            active_session: None,
            session_history: Vec::new(),
            output_dir: output_dir.to_string(),
        }
    }
    
    /// Démarre une nouvelle session de profilage
    pub fn start_session(&mut self, name: &str) -> Result<Arc<Mutex<ProfilingSession>>, PerfError> {
        info!("Démarrage d'une nouvelle session de profilage: {}", name);
        
        // Arrêter la session active si elle existe
        if let Some(session) = &self.active_session {
            let session_name = session.lock().unwrap().name.clone();
            self.stop_session()?;
            info!("Session précédente arrêtée: {}", session_name);
        }
        
        // Créer une nouvelle session
        let session = Arc::new(Mutex::new(ProfilingSession::new(name)));
        self.active_session = Some(session.clone());
        
        Ok(session)
    }
    
    /// Arrête la session active
    pub fn stop_session(&mut self) -> Result<(), PerfError> {
        if let Some(session) = self.active_session.take() {
            let session_name = {
                let session_guard = session.lock().unwrap();
                let name = session_guard.name.clone();
                let duration = session_guard.duration();
                info!("Arrêt de la session de profilage: {} (durée: {:?})", name, duration);
                name
            };
            
            // Ajouter à l'historique
            self.session_history.push(session_name);
            
            // Générer un rapport
            let report_path = Path::new(&self.output_dir).join(format!("{}.json", self.session_history.len()));
            {
                let session_guard = session.lock().unwrap();
                session_guard.save_report(&report_path, reports::ReportFormat::Json)?;
            }
            
            Ok(())
        } else {
            Err(PerfError::MeasurementError("Aucune session active".to_string()))
        }
    }
    
    /// Obtient la session active
    pub fn active_session(&self) -> Option<Arc<Mutex<ProfilingSession>>> {
        self.active_session.clone()
    }
    
    /// Obtient l'historique des sessions
    pub fn session_history(&self) -> &[String] {
        &self.session_history
    }
}

/// Initialise le profilage global
pub fn init(output_dir: &str) -> Result<Arc<Mutex<ProfilingManager>>, PerfError> {
    info!("Initialisation du profilage global");
    
    // Initialiser le gestionnaire de profilage
    let manager = Arc::new(Mutex::new(ProfilingManager::new(output_dir)));
    
    // Initialiser le profiler
    profiler::init()?;
    
    Ok(manager)
}
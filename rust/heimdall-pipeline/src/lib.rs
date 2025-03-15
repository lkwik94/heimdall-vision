use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use log::{debug, error, info, warn};

pub mod buffer;
pub mod pipeline;
pub mod metrics;
pub mod timestamp;
pub mod scheduler;

/// Erreur liée au pipeline d'acquisition
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Erreur d'initialisation du pipeline: {0}")]
    InitError(String),

    #[error("Erreur de configuration: {0}")]
    ConfigError(String),

    #[error("Erreur d'acquisition: {0}")]
    AcquisitionError(String),

    #[error("Erreur de buffer: {0}")]
    BufferError(String),

    #[error("Erreur de synchronisation: {0}")]
    SyncError(String),

    #[error("Délai dépassé: {0}")]
    TimeoutError(String),

    #[error("Erreur de traitement: {0}")]
    ProcessingError(String),

    #[error("Erreur de caméra: {0}")]
    CameraError(#[from] heimdall_camera::CameraError),

    #[error("Erreur temps réel: {0}")]
    RtError(#[from] heimdall_rt::RtError),
}

/// Configuration du pipeline d'acquisition
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Nombre de buffers dans le pipeline
    pub buffer_capacity: usize,
    
    /// Taille maximale d'une image en octets
    pub max_image_size: usize,
    
    /// Nombre de threads d'acquisition
    pub acquisition_threads: usize,
    
    /// Nombre de threads de traitement
    pub processing_threads: usize,
    
    /// Priorité des threads d'acquisition
    pub acquisition_priority: heimdall_rt::RtPriority,
    
    /// Priorité des threads de traitement
    pub processing_priority: heimdall_rt::RtPriority,
    
    /// Affinité CPU des threads d'acquisition
    pub acquisition_cpu_affinity: Vec<usize>,
    
    /// Affinité CPU des threads de traitement
    pub processing_cpu_affinity: Vec<usize>,
    
    /// Intervalle de collecte des métriques en millisecondes
    pub metrics_interval_ms: u64,
    
    /// Activer la récupération automatique des désynchronisations
    pub enable_auto_recovery: bool,
    
    /// Délai maximum d'attente pour une image en millisecondes
    pub max_wait_time_ms: u64,
    
    /// Stratégie de gestion des débordements de buffer
    pub overflow_strategy: OverflowStrategy,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            buffer_capacity: 32,
            max_image_size: 1920 * 1080 * 3, // Full HD RGB
            acquisition_threads: 1,
            processing_threads: 2,
            acquisition_priority: heimdall_rt::RtPriority::Critical,
            processing_priority: heimdall_rt::RtPriority::High,
            acquisition_cpu_affinity: vec![0],
            processing_cpu_affinity: vec![1, 2],
            metrics_interval_ms: 1000,
            enable_auto_recovery: true,
            max_wait_time_ms: 100,
            overflow_strategy: OverflowStrategy::DropOldest,
        }
    }
}

/// Stratégie de gestion des débordements de buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowStrategy {
    /// Bloquer jusqu'à ce qu'un emplacement soit disponible
    Block,
    
    /// Supprimer l'image la plus ancienne
    DropOldest,
    
    /// Supprimer la nouvelle image
    DropNewest,
    
    /// Redimensionner le buffer (peut causer des allocations)
    Resize,
}

/// État du pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    /// Pipeline non initialisé
    Uninitialized,
    
    /// Pipeline initialisé mais pas démarré
    Ready,
    
    /// Pipeline en cours d'exécution
    Running,
    
    /// Pipeline en pause
    Paused,
    
    /// Pipeline arrêté
    Stopped,
    
    /// Pipeline en erreur
    Error,
}

/// Statistiques du pipeline
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// Nombre total d'images acquises
    pub total_frames_acquired: u64,
    
    /// Nombre total d'images traitées
    pub total_frames_processed: u64,
    
    /// Nombre total d'images perdues
    pub total_frames_dropped: u64,
    
    /// Nombre de débordements de buffer
    pub buffer_overflows: u64,
    
    /// Nombre de désynchronisations détectées
    pub desync_events: u64,
    
    /// Nombre de récupérations réussies
    pub recovery_events: u64,
    
    /// Taux d'acquisition moyen (images/seconde)
    pub avg_acquisition_rate: f64,
    
    /// Taux de traitement moyen (images/seconde)
    pub avg_processing_rate: f64,
    
    /// Latence moyenne d'acquisition (ms)
    pub avg_acquisition_latency: f64,
    
    /// Latence moyenne de traitement (ms)
    pub avg_processing_latency: f64,
    
    /// Utilisation moyenne du buffer (%)
    pub avg_buffer_usage: f64,
    
    /// Horodatage de la dernière mise à jour
    pub last_update: std::time::SystemTime,
}

/// Version du pipeline pour la compatibilité
pub const PIPELINE_VERSION: &str = "1.0.0";

/// Capacité maximale du buffer par défaut
pub const DEFAULT_BUFFER_CAPACITY: usize = 32;

/// Taille maximale d'image par défaut (Full HD RGB)
pub const DEFAULT_MAX_IMAGE_SIZE: usize = 1920 * 1080 * 3;

/// Nombre maximum de threads d'acquisition supportés
pub const MAX_ACQUISITION_THREADS: usize = 8;

/// Nombre maximum de threads de traitement supportés
pub const MAX_PROCESSING_THREADS: usize = 16;
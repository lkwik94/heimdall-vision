use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use log::{debug, error, info, warn};

pub mod scheduler;
pub mod sync;

/// Erreur liée au temps réel
#[derive(Error, Debug)]
pub enum RtError {
    #[error("Erreur de planification: {0}")]
    SchedulingError(String),

    #[error("Erreur de synchronisation: {0}")]
    SyncError(String),

    #[error("Délai dépassé: {0}")]
    TimeoutError(String),

    #[error("Erreur système: {0}")]
    SystemError(String),
}

/// Priorité de tâche temps réel
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RtPriority {
    /// Priorité basse (non temps réel)
    Low = 0,
    
    /// Priorité normale
    Normal = 1,
    
    /// Priorité haute
    High = 2,
    
    /// Priorité critique
    Critical = 3,
}

/// Configuration temps réel
#[derive(Debug, Clone)]
pub struct RtConfig {
    /// Priorité de la tâche
    pub priority: RtPriority,
    
    /// Période d'exécution en millisecondes (0 = apériodique)
    pub period_ms: u64,
    
    /// Délai d'exécution maximal en millisecondes
    pub deadline_ms: u64,
    
    /// Affinité CPU (liste des cœurs à utiliser)
    pub cpu_affinity: Vec<usize>,
    
    /// Verrouiller la mémoire
    pub lock_memory: bool,
    
    /// Utiliser l'ordonnanceur temps réel
    pub use_rt_scheduler: bool,
}

impl Default for RtConfig {
    fn default() -> Self {
        Self {
            priority: RtPriority::Normal,
            period_ms: 0,
            deadline_ms: 0,
            cpu_affinity: vec![],
            lock_memory: false,
            use_rt_scheduler: false,
        }
    }
}

/// Statistiques d'exécution temps réel
#[derive(Debug, Clone, Default)]
pub struct RtStats {
    /// Nombre d'exécutions
    pub executions: u64,
    
    /// Temps d'exécution minimum
    pub min_execution_time: Duration,
    
    /// Temps d'exécution maximum
    pub max_execution_time: Duration,
    
    /// Temps d'exécution moyen
    pub avg_execution_time: Duration,
    
    /// Nombre de dépassements de délai
    pub deadline_misses: u64,
    
    /// Gigue minimum (différence entre période prévue et réelle)
    pub min_jitter: Duration,
    
    /// Gigue maximum
    pub max_jitter: Duration,
    
    /// Gigue moyenne
    pub avg_jitter: Duration,
}

/// Contexte d'exécution temps réel
pub struct RtContext {
    /// Configuration
    config: RtConfig,
    
    /// Statistiques
    stats: RtStats,
    
    /// Heure de début de la dernière exécution
    last_start: Option<Instant>,
    
    /// Heure de fin de la dernière exécution
    last_end: Option<Instant>,
    
    /// Heure prévue de la prochaine exécution
    next_scheduled: Option<Instant>,
}

impl RtContext {
    /// Crée un nouveau contexte temps réel
    pub fn new(config: RtConfig) -> Self {
        Self {
            config,
            stats: RtStats::default(),
            last_start: None,
            last_end: None,
            next_scheduled: None,
        }
    }
    
    /// Marque le début d'une exécution
    pub fn start_execution(&mut self) {
        let now = Instant::now();
        self.last_start = Some(now);
        
        // Calculer la gigue si périodique
        if self.config.period_ms > 0 {
            if let Some(scheduled) = self.next_scheduled {
                let jitter = if now > scheduled {
                    now.duration_since(scheduled)
                } else {
                    scheduled.duration_since(now)
                };
                
                // Mettre à jour les statistiques de gigue
                if self.stats.executions == 0 {
                    self.stats.min_jitter = jitter;
                    self.stats.max_jitter = jitter;
                    self.stats.avg_jitter = jitter;
                } else {
                    if jitter < self.stats.min_jitter {
                        self.stats.min_jitter = jitter;
                    }
                    if jitter > self.stats.max_jitter {
                        self.stats.max_jitter = jitter;
                    }
                    
                    // Moyenne mobile
                    let total_jitter = self.stats.avg_jitter.as_nanos() as u128 * self.stats.executions as u128;
                    let new_total = total_jitter + jitter.as_nanos() as u128;
                    let new_avg = new_total / (self.stats.executions + 1) as u128;
                    self.stats.avg_jitter = Duration::from_nanos(new_avg as u64);
                }
            }
            
            // Planifier la prochaine exécution
            self.next_scheduled = Some(now + Duration::from_millis(self.config.period_ms));
        }
    }
    
    /// Marque la fin d'une exécution
    pub fn end_execution(&mut self) {
        let now = Instant::now();
        self.last_end = Some(now);
        
        if let Some(start) = self.last_start {
            let execution_time = now.duration_since(start);
            
            // Vérifier le dépassement de délai
            if self.config.deadline_ms > 0 {
                let deadline = Duration::from_millis(self.config.deadline_ms);
                if execution_time > deadline {
                    self.stats.deadline_misses += 1;
                    warn!("Dépassement de délai: {:?} > {:?}", execution_time, deadline);
                }
            }
            
            // Mettre à jour les statistiques d'exécution
            if self.stats.executions == 0 {
                self.stats.min_execution_time = execution_time;
                self.stats.max_execution_time = execution_time;
                self.stats.avg_execution_time = execution_time;
            } else {
                if execution_time < self.stats.min_execution_time {
                    self.stats.min_execution_time = execution_time;
                }
                if execution_time > self.stats.max_execution_time {
                    self.stats.max_execution_time = execution_time;
                }
                
                // Moyenne mobile
                let total_time = self.stats.avg_execution_time.as_nanos() as u128 * self.stats.executions as u128;
                let new_total = total_time + execution_time.as_nanos() as u128;
                let new_avg = new_total / (self.stats.executions + 1) as u128;
                self.stats.avg_execution_time = Duration::from_nanos(new_avg as u64);
            }
            
            self.stats.executions += 1;
        }
    }
    
    /// Obtient les statistiques d'exécution
    pub fn get_stats(&self) -> RtStats {
        self.stats.clone()
    }
    
    /// Réinitialise les statistiques
    pub fn reset_stats(&mut self) {
        self.stats = RtStats::default();
    }
    
    /// Obtient la configuration
    pub fn get_config(&self) -> &RtConfig {
        &self.config
    }
}

/// Initialise l'environnement temps réel
pub fn init_rt_environment(config: &RtConfig) -> Result<(), RtError> {
    info!("Initialisation de l'environnement temps réel avec config: {:?}", config);
    
    // Verrouiller la mémoire si demandé
    if config.lock_memory {
        #[cfg(target_os = "linux")]
        {
            use nix::sys::mman::{mlockall, MlockallFlags};
            match mlockall(MlockallFlags::MCL_CURRENT | MlockallFlags::MCL_FUTURE) {
                Ok(_) => info!("Mémoire verrouillée avec succès"),
                Err(e) => {
                    warn!("Impossible de verrouiller la mémoire: {}", e);
                    return Err(RtError::SystemError(format!("Impossible de verrouiller la mémoire: {}", e)));
                }
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("Le verrouillage de la mémoire n'est pas supporté sur cette plateforme");
        }
    }
    
    // Configurer l'affinité CPU si spécifiée
    if !config.cpu_affinity.is_empty() {
        #[cfg(target_os = "linux")]
        {
            use nix::sched::{sched_setaffinity, CpuSet};
            use nix::unistd::Pid;
            
            let mut cpu_set = CpuSet::new();
            for cpu in &config.cpu_affinity {
                cpu_set.set(*cpu)?;
            }
            
            match sched_setaffinity(Pid::this(), &cpu_set) {
                Ok(_) => info!("Affinité CPU configurée avec succès"),
                Err(e) => {
                    warn!("Impossible de configurer l'affinité CPU: {}", e);
                    return Err(RtError::SystemError(format!("Impossible de configurer l'affinité CPU: {}", e)));
                }
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("L'affinité CPU n'est pas supportée sur cette plateforme");
        }
    }
    
    // Configurer l'ordonnanceur temps réel si demandé
    if config.use_rt_scheduler {
        #[cfg(target_os = "linux")]
        {
            use nix::sched::{sched_setscheduler, sched_param, SchedPolicy};
            use nix::unistd::Pid;
            
            let policy = SchedPolicy::SCHED_FIFO;
            let priority = match config.priority {
                RtPriority::Low => 1,
                RtPriority::Normal => 50,
                RtPriority::High => 80,
                RtPriority::Critical => 99,
            };
            
            let param = sched_param { sched_priority: priority };
            
            match sched_setscheduler(Pid::this(), policy, &param) {
                Ok(_) => info!("Ordonnanceur temps réel configuré avec succès"),
                Err(e) => {
                    warn!("Impossible de configurer l'ordonnanceur temps réel: {}", e);
                    return Err(RtError::SystemError(format!("Impossible de configurer l'ordonnanceur temps réel: {}", e)));
                }
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("L'ordonnanceur temps réel n'est pas supporté sur cette plateforme");
        }
    }
    
    Ok(())
}
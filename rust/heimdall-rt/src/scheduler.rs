use crate::{RtConfig, RtContext, RtError, RtPriority, RtStats};
use log::{debug, error, info, warn};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// Type de tâche temps réel
pub enum TaskType {
    /// Tâche périodique
    Periodic,
    
    /// Tâche apériodique
    Aperiodic,
    
    /// Tâche sporadique (déclenchée par événement)
    Sporadic,
}

/// Commande pour une tâche
pub enum TaskCommand {
    /// Exécuter la tâche
    Execute,
    
    /// Arrêter la tâche
    Stop,
    
    /// Mettre en pause la tâche
    Pause,
    
    /// Reprendre la tâche
    Resume,
}

/// État d'une tâche
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Tâche inactive
    Inactive,
    
    /// Tâche en cours d'exécution
    Running,
    
    /// Tâche en pause
    Paused,
    
    /// Tâche terminée
    Terminated,
}

/// Tâche temps réel
pub struct RtTask {
    /// Identifiant de la tâche
    id: String,
    
    /// Type de tâche
    task_type: TaskType,
    
    /// Configuration temps réel
    config: RtConfig,
    
    /// Contexte d'exécution
    context: Arc<Mutex<RtContext>>,
    
    /// État de la tâche
    state: TaskState,
    
    /// Canal de commande
    command_tx: Option<mpsc::Sender<TaskCommand>>,
    
    /// Handle de la tâche
    task_handle: Option<JoinHandle<()>>,
}

impl RtTask {
    /// Crée une nouvelle tâche temps réel
    pub fn new(id: &str, task_type: TaskType, config: RtConfig) -> Self {
        let context = Arc::new(Mutex::new(RtContext::new(config.clone())));
        
        Self {
            id: id.to_string(),
            task_type,
            config,
            context,
            state: TaskState::Inactive,
            command_tx: None,
            task_handle: None,
        }
    }
    
    /// Démarre la tâche
    pub async fn start<F>(&mut self, mut task_fn: F) -> Result<(), RtError>
    where
        F: FnMut() + Send + 'static,
    {
        if self.state != TaskState::Inactive && self.state != TaskState::Terminated {
            return Err(RtError::SchedulingError(format!("La tâche {} est déjà en cours d'exécution", self.id)));
        }
        
        info!("Démarrage de la tâche temps réel: {}", self.id);
        
        // Initialiser l'environnement temps réel
        crate::init_rt_environment(&self.config)?;
        
        // Créer le canal de commande
        let (command_tx, mut command_rx) = mpsc::channel(10);
        self.command_tx = Some(command_tx);
        
        // Cloner le contexte pour la tâche
        let context = self.context.clone();
        
        // Créer la tâche
        let task_id = self.id.clone();
        let config = self.config.clone();
        
        let handle = tokio::spawn(async move {
            info!("Tâche {} démarrée", task_id);
            
            let mut running = true;
            let mut paused = false;
            
            // Boucle principale de la tâche
            while running {
                // Vérifier les commandes
                if let Ok(command) = command_rx.try_recv() {
                    match command {
                        TaskCommand::Execute => {
                            // Exécuter immédiatement (pour les tâches sporadiques)
                            if !paused {
                                let mut ctx = context.lock().unwrap();
                                ctx.start_execution();
                                drop(ctx);
                                
                                task_fn();
                                
                                let mut ctx = context.lock().unwrap();
                                ctx.end_execution();
                            }
                        },
                        TaskCommand::Stop => {
                            info!("Tâche {} arrêtée", task_id);
                            running = false;
                            break;
                        },
                        TaskCommand::Pause => {
                            info!("Tâche {} mise en pause", task_id);
                            paused = true;
                        },
                        TaskCommand::Resume => {
                            info!("Tâche {} reprise", task_id);
                            paused = false;
                        },
                    }
                }
                
                // Exécuter la tâche si périodique et non en pause
                if config.period_ms > 0 && !paused {
                    let mut ctx = context.lock().unwrap();
                    ctx.start_execution();
                    drop(ctx);
                    
                    task_fn();
                    
                    let mut ctx = context.lock().unwrap();
                    ctx.end_execution();
                    
                    // Attendre jusqu'à la prochaine période
                    let period = Duration::from_millis(config.period_ms);
                    sleep(period).await;
                } else {
                    // Pour les tâches non périodiques, attendre une commande
                    sleep(Duration::from_millis(1)).await;
                }
            }
            
            info!("Tâche {} terminée", task_id);
        });
        
        self.task_handle = Some(handle);
        self.state = TaskState::Running;
        
        Ok(())
    }
    
    /// Arrête la tâche
    pub async fn stop(&mut self) -> Result<(), RtError> {
        if self.state != TaskState::Running && self.state != TaskState::Paused {
            return Err(RtError::SchedulingError(format!("La tâche {} n'est pas en cours d'exécution", self.id)));
        }
        
        info!("Arrêt de la tâche temps réel: {}", self.id);
        
        if let Some(tx) = &self.command_tx {
            if let Err(e) = tx.send(TaskCommand::Stop).await {
                error!("Erreur lors de l'envoi de la commande d'arrêt: {}", e);
                return Err(RtError::SchedulingError(format!("Erreur lors de l'arrêt de la tâche: {}", e)));
            }
        }
        
        if let Some(handle) = self.task_handle.take() {
            // Attendre la fin de la tâche avec un timeout
            let timeout = Duration::from_secs(5);
            let result = tokio::time::timeout(timeout, handle).await;
            
            if result.is_err() {
                warn!("La tâche {} n'a pas pu être arrêtée proprement", self.id);
                return Err(RtError::TimeoutError(format!("Timeout lors de l'arrêt de la tâche {}", self.id)));
            }
        }
        
        self.state = TaskState::Terminated;
        self.command_tx = None;
        
        Ok(())
    }
    
    /// Met en pause la tâche
    pub async fn pause(&mut self) -> Result<(), RtError> {
        if self.state != TaskState::Running {
            return Err(RtError::SchedulingError(format!("La tâche {} n'est pas en cours d'exécution", self.id)));
        }
        
        info!("Mise en pause de la tâche temps réel: {}", self.id);
        
        if let Some(tx) = &self.command_tx {
            if let Err(e) = tx.send(TaskCommand::Pause).await {
                error!("Erreur lors de l'envoi de la commande de pause: {}", e);
                return Err(RtError::SchedulingError(format!("Erreur lors de la mise en pause de la tâche: {}", e)));
            }
        }
        
        self.state = TaskState::Paused;
        
        Ok(())
    }
    
    /// Reprend la tâche
    pub async fn resume(&mut self) -> Result<(), RtError> {
        if self.state != TaskState::Paused {
            return Err(RtError::SchedulingError(format!("La tâche {} n'est pas en pause", self.id)));
        }
        
        info!("Reprise de la tâche temps réel: {}", self.id);
        
        if let Some(tx) = &self.command_tx {
            if let Err(e) = tx.send(TaskCommand::Resume).await {
                error!("Erreur lors de l'envoi de la commande de reprise: {}", e);
                return Err(RtError::SchedulingError(format!("Erreur lors de la reprise de la tâche: {}", e)));
            }
        }
        
        self.state = TaskState::Running;
        
        Ok(())
    }
    
    /// Exécute la tâche immédiatement (pour les tâches sporadiques)
    pub async fn execute(&self) -> Result<(), RtError> {
        if self.state != TaskState::Running {
            return Err(RtError::SchedulingError(format!("La tâche {} n'est pas en cours d'exécution", self.id)));
        }
        
        debug!("Exécution immédiate de la tâche temps réel: {}", self.id);
        
        if let Some(tx) = &self.command_tx {
            if let Err(e) = tx.send(TaskCommand::Execute).await {
                error!("Erreur lors de l'envoi de la commande d'exécution: {}", e);
                return Err(RtError::SchedulingError(format!("Erreur lors de l'exécution de la tâche: {}", e)));
            }
        }
        
        Ok(())
    }
    
    /// Obtient les statistiques de la tâche
    pub fn get_stats(&self) -> Result<RtStats, RtError> {
        let context = self.context.lock().map_err(|e| {
            RtError::SyncError(format!("Erreur lors de l'accès au contexte: {}", e))
        })?;
        
        Ok(context.get_stats())
    }
    
    /// Réinitialise les statistiques de la tâche
    pub fn reset_stats(&self) -> Result<(), RtError> {
        let mut context = self.context.lock().map_err(|e| {
            RtError::SyncError(format!("Erreur lors de l'accès au contexte: {}", e))
        })?;
        
        context.reset_stats();
        
        Ok(())
    }
    
    /// Obtient l'état de la tâche
    pub fn get_state(&self) -> TaskState {
        self.state
    }
    
    /// Obtient l'identifiant de la tâche
    pub fn get_id(&self) -> &str {
        &self.id
    }
}

/// Ordonnanceur de tâches temps réel
pub struct RtScheduler {
    /// Tâches gérées par l'ordonnanceur
    tasks: Vec<RtTask>,
}

impl RtScheduler {
    /// Crée un nouvel ordonnanceur
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
        }
    }
    
    /// Ajoute une tâche à l'ordonnanceur
    pub fn add_task(&mut self, task: RtTask) {
        self.tasks.push(task);
    }
    
    /// Démarre toutes les tâches
    pub async fn start_all<F>(&mut self, task_factory: F) -> Result<(), RtError>
    where
        F: Fn(&str) -> Box<dyn FnMut() + Send + 'static>,
    {
        for task in &mut self.tasks {
            let task_fn = task_factory(task.get_id());
            task.start(*task_fn).await?;
        }
        
        Ok(())
    }
    
    /// Arrête toutes les tâches
    pub async fn stop_all(&mut self) -> Result<(), RtError> {
        for task in &mut self.tasks {
            task.stop().await?;
        }
        
        Ok(())
    }
    
    /// Obtient une tâche par son identifiant
    pub fn get_task(&mut self, id: &str) -> Option<&mut RtTask> {
        self.tasks.iter_mut().find(|task| task.get_id() == id)
    }
    
    /// Obtient les statistiques de toutes les tâches
    pub fn get_all_stats(&self) -> HashMap<String, Result<RtStats, RtError>> {
        let mut stats = HashMap::new();
        
        for task in &self.tasks {
            stats.insert(task.get_id().to_string(), task.get_stats());
        }
        
        stats
    }
}

use std::collections::HashMap;
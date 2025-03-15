use std::sync::{Arc, Mutex, Condvar};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use crossbeam::channel::{self, Sender, Receiver};
use log::{debug, error, info, warn};
use heimdall_rt::{RtConfig, RtPriority, RtContext};
use crate::PipelineError;

/// Type de tâche
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// Tâche d'acquisition
    Acquisition,
    
    /// Tâche de traitement
    Processing,
    
    /// Tâche de surveillance
    Monitoring,
}

/// État d'une tâche
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Tâche non démarrée
    NotStarted,
    
    /// Tâche en cours d'exécution
    Running,
    
    /// Tâche en pause
    Paused,
    
    /// Tâche terminée
    Finished,
    
    /// Tâche en erreur
    Error,
}

/// Message de contrôle pour une tâche
#[derive(Debug, Clone)]
pub enum TaskControl {
    /// Démarrer la tâche
    Start,
    
    /// Mettre en pause la tâche
    Pause,
    
    /// Reprendre la tâche
    Resume,
    
    /// Arrêter la tâche
    Stop,
    
    /// Réinitialiser la tâche
    Reset,
}

/// Contexte d'une tâche
pub struct TaskContext {
    /// Type de tâche
    task_type: TaskType,
    
    /// État de la tâche
    state: Arc<Mutex<TaskState>>,
    
    /// Canal de contrôle
    control_rx: Receiver<TaskControl>,
    
    /// Canal de contrôle (émetteur pour le planificateur)
    control_tx: Sender<TaskControl>,
    
    /// Configuration temps réel
    rt_config: RtConfig,
    
    /// Contexte temps réel
    rt_context: Option<RtContext>,
    
    /// Horodatage de démarrage
    start_time: Option<Instant>,
    
    /// Horodatage de la dernière exécution
    last_execution: Option<Instant>,
    
    /// Période d'exécution en millisecondes (0 = apériodique)
    period_ms: u64,
}

impl TaskContext {
    /// Crée un nouveau contexte de tâche
    pub fn new(task_type: TaskType, rt_config: RtConfig, period_ms: u64) -> Self {
        let (tx, rx) = channel::unbounded();
        
        Self {
            task_type,
            state: Arc::new(Mutex::new(TaskState::NotStarted)),
            control_rx: rx,
            control_tx: tx,
            rt_config,
            rt_context: None,
            start_time: None,
            last_execution: None,
            period_ms,
        }
    }
    
    /// Initialise le contexte temps réel
    pub fn init_rt(&mut self) -> Result<(), PipelineError> {
        // Initialiser l'environnement temps réel
        heimdall_rt::init_rt_environment(&self.rt_config)
            .map_err(|e| PipelineError::RtError(e))?;
        
        // Créer le contexte temps réel
        self.rt_context = Some(RtContext::new(self.rt_config.clone()));
        
        Ok(())
    }
    
    /// Obtient l'état actuel de la tâche
    pub fn get_state(&self) -> TaskState {
        *self.state.lock().unwrap()
    }
    
    /// Définit l'état de la tâche
    pub fn set_state(&self, state: TaskState) {
        *self.state.lock().unwrap() = state;
    }
    
    /// Vérifie si un message de contrôle est disponible
    pub fn check_control(&self) -> Option<TaskControl> {
        match self.control_rx.try_recv() {
            Ok(control) => Some(control),
            Err(_) => None,
        }
    }
    
    /// Attend un message de contrôle avec timeout
    pub fn wait_control(&self, timeout: Duration) -> Option<TaskControl> {
        match self.control_rx.recv_timeout(timeout) {
            Ok(control) => Some(control),
            Err(_) => None,
        }
    }
    
    /// Marque le début d'une exécution
    pub fn start_execution(&mut self) {
        let now = Instant::now();
        
        if self.start_time.is_none() {
            self.start_time = Some(now);
        }
        
        self.last_execution = Some(now);
        
        if let Some(rt_context) = &mut self.rt_context {
            rt_context.start_execution();
        }
    }
    
    /// Marque la fin d'une exécution
    pub fn end_execution(&mut self) {
        if let Some(rt_context) = &mut self.rt_context {
            rt_context.end_execution();
        }
    }
    
    /// Calcule le temps à attendre jusqu'à la prochaine exécution
    pub fn time_until_next_execution(&self) -> Option<Duration> {
        if self.period_ms == 0 {
            // Tâche apériodique
            return None;
        }
        
        if let Some(last) = self.last_execution {
            let period = Duration::from_millis(self.period_ms);
            let now = Instant::now();
            let elapsed = now.duration_since(last);
            
            if elapsed >= period {
                // La période est déjà écoulée
                Some(Duration::from_millis(0))
            } else {
                // Attendre le reste de la période
                Some(period - elapsed)
            }
        } else {
            // Première exécution
            Some(Duration::from_millis(0))
        }
    }
    
    /// Obtient le canal de contrôle (émetteur)
    pub fn get_control_sender(&self) -> Sender<TaskControl> {
        self.control_tx.clone()
    }
    
    /// Obtient les statistiques temps réel
    pub fn get_rt_stats(&self) -> Option<heimdall_rt::RtStats> {
        self.rt_context.as_ref().map(|ctx| ctx.get_stats())
    }
}

/// Planificateur de tâches
pub struct TaskScheduler {
    /// Tâches gérées par le planificateur
    tasks: Mutex<Vec<Arc<Mutex<TaskContext>>>>,
    
    /// Threads des tâches
    threads: Mutex<Vec<JoinHandle<()>>>,
    
    /// Condition pour la synchronisation
    condition: Arc<(Mutex<bool>, Condvar)>,
    
    /// Drapeau d'arrêt
    stop_flag: Arc<Mutex<bool>>,
}

impl TaskScheduler {
    /// Crée un nouveau planificateur de tâches
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            threads: Mutex::new(Vec::new()),
            condition: Arc::new((Mutex::new(false), Condvar::new())),
            stop_flag: Arc::new(Mutex::new(false)),
        }
    }
    
    /// Ajoute une tâche au planificateur
    pub fn add_task(&self, task: TaskContext) -> Arc<Mutex<TaskContext>> {
        let task = Arc::new(Mutex::new(task));
        self.tasks.lock().unwrap().push(task.clone());
        task
    }
    
    /// Démarre toutes les tâches
    pub fn start_all(&self) -> Result<(), PipelineError> {
        let tasks = self.tasks.lock().unwrap();
        
        for task in tasks.iter() {
            let mut task_guard = task.lock().unwrap();
            task_guard.set_state(TaskState::Running);
            
            // Envoyer le message de démarrage
            let sender = task_guard.get_control_sender();
            sender.send(TaskControl::Start).map_err(|_| {
                PipelineError::SyncError("Impossible d'envoyer le message de démarrage".to_string())
            })?;
        }
        
        Ok(())
    }
    
    /// Arrête toutes les tâches
    pub fn stop_all(&self) -> Result<(), PipelineError> {
        let tasks = self.tasks.lock().unwrap();
        
        for task in tasks.iter() {
            let task_guard = task.lock().unwrap();
            
            // Envoyer le message d'arrêt
            let sender = task_guard.get_control_sender();
            sender.send(TaskControl::Stop).map_err(|_| {
                PipelineError::SyncError("Impossible d'envoyer le message d'arrêt".to_string())
            })?;
        }
        
        // Définir le drapeau d'arrêt
        *self.stop_flag.lock().unwrap() = true;
        
        // Réveiller toutes les tâches en attente
        let (lock, cvar) = &*self.condition;
        let mut ready = lock.lock().unwrap();
        *ready = true;
        cvar.notify_all();
        
        Ok(())
    }
    
    /// Attend que toutes les tâches se terminent
    pub fn join_all(&self) -> Result<(), PipelineError> {
        let mut threads = self.threads.lock().unwrap();
        
        while let Some(handle) = threads.pop() {
            handle.join().map_err(|_| {
                PipelineError::SyncError("Impossible de joindre le thread".to_string())
            })?;
        }
        
        Ok(())
    }
    
    /// Exécute une fonction dans une tâche temps réel
    pub fn spawn_rt_task<F>(&self, mut task_context: TaskContext, task_fn: F) -> Result<Arc<Mutex<TaskContext>>, PipelineError>
    where
        F: FnMut(&mut TaskContext) -> Result<(), PipelineError> + Send + 'static,
    {
        // Initialiser le contexte temps réel
        task_context.init_rt()?;
        
        // Ajouter la tâche au planificateur
        let task = self.add_task(task_context);
        let task_clone = task.clone();
        
        // Créer le thread
        let condition = self.condition.clone();
        let stop_flag = self.stop_flag.clone();
        
        let handle = thread::spawn(move || {
            let mut task_fn = task_fn;
            
            loop {
                // Vérifier si nous devons arrêter
                if *stop_flag.lock().unwrap() {
                    break;
                }
                
                // Obtenir le contexte de la tâche
                let mut task_guard = task_clone.lock().unwrap();
                
                // Vérifier les messages de contrôle
                if let Some(control) = task_guard.check_control() {
                    match control {
                        TaskControl::Start => {
                            task_guard.set_state(TaskState::Running);
                        },
                        TaskControl::Pause => {
                            task_guard.set_state(TaskState::Paused);
                            
                            // Attendre le message de reprise
                            drop(task_guard);
                            let (lock, cvar) = &*condition;
                            let mut ready = lock.lock().unwrap();
                            while !*ready && !*stop_flag.lock().unwrap() {
                                ready = cvar.wait(ready).unwrap();
                            }
                            continue;
                        },
                        TaskControl::Resume => {
                            task_guard.set_state(TaskState::Running);
                        },
                        TaskControl::Stop => {
                            task_guard.set_state(TaskState::Finished);
                            break;
                        },
                        TaskControl::Reset => {
                            // Réinitialiser le contexte
                            if let Some(rt_context) = &mut task_guard.rt_context {
                                rt_context.reset_stats();
                            }
                        },
                    }
                }
                
                // Vérifier si la tâche est en cours d'exécution
                if task_guard.get_state() != TaskState::Running {
                    drop(task_guard);
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                
                // Calculer le temps à attendre jusqu'à la prochaine exécution
                let wait_time = task_guard.time_until_next_execution();
                
                if let Some(duration) = wait_time {
                    if !duration.is_zero() {
                        // Attendre jusqu'à la prochaine exécution
                        drop(task_guard);
                        thread::sleep(duration);
                        task_guard = task_clone.lock().unwrap();
                    }
                }
                
                // Exécuter la tâche
                task_guard.start_execution();
                
                // Exécuter la fonction de la tâche
                let result = task_fn(&mut task_guard);
                
                // Marquer la fin de l'exécution
                task_guard.end_execution();
                
                // Vérifier le résultat
                if let Err(e) = result {
                    error!("Erreur lors de l'exécution de la tâche: {}", e);
                    task_guard.set_state(TaskState::Error);
                    break;
                }
                
                // Libérer le verrou
                drop(task_guard);
                
                // Céder le CPU si la tâche est apériodique
                if wait_time.is_none() {
                    thread::yield_now();
                }
            }
        });
        
        // Ajouter le thread à la liste
        self.threads.lock().unwrap().push(handle);
        
        Ok(task)
    }
}

/// Tests unitaires
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    #[test]
    fn test_task_context() {
        let mut context = TaskContext::new(
            TaskType::Processing,
            RtConfig::default(),
            100, // 100ms
        );
        
        assert_eq!(context.get_state(), TaskState::NotStarted);
        
        context.set_state(TaskState::Running);
        assert_eq!(context.get_state(), TaskState::Running);
        
        context.start_execution();
        assert!(context.last_execution.is_some());
        
        // Attendre un peu
        thread::sleep(Duration::from_millis(50));
        
        // Vérifier le temps jusqu'à la prochaine exécution
        let wait_time = context.time_until_next_execution().unwrap();
        assert!(wait_time.as_millis() <= 50);
        
        context.end_execution();
    }
    
    #[test]
    fn test_scheduler_basic() {
        let scheduler = TaskScheduler::new();
        
        // Créer une tâche
        let context = TaskContext::new(
            TaskType::Processing,
            RtConfig::default(),
            0, // apériodique
        );
        
        // Compteur partagé
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        
        // Ajouter la tâche
        let task = scheduler.spawn_rt_task(context, move |ctx| {
            // Incrémenter le compteur
            counter_clone.fetch_add(1, Ordering::SeqCst);
            
            // Vérifier si nous devons arrêter après 10 incréments
            if counter_clone.load(Ordering::SeqCst) >= 10 {
                ctx.set_state(TaskState::Finished);
            }
            
            // Simuler un traitement
            thread::sleep(Duration::from_millis(10));
            
            Ok(())
        }).unwrap();
        
        // Démarrer toutes les tâches
        scheduler.start_all().unwrap();
        
        // Attendre un peu
        thread::sleep(Duration::from_millis(200));
        
        // Arrêter toutes les tâches
        scheduler.stop_all().unwrap();
        
        // Attendre que toutes les tâches se terminent
        scheduler.join_all().unwrap();
        
        // Vérifier le compteur
        assert!(counter.load(Ordering::SeqCst) >= 10);
    }
}
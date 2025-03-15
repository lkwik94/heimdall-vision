use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};
use std::thread;
use std::collections::VecDeque;
use crossbeam::channel::{self, Sender, Receiver, TrySendError, TryRecvError};
use log::{debug, error, info, warn};
use heimdall_camera::{Camera, CameraConfig, CameraFrame, CameraError, CameraFactory};
use heimdall_rt::{RtConfig, RtPriority};
use crate::{PipelineConfig, PipelineError, PipelineState, PipelineStats, OverflowStrategy};
use crate::buffer::{LockFreeRingBuffer, ImageSlot};
use crate::metrics::PipelineMetrics;
use crate::timestamp::Timestamp;
use crate::scheduler::{TaskScheduler, TaskContext, TaskType, TaskControl, TaskState};

/// Structure d'une image dans le pipeline
#[derive(Debug)]
pub struct PipelineImage {
    /// Données de l'image
    pub data: Vec<u8>,
    
    /// Largeur de l'image
    pub width: u32,
    
    /// Hauteur de l'image
    pub height: u32,
    
    /// Format de pixel
    pub format: u32,
    
    /// Horodatage précis de l'acquisition
    pub timestamp: Timestamp,
    
    /// Numéro de séquence de l'image
    pub sequence: u64,
    
    /// Métadonnées supplémentaires
    pub metadata: Vec<(String, String)>,
}

impl PipelineImage {
    /// Crée une nouvelle image à partir d'un frame de caméra
    pub fn from_camera_frame(frame: &CameraFrame, sequence: u64) -> Self {
        let format = match frame.pixel_format {
            heimdall_camera::PixelFormat::Mono8 => 0,
            heimdall_camera::PixelFormat::Mono16 => 1,
            heimdall_camera::PixelFormat::RGB8 => 2,
            heimdall_camera::PixelFormat::BGR8 => 3,
            heimdall_camera::PixelFormat::RGBA8 => 4,
            heimdall_camera::PixelFormat::BGRA8 => 5,
            _ => 0,
        };
        
        Self {
            data: frame.data.clone(),
            width: frame.width,
            height: frame.height,
            format,
            timestamp: Timestamp::from_system_time(frame.timestamp),
            sequence,
            metadata: frame.metadata.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
    
    /// Crée une nouvelle image à partir d'un slot du buffer
    pub fn from_buffer_slot(slot: &ImageSlot) -> Self {
        let data = slot.data[0..slot.size].to_vec();
        
        Self {
            data,
            width: slot.width,
            height: slot.height,
            format: slot.format,
            timestamp: slot.timestamp,
            sequence: slot.sequence,
            metadata: Vec::new(), // Les métadonnées ne sont pas stockées dans le slot
        }
    }
}

/// Callback de traitement d'image
pub type ImageProcessorCallback = Box<dyn Fn(&PipelineImage) -> Result<(), PipelineError> + Send + Sync>;

/// Pipeline d'acquisition d'images
pub struct AcquisitionPipeline {
    /// Configuration du pipeline
    config: Arc<RwLock<PipelineConfig>>,
    
    /// État du pipeline
    state: Arc<RwLock<PipelineState>>,
    
    /// Buffer circulaire lock-free
    buffer: Arc<LockFreeRingBuffer>,
    
    /// Métriques du pipeline
    metrics: Arc<PipelineMetrics>,
    
    /// Planificateur de tâches
    scheduler: Arc<TaskScheduler>,
    
    /// Tâches d'acquisition
    acquisition_tasks: Arc<Mutex<Vec<Arc<Mutex<TaskContext>>>>>,
    
    /// Tâches de traitement
    processing_tasks: Arc<Mutex<Vec<Arc<Mutex<TaskContext>>>>>,
    
    /// Tâche de surveillance
    monitoring_task: Arc<Mutex<Option<Arc<Mutex<TaskContext>>>>>,
    
    /// Caméras
    cameras: Arc<Mutex<Vec<Box<dyn Camera>>>>,
    
    /// Compteur de séquence global
    sequence_counter: Arc<Mutex<u64>>,
    
    /// Canal pour les callbacks de traitement
    processor_callbacks: Arc<Mutex<Vec<ImageProcessorCallback>>>,
    
    /// Dernier horodatage d'acquisition
    last_acquisition: Arc<Mutex<Option<Timestamp>>>,
    
    /// Dernier horodatage de traitement
    last_processing: Arc<Mutex<Option<Timestamp>>>,
    
    /// Statistiques du pipeline
    stats: Arc<RwLock<PipelineStats>>,
}

impl AcquisitionPipeline {
    /// Crée un nouveau pipeline d'acquisition
    pub fn new(config: PipelineConfig) -> Result<Self, PipelineError> {
        // Valider la configuration
        if config.buffer_capacity == 0 {
            return Err(PipelineError::ConfigError("La capacité du buffer doit être supérieure à 0".to_string()));
        }
        
        if config.max_image_size == 0 {
            return Err(PipelineError::ConfigError("La taille maximale d'image doit être supérieure à 0".to_string()));
        }
        
        // Créer le buffer circulaire
        let buffer = Arc::new(LockFreeRingBuffer::new(
            config.buffer_capacity,
            config.max_image_size,
            config.overflow_strategy,
        ));
        
        // Créer les métriques
        let metrics = Arc::new(PipelineMetrics::new());
        
        // Créer le planificateur
        let scheduler = Arc::new(TaskScheduler::new());
        
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            state: Arc::new(RwLock::new(PipelineState::Uninitialized)),
            buffer,
            metrics,
            scheduler,
            acquisition_tasks: Arc::new(Mutex::new(Vec::new())),
            processing_tasks: Arc::new(Mutex::new(Vec::new())),
            monitoring_task: Arc::new(Mutex::new(None)),
            cameras: Arc::new(Mutex::new(Vec::new())),
            sequence_counter: Arc::new(Mutex::new(0)),
            processor_callbacks: Arc::new(Mutex::new(Vec::new())),
            last_acquisition: Arc::new(Mutex::new(None)),
            last_processing: Arc::new(Mutex::new(None)),
            stats: Arc::new(RwLock::new(PipelineStats::default())),
        })
    }
    
    /// Initialise le pipeline
    pub fn initialize(&self) -> Result<(), PipelineError> {
        // Vérifier l'état actuel
        {
            let state = self.state.read().unwrap();
            if *state != PipelineState::Uninitialized {
                return Err(PipelineError::InitError("Le pipeline est déjà initialisé".to_string()));
            }
        }
        
        // Initialiser les caméras
        self.initialize_cameras()?;
        
        // Créer les tâches d'acquisition
        self.create_acquisition_tasks()?;
        
        // Créer les tâches de traitement
        self.create_processing_tasks()?;
        
        // Créer la tâche de surveillance
        self.create_monitoring_task()?;
        
        // Mettre à jour l'état
        {
            let mut state = self.state.write().unwrap();
            *state = PipelineState::Ready;
        }
        
        info!("Pipeline d'acquisition initialisé avec succès");
        
        Ok(())
    }
    
    /// Initialise les caméras
    fn initialize_cameras(&self) -> Result<(), PipelineError> {
        let config = self.config.read().unwrap();
        let mut cameras = self.cameras.lock().unwrap();
        
        // Énumérer les caméras disponibles
        let available_cameras = CameraFactory::enumerate();
        info!("Caméras disponibles: {:?}", available_cameras);
        
        if available_cameras.is_empty() {
            warn!("Aucune caméra disponible, utilisation du simulateur");
            
            // Créer une caméra simulée
            let camera = CameraFactory::create("simulator", "simulated_camera")
                .map_err(|e| PipelineError::CameraError(e))?;
            
            // Configurer la caméra
            let camera_config = CameraConfig {
                id: "simulated_camera".to_string(),
                pixel_format: heimdall_camera::PixelFormat::RGB8,
                width: 1280,
                height: 1024,
                frame_rate: 100.0, // 100 FPS pour simuler 100 000 bouteilles/heure
                exposure_time_us: 5000,
                gain_db: 0.0,
                trigger_mode: heimdall_camera::TriggerMode::Continuous,
                vendor_params: std::collections::HashMap::new(),
            };
            
            // Initialiser la caméra de manière asynchrone
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                camera.initialize(camera_config).await
            }).map_err(|e| PipelineError::CameraError(e))?;
            
            cameras.push(camera);
        } else {
            // Utiliser les caméras réelles
            for (camera_type, camera_id) in available_cameras {
                if cameras.len() >= config.acquisition_threads {
                    break;
                }
                
                info!("Initialisation de la caméra {} ({})", camera_id, camera_type);
                
                // Créer la caméra
                let camera = CameraFactory::create(&camera_type, &camera_id)
                    .map_err(|e| PipelineError::CameraError(e))?;
                
                // Configurer la caméra
                let camera_config = CameraConfig {
                    id: camera_id.clone(),
                    pixel_format: heimdall_camera::PixelFormat::RGB8,
                    width: 1280,
                    height: 1024,
                    frame_rate: 100.0, // 100 FPS pour simuler 100 000 bouteilles/heure
                    exposure_time_us: 5000,
                    gain_db: 0.0,
                    trigger_mode: heimdall_camera::TriggerMode::Continuous,
                    vendor_params: std::collections::HashMap::new(),
                };
                
                // Initialiser la caméra de manière asynchrone
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    camera.initialize(camera_config).await
                }).map_err(|e| PipelineError::CameraError(e))?;
                
                cameras.push(camera);
            }
        }
        
        info!("{} caméras initialisées", cameras.len());
        
        Ok(())
    }
    
    /// Crée les tâches d'acquisition
    fn create_acquisition_tasks(&self) -> Result<(), PipelineError> {
        let config = self.config.read().unwrap();
        let cameras = self.cameras.lock().unwrap();
        let mut acquisition_tasks = self.acquisition_tasks.lock().unwrap();
        
        // Créer une tâche pour chaque caméra
        for (camera_index, _) in cameras.iter().enumerate() {
            // Créer la configuration temps réel
            let rt_config = RtConfig {
                priority: config.acquisition_priority,
                period_ms: 0, // Apériodique (acquisition continue)
                deadline_ms: 0, // Pas de deadline stricte
                cpu_affinity: if camera_index < config.acquisition_cpu_affinity.len() {
                    vec![config.acquisition_cpu_affinity[camera_index]]
                } else {
                    vec![]
                },
                lock_memory: true,
                use_rt_scheduler: true,
            };
            
            // Créer le contexte de la tâche
            let task_context = TaskContext::new(
                TaskType::Acquisition,
                rt_config,
                0, // Apériodique
            );
            
            // Cloner les références nécessaires
            let buffer = self.buffer.clone();
            let metrics = self.metrics.clone();
            let sequence_counter = self.sequence_counter.clone();
            let last_acquisition = self.last_acquisition.clone();
            let cameras = self.cameras.clone();
            let camera_idx = camera_index;
            
            // Créer la tâche d'acquisition
            let task = self.scheduler.spawn_rt_task(task_context, move |ctx| {
                // Obtenir la caméra
                let mut cameras_guard = cameras.lock().unwrap();
                let camera = &mut cameras_guard[camera_idx];
                
                // Acquérir une image de manière asynchrone
                let rt = tokio::runtime::Runtime::new().unwrap();
                let frame_result = rt.block_on(async {
                    camera.acquire_frame().await
                });
                
                match frame_result {
                    Ok(frame) => {
                        // Horodatage de l'acquisition
                        let acquisition_time = Timestamp::now();
                        
                        // Incrémenter le compteur de séquence
                        let sequence = {
                            let mut counter = sequence_counter.lock().unwrap();
                            *counter += 1;
                            *counter
                        };
                        
                        // Réserver un slot dans le buffer
                        match buffer.reserve_write_slot() {
                            Ok((slot_index, slot)) => {
                                // Copier les données de l'image
                                let data_len = frame.data.len().min(slot.data.len());
                                slot.data[0..data_len].copy_from_slice(&frame.data[0..data_len]);
                                slot.size = data_len;
                                slot.width = frame.width;
                                slot.height = frame.height;
                                slot.format = match frame.pixel_format {
                                    heimdall_camera::PixelFormat::Mono8 => 0,
                                    heimdall_camera::PixelFormat::Mono16 => 1,
                                    heimdall_camera::PixelFormat::RGB8 => 2,
                                    heimdall_camera::PixelFormat::BGR8 => 3,
                                    heimdall_camera::PixelFormat::RGBA8 => 4,
                                    heimdall_camera::PixelFormat::BGRA8 => 5,
                                    _ => 0,
                                };
                                slot.timestamp = acquisition_time;
                                
                                // Finaliser l'écriture
                                buffer.commit_write(slot_index, sequence);
                                
                                // Mettre à jour le dernier horodatage d'acquisition
                                *last_acquisition.lock().unwrap() = Some(acquisition_time);
                                
                                // Enregistrer les métriques
                                let latency = if let Some(frame_time) = Timestamp::from_system_time(frame.timestamp).diff_millis(&acquisition_time).checked_abs() {
                                    frame_time as f64
                                } else {
                                    0.0
                                };
                                
                                metrics.record_acquisition(latency);
                                metrics.update_buffer_usage(buffer.len(), buffer.capacity());
                            },
                            Err(e) => {
                                warn!("Impossible de réserver un slot dans le buffer: {}", e);
                                metrics.record_dropped_frame();
                                metrics.record_buffer_overflow();
                            }
                        }
                    },
                    Err(e) => {
                        error!("Erreur lors de l'acquisition d'image: {}", e);
                        return Err(PipelineError::AcquisitionError(format!("Erreur d'acquisition: {}", e)));
                    }
                }
                
                Ok(())
            })?;
            
            acquisition_tasks.push(task);
        }
        
        info!("{} tâches d'acquisition créées", acquisition_tasks.len());
        
        Ok(())
    }
    
    /// Crée les tâches de traitement
    fn create_processing_tasks(&self) -> Result<(), PipelineError> {
        let config = self.config.read().unwrap();
        let mut processing_tasks = self.processing_tasks.lock().unwrap();
        
        // Créer les tâches de traitement
        for i in 0..config.processing_threads {
            // Créer la configuration temps réel
            let rt_config = RtConfig {
                priority: config.processing_priority,
                period_ms: 0, // Apériodique (traitement continu)
                deadline_ms: 0, // Pas de deadline stricte
                cpu_affinity: if i < config.processing_cpu_affinity.len() {
                    vec![config.processing_cpu_affinity[i]]
                } else {
                    vec![]
                },
                lock_memory: true,
                use_rt_scheduler: true,
            };
            
            // Créer le contexte de la tâche
            let task_context = TaskContext::new(
                TaskType::Processing,
                rt_config,
                0, // Apériodique
            );
            
            // Cloner les références nécessaires
            let buffer = self.buffer.clone();
            let metrics = self.metrics.clone();
            let processor_callbacks = self.processor_callbacks.clone();
            let last_processing = self.last_processing.clone();
            let enable_auto_recovery = config.enable_auto_recovery;
            
            // Créer la tâche de traitement
            let task = self.scheduler.spawn_rt_task(task_context, move |ctx| {
                // Lire une image du buffer
                match buffer.read_slot() {
                    Ok((slot_index, slot)) => {
                        // Créer l'image du pipeline
                        let image = PipelineImage::from_buffer_slot(slot);
                        
                        // Horodatage du traitement
                        let processing_time = Timestamp::now();
                        
                        // Mettre à jour le dernier horodatage de traitement
                        *last_processing.lock().unwrap() = Some(processing_time);
                        
                        // Calculer la latence
                        let latency = image.timestamp.diff_millis(&processing_time).abs() as f64;
                        
                        // Appeler les callbacks de traitement
                        let callbacks = processor_callbacks.lock().unwrap();
                        for callback in callbacks.iter() {
                            if let Err(e) = callback(&image) {
                                error!("Erreur lors du traitement d'image: {}", e);
                            }
                        }
                        
                        // Finaliser la lecture
                        buffer.commit_read(slot_index);
                        
                        // Enregistrer les métriques
                        metrics.record_processing(latency);
                        metrics.update_buffer_usage(buffer.len(), buffer.capacity());
                    },
                    Err(e) => {
                        // Buffer vide, attendre un peu
                        thread::sleep(Duration::from_millis(1));
                        
                        // Vérifier si nous devons récupérer d'une désynchronisation
                        if enable_auto_recovery {
                            let last_acq = last_processing.lock().unwrap();
                            let last_proc = last_processing.lock().unwrap();
                            
                            if let (Some(acq), Some(proc)) = (last_acq.as_ref(), last_proc.as_ref()) {
                                let diff = acq.diff_millis(proc).abs() as u64;
                                
                                // Si la différence est trop grande, réinitialiser le buffer
                                if diff > 1000 { // 1 seconde
                                    warn!("Désynchronisation détectée ({}ms), réinitialisation du buffer", diff);
                                    buffer.reset();
                                    metrics.record_desync();
                                    metrics.record_recovery();
                                }
                            }
                        }
                    }
                }
                
                Ok(())
            })?;
            
            processing_tasks.push(task);
        }
        
        info!("{} tâches de traitement créées", processing_tasks.len());
        
        Ok(())
    }
    
    /// Crée la tâche de surveillance
    fn create_monitoring_task(&self) -> Result<(), PipelineError> {
        let config = self.config.read().unwrap();
        let mut monitoring_task = self.monitoring_task.lock().unwrap();
        
        // Créer la configuration temps réel
        let rt_config = RtConfig {
            priority: RtPriority::Normal,
            period_ms: config.metrics_interval_ms,
            deadline_ms: 0, // Pas de deadline stricte
            cpu_affinity: vec![],
            lock_memory: false,
            use_rt_scheduler: false,
        };
        
        // Créer le contexte de la tâche
        let task_context = TaskContext::new(
            TaskType::Monitoring,
            rt_config,
            config.metrics_interval_ms,
        );
        
        // Cloner les références nécessaires
        let buffer = self.buffer.clone();
        let metrics = self.metrics.clone();
        let stats = self.stats.clone();
        
        // Créer la tâche de surveillance
        let task = self.scheduler.spawn_rt_task(task_context, move |ctx| {
            // Mettre à jour les statistiques
            let current_stats = metrics.get_stats();
            
            // Mettre à jour l'utilisation du buffer
            metrics.update_buffer_usage(buffer.len(), buffer.capacity());
            
            // Mettre à jour les statistiques globales
            {
                let mut stats_guard = stats.write().unwrap();
                *stats_guard = current_stats;
            }
            
            // Afficher les statistiques
            debug!(
                "Pipeline stats: {} acq, {} proc, {} drop, {:.1}% buffer, {:.1} FPS",
                current_stats.total_frames_acquired,
                current_stats.total_frames_processed,
                current_stats.total_frames_dropped,
                current_stats.avg_buffer_usage,
                current_stats.avg_acquisition_rate,
            );
            
            Ok(())
        })?;
        
        *monitoring_task = Some(task);
        
        info!("Tâche de surveillance créée");
        
        Ok(())
    }
    
    /// Démarre le pipeline
    pub fn start(&self) -> Result<(), PipelineError> {
        // Vérifier l'état actuel
        {
            let state = self.state.read().unwrap();
            if *state != PipelineState::Ready && *state != PipelineState::Paused {
                return Err(PipelineError::InitError(format!("Le pipeline n'est pas prêt à démarrer (état: {:?})", state)));
            }
        }
        
        // Démarrer les caméras
        {
            let mut cameras = self.cameras.lock().unwrap();
            
            for camera in cameras.iter_mut() {
                // Démarrer l'acquisition de manière asynchrone
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    camera.start_acquisition().await
                }).map_err(|e| PipelineError::CameraError(e))?;
            }
        }
        
        // Démarrer toutes les tâches
        self.scheduler.start_all()?;
        
        // Mettre à jour l'état
        {
            let mut state = self.state.write().unwrap();
            *state = PipelineState::Running;
        }
        
        info!("Pipeline d'acquisition démarré");
        
        Ok(())
    }
    
    /// Met en pause le pipeline
    pub fn pause(&self) -> Result<(), PipelineError> {
        // Vérifier l'état actuel
        {
            let state = self.state.read().unwrap();
            if *state != PipelineState::Running {
                return Err(PipelineError::InitError(format!("Le pipeline n'est pas en cours d'exécution (état: {:?})", state)));
            }
        }
        
        // Arrêter les caméras
        {
            let mut cameras = self.cameras.lock().unwrap();
            
            for camera in cameras.iter_mut() {
                // Arrêter l'acquisition de manière asynchrone
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    camera.stop_acquisition().await
                }).map_err(|e| PipelineError::CameraError(e))?;
            }
        }
        
        // Mettre à jour l'état
        {
            let mut state = self.state.write().unwrap();
            *state = PipelineState::Paused;
        }
        
        info!("Pipeline d'acquisition mis en pause");
        
        Ok(())
    }
    
    /// Arrête le pipeline
    pub fn stop(&self) -> Result<(), PipelineError> {
        // Vérifier l'état actuel
        {
            let state = self.state.read().unwrap();
            if *state != PipelineState::Running && *state != PipelineState::Paused {
                return Err(PipelineError::InitError(format!("Le pipeline n'est pas en cours d'exécution ou en pause (état: {:?})", state)));
            }
        }
        
        // Arrêter les caméras
        {
            let mut cameras = self.cameras.lock().unwrap();
            
            for camera in cameras.iter_mut() {
                // Arrêter l'acquisition de manière asynchrone
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    camera.stop_acquisition().await
                }).map_err(|e| PipelineError::CameraError(e))?;
            }
        }
        
        // Arrêter toutes les tâches
        self.scheduler.stop_all()?;
        
        // Attendre que toutes les tâches se terminent
        self.scheduler.join_all()?;
        
        // Mettre à jour l'état
        {
            let mut state = self.state.write().unwrap();
            *state = PipelineState::Stopped;
        }
        
        info!("Pipeline d'acquisition arrêté");
        
        Ok(())
    }
    
    /// Réinitialise le pipeline
    pub fn reset(&self) -> Result<(), PipelineError> {
        // Vérifier l'état actuel
        {
            let state = self.state.read().unwrap();
            if *state == PipelineState::Running {
                return Err(PipelineError::InitError("Le pipeline est en cours d'exécution".to_string()));
            }
        }
        
        // Réinitialiser le buffer
        self.buffer.reset();
        
        // Réinitialiser les métriques
        self.metrics.reset();
        
        // Réinitialiser le compteur de séquence
        {
            let mut counter = self.sequence_counter.lock().unwrap();
            *counter = 0;
        }
        
        // Réinitialiser les horodatages
        {
            let mut last_acq = self.last_acquisition.lock().unwrap();
            *last_acq = None;
        }
        
        {
            let mut last_proc = self.last_processing.lock().unwrap();
            *last_proc = None;
        }
        
        // Mettre à jour l'état
        {
            let mut state = self.state.write().unwrap();
            *state = PipelineState::Ready;
        }
        
        info!("Pipeline d'acquisition réinitialisé");
        
        Ok(())
    }
    
    /// Ajoute un callback de traitement d'image
    pub fn add_processor_callback<F>(&self, callback: F) -> Result<(), PipelineError>
    where
        F: Fn(&PipelineImage) -> Result<(), PipelineError> + Send + Sync + 'static,
    {
        let mut callbacks = self.processor_callbacks.lock().unwrap();
        callbacks.push(Box::new(callback));
        
        Ok(())
    }
    
    /// Obtient l'état actuel du pipeline
    pub fn get_state(&self) -> PipelineState {
        *self.state.read().unwrap()
    }
    
    /// Obtient les statistiques du pipeline
    pub fn get_stats(&self) -> PipelineStats {
        self.stats.read().unwrap().clone()
    }
    
    /// Obtient la configuration du pipeline
    pub fn get_config(&self) -> PipelineConfig {
        self.config.read().unwrap().clone()
    }
    
    /// Définit la configuration du pipeline
    pub fn set_config(&self, config: PipelineConfig) -> Result<(), PipelineError> {
        // Vérifier l'état actuel
        {
            let state = self.state.read().unwrap();
            if *state == PipelineState::Running {
                return Err(PipelineError::ConfigError("Impossible de modifier la configuration pendant l'exécution".to_string()));
            }
        }
        
        // Mettre à jour la configuration
        {
            let mut config_guard = self.config.write().unwrap();
            *config_guard = config;
        }
        
        Ok(())
    }
}

/// Tests unitaires
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    #[test]
    fn test_pipeline_image() {
        // Créer un frame de caméra
        let frame = CameraFrame {
            data: vec![0, 1, 2, 3, 4],
            width: 10,
            height: 20,
            pixel_format: heimdall_camera::PixelFormat::RGB8,
            timestamp: SystemTime::now(),
            frame_id: 42,
            metadata: std::collections::HashMap::new(),
        };
        
        // Convertir en image de pipeline
        let image = PipelineImage::from_camera_frame(&frame, 42);
        
        // Vérifier les valeurs
        assert_eq!(image.data, vec![0, 1, 2, 3, 4]);
        assert_eq!(image.width, 10);
        assert_eq!(image.height, 20);
        assert_eq!(image.format, 2); // RGB8
        assert_eq!(image.sequence, 42);
    }
    
    #[test]
    fn test_pipeline_basic() {
        // Créer une configuration
        let config = PipelineConfig {
            buffer_capacity: 10,
            max_image_size: 1024,
            acquisition_threads: 1,
            processing_threads: 1,
            acquisition_priority: RtPriority::High,
            processing_priority: RtPriority::Normal,
            acquisition_cpu_affinity: vec![0],
            processing_cpu_affinity: vec![1],
            metrics_interval_ms: 100,
            enable_auto_recovery: true,
            max_wait_time_ms: 100,
            overflow_strategy: OverflowStrategy::DropOldest,
        };
        
        // Créer le pipeline
        let pipeline = AcquisitionPipeline::new(config).unwrap();
        
        // Vérifier l'état initial
        assert_eq!(pipeline.get_state(), PipelineState::Uninitialized);
        
        // Ajouter un callback de traitement
        let processed_count = Arc::new(AtomicUsize::new(0));
        let processed_count_clone = processed_count.clone();
        
        pipeline.add_processor_callback(move |image| {
            processed_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }).unwrap();
    }
}
//! # Module d'acquisition d'images pour caméras GigE Vision
//! 
//! Ce module fournit une interface complète pour l'acquisition d'images à partir de caméras
//! GigE Vision dans un contexte d'inspection de bouteilles à haute cadence.
//! 
//! ## Caractéristiques principales
//! 
//! - Support pour caméras GigE Vision 2MP en niveaux de gris
//! - Acquisition synchronisée de 4 caméras avec latence < 5ms
//! - Mécanismes de synchronisation hardware/software
//! - Gestion robuste des erreurs et stratégies de reprise
//! - Optimisation des paramètres de caméra
//! - Métriques et diagnostics
//! 
//! ## Exemple d'utilisation
//! 
//! ```rust
//! use heimdall_gige::{GigESystem, CameraConfig, SyncMode};
//! use std::time::Duration;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialiser le système GigE
//!     let mut gige = GigESystem::new()?;
//!     
//!     // Découvrir les caméras disponibles
//!     let cameras = gige.discover_cameras().await?;
//!     println!("Caméras découvertes: {:?}", cameras);
//!     
//!     // Configurer et initialiser les caméras
//!     gige.configure_cameras(SyncMode::Hardware).await?;
//!     
//!     // Démarrer l'acquisition
//!     gige.start_acquisition().await?;
//!     
//!     // Acquérir des images
//!     for _ in 0..10 {
//!         let frames = gige.acquire_frames().await?;
//!         println!("Images acquises: {}", frames.len());
//!         
//!         // Traiter les images...
//!     }
//!     
//!     // Arrêter l'acquisition
//!     gige.stop_acquisition().await?;
//!     
//!     Ok(())
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use backoff::ExponentialBackoff;
use futures::future::{join_all, FutureExt};
use log::{debug, error, info, trace, warn};
use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, Semaphore};
use tokio::time;

pub mod camera;
pub mod config;
pub mod diagnostics;
pub mod error;
pub mod frame;
pub mod sync;
pub mod utils;

// Re-exports
pub use camera::{GigECamera, CameraInfo, CameraCapabilities};
pub use config::{CameraConfig, SystemConfig};
pub use error::GigEError;
pub use frame::{Frame, FrameMetadata, FrameSet};
pub use sync::{SyncManager, SyncMode, TriggerSource};

/// Version du module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Système d'acquisition GigE Vision
/// 
/// Cette structure représente le système complet d'acquisition d'images
/// à partir de caméras GigE Vision. Elle gère la découverte, la configuration,
/// la synchronisation et l'acquisition d'images à partir de plusieurs caméras.
#[derive(Debug)]
pub struct GigESystem {
    /// Configuration du système
    config: SystemConfig,
    
    /// Caméras connectées
    cameras: HashMap<String, Arc<Mutex<GigECamera>>>,
    
    /// Gestionnaire de synchronisation
    sync_manager: Arc<RwLock<SyncManager>>,
    
    /// État d'acquisition
    is_acquiring: bool,
    
    /// Canal pour les images acquises
    frame_channel: Option<(mpsc::Sender<FrameSet>, mpsc::Receiver<FrameSet>)>,
    
    /// Compteur de trames
    frame_counter: Arc<Mutex<u64>>,
    
    /// Horodatage de démarrage de l'acquisition
    acquisition_start_time: Option<Instant>,
    
    /// Sémaphore pour limiter les acquisitions parallèles
    acquisition_semaphore: Arc<Semaphore>,
}

impl GigESystem {
    /// Crée une nouvelle instance du système GigE
    pub fn new() -> Result<Self> {
        info!("Initialisation du système GigE Vision");
        
        // Initialiser la bibliothèque Aravis
        camera::init_aravis()?;
        
        let config = SystemConfig::default();
        
        Ok(Self {
            config,
            cameras: HashMap::new(),
            sync_manager: Arc::new(RwLock::new(SyncManager::new())),
            is_acquiring: false,
            frame_channel: None,
            frame_counter: Arc::new(Mutex::new(0)),
            acquisition_start_time: None,
            acquisition_semaphore: Arc::new(Semaphore::new(4)), // Limiter à 4 acquisitions parallèles
        })
    }
    
    /// Crée une nouvelle instance avec une configuration spécifique
    pub fn with_config(config: SystemConfig) -> Result<Self> {
        info!("Initialisation du système GigE Vision avec configuration personnalisée");
        
        // Initialiser la bibliothèque Aravis
        camera::init_aravis()?;
        
        Ok(Self {
            config,
            cameras: HashMap::new(),
            sync_manager: Arc::new(RwLock::new(SyncManager::new())),
            is_acquiring: false,
            frame_channel: None,
            frame_counter: Arc::new(Mutex::new(0)),
            acquisition_start_time: None,
            acquisition_semaphore: Arc::new(Semaphore::new(4)), // Limiter à 4 acquisitions parallèles
        })
    }
    
    /// Découvre les caméras GigE disponibles sur le réseau
    pub async fn discover_cameras(&mut self) -> Result<Vec<CameraInfo>> {
        info!("Découverte des caméras GigE Vision");
        
        // Mesurer le temps de découverte
        let start_time = Instant::now();
        
        // Découvrir les caméras
        let camera_infos = camera::discover_cameras().await?;
        
        // Enregistrer la métrique de temps de découverte
        let discovery_time = start_time.elapsed();
        histogram!("gige.discovery.time", discovery_time.as_millis() as f64);
        gauge!("gige.cameras.count", camera_infos.len() as f64);
        
        info!("Découverte terminée: {} caméras trouvées en {:?}", camera_infos.len(), discovery_time);
        
        Ok(camera_infos)
    }
    
    /// Configure et initialise les caméras avec le mode de synchronisation spécifié
    pub async fn configure_cameras(&mut self, sync_mode: SyncMode) -> Result<()> {
        info!("Configuration des caméras en mode {:?}", sync_mode);
        
        // Découvrir les caméras si ce n'est pas déjà fait
        let camera_infos = if self.cameras.is_empty() {
            self.discover_cameras().await?
        } else {
            Vec::new()
        };
        
        // Filtrer les caméras selon les critères (2MP, niveaux de gris)
        let filtered_cameras = camera_infos.into_iter()
            .filter(|info| {
                // Vérifier si la caméra correspond aux critères
                let is_grayscale = info.capabilities.pixel_formats.contains(&heimdall_camera::PixelFormat::Mono8);
                let is_2mp = info.capabilities.max_width * info.capabilities.max_height >= 2_000_000;
                
                is_grayscale && is_2mp
            })
            .collect::<Vec<_>>();
        
        // Vérifier qu'on a au moins une caméra
        if filtered_cameras.is_empty() {
            return Err(anyhow!("Aucune caméra GigE Vision 2MP en niveaux de gris trouvée"));
        }
        
        // Limiter à 4 caméras maximum
        let cameras_to_use = filtered_cameras.into_iter()
            .take(4)
            .collect::<Vec<_>>();
        
        info!("Configuration de {} caméras", cameras_to_use.len());
        
        // Configurer le gestionnaire de synchronisation
        {
            let mut sync_manager = self.sync_manager.write().unwrap();
            sync_manager.set_mode(sync_mode);
            sync_manager.set_camera_count(cameras_to_use.len());
        }
        
        // Initialiser les caméras en parallèle
        let mut init_futures = Vec::new();
        
        for camera_info in cameras_to_use {
            let camera_id = camera_info.id.clone();
            let sync_manager = Arc::clone(&self.sync_manager);
            let config = self.config.clone();
            
            let future = async move {
                // Créer et initialiser la caméra
                let mut camera = GigECamera::new(&camera_id, camera_info)?;
                
                // Appliquer la configuration
                let camera_config = CameraConfig {
                    pixel_format: heimdall_camera::PixelFormat::Mono8,
                    width: 1920,
                    height: 1080,
                    frame_rate: config.frame_rate,
                    exposure_time_us: config.exposure_time_us,
                    gain_db: config.gain_db,
                    trigger_mode: match sync_mode {
                        SyncMode::Software => heimdall_camera::TriggerMode::Software,
                        SyncMode::Hardware => heimdall_camera::TriggerMode::Hardware,
                        SyncMode::Freerun => heimdall_camera::TriggerMode::Continuous,
                    },
                    roi_enabled: config.roi_enabled,
                    roi_x: config.roi_x,
                    roi_y: config.roi_y,
                    roi_width: config.roi_width,
                    roi_height: config.roi_height,
                    packet_size: config.packet_size,
                    packet_delay: config.packet_delay,
                    buffer_count: config.buffer_count,
                };
                
                camera.configure(camera_config).await?;
                
                // Optimiser les paramètres réseau
                camera.optimize_network_parameters().await?;
                
                // Configurer la synchronisation
                if sync_mode == SyncMode::Hardware {
                    camera.configure_hardware_sync(&sync_manager.read().unwrap()).await?;
                }
                
                Ok::<(String, GigECamera), anyhow::Error>((camera_id, camera))
            };
            
            init_futures.push(future);
        }
        
        // Attendre que toutes les initialisations soient terminées
        let results = join_all(init_futures).await;
        
        // Traiter les résultats
        for result in results {
            match result {
                Ok((camera_id, camera)) => {
                    self.cameras.insert(camera_id.clone(), Arc::new(Mutex::new(camera)));
                    info!("Caméra {} configurée avec succès", camera_id);
                },
                Err(e) => {
                    error!("Erreur lors de la configuration de la caméra: {}", e);
                    return Err(e);
                }
            }
        }
        
        // Vérifier qu'on a au moins une caméra configurée
        if self.cameras.is_empty() {
            return Err(anyhow!("Aucune caméra n'a pu être configurée"));
        }
        
        info!("{} caméras configurées avec succès", self.cameras.len());
        
        Ok(())
    }
    
    /// Démarre l'acquisition d'images
    pub async fn start_acquisition(&mut self) -> Result<()> {
        if self.is_acquiring {
            warn!("L'acquisition est déjà en cours");
            return Ok(());
        }
        
        info!("Démarrage de l'acquisition");
        
        // Vérifier qu'on a des caméras configurées
        if self.cameras.is_empty() {
            return Err(anyhow!("Aucune caméra configurée"));
        }
        
        // Créer le canal pour les images
        let (tx, rx) = mpsc::channel(32); // Buffer de 32 ensembles d'images
        self.frame_channel = Some((tx, rx));
        
        // Réinitialiser le compteur de trames
        {
            let mut counter = self.frame_counter.lock().unwrap();
            *counter = 0;
        }
        
        // Démarrer l'acquisition sur toutes les caméras en parallèle
        let mut start_futures = Vec::new();
        
        for (camera_id, camera) in &self.cameras {
            let camera_id = camera_id.clone();
            let camera = Arc::clone(camera);
            
            let future = async move {
                let mut camera = camera.lock().unwrap();
                camera.start_acquisition().await
                    .with_context(|| format!("Erreur lors du démarrage de l'acquisition sur la caméra {}", camera_id))
            };
            
            start_futures.push(future);
        }
        
        // Attendre que toutes les caméras soient démarrées
        let results = join_all(start_futures).await;
        
        // Vérifier les résultats
        for result in results {
            if let Err(e) = result {
                // Arrêter les caméras déjà démarrées
                self.stop_acquisition().await?;
                return Err(e);
            }
        }
        
        // Démarrer le gestionnaire de synchronisation
        {
            let mut sync_manager = self.sync_manager.write().unwrap();
            sync_manager.start()?;
        }
        
        self.is_acquiring = true;
        self.acquisition_start_time = Some(Instant::now());
        
        info!("Acquisition démarrée avec succès");
        
        // Démarrer la tâche d'acquisition en arrière-plan si en mode continu
        if self.sync_manager.read().unwrap().get_mode() == SyncMode::Freerun {
            self.start_background_acquisition()?;
        }
        
        Ok(())
    }
    
    /// Démarre l'acquisition en arrière-plan (mode continu)
    fn start_background_acquisition(&self) -> Result<()> {
        info!("Démarrage de l'acquisition en arrière-plan");
        
        // Cloner les références nécessaires
        let cameras = self.cameras.clone();
        let frame_sender = self.frame_channel.as_ref().unwrap().0.clone();
        let frame_counter = Arc::clone(&self.frame_counter);
        let acquisition_semaphore = Arc::clone(&self.acquisition_semaphore);
        
        // Démarrer la tâche d'acquisition
        tokio::spawn(async move {
            loop {
                // Acquérir un permit du sémaphore
                let _permit = acquisition_semaphore.acquire().await.unwrap();
                
                // Acquérir les images de toutes les caméras en parallèle
                let mut acquire_futures = Vec::new();
                
                for (camera_id, camera) in &cameras {
                    let camera_id = camera_id.clone();
                    let camera = Arc::clone(camera);
                    
                    let future = async move {
                        let backoff = ExponentialBackoff {
                            max_elapsed_time: Some(Duration::from_millis(100)),
                            ..Default::default()
                        };
                        
                        let result = backoff::future::retry(backoff, || async {
                            let mut camera = camera.lock().unwrap();
                            camera.acquire_frame().await
                                .map_err(|e| {
                                    warn!("Erreur d'acquisition sur caméra {}: {}", camera_id, e);
                                    backoff::Error::transient(e)
                                })
                        }).await;
                        
                        result.map(|frame| (camera_id, frame))
                    };
                    
                    acquire_futures.push(future);
                }
                
                // Attendre que toutes les acquisitions soient terminées
                let results = join_all(acquire_futures).await;
                
                // Traiter les résultats
                let mut frames = HashMap::new();
                let mut success = true;
                
                for result in results {
                    match result {
                        Ok((camera_id, frame)) => {
                            frames.insert(camera_id, frame);
                        },
                        Err(e) => {
                            error!("Erreur lors de l'acquisition d'image: {}", e);
                            success = false;
                            break;
                        }
                    }
                }
                
                // Si toutes les acquisitions ont réussi, envoyer les images
                if success && !frames.is_empty() {
                    // Incrémenter le compteur de trames
                    let frame_id = {
                        let mut counter = frame_counter.lock().unwrap();
                        *counter += 1;
                        *counter
                    };
                    
                    // Créer l'ensemble d'images
                    let frame_set = FrameSet {
                        frames,
                        timestamp: SystemTime::now(),
                        frame_id,
                    };
                    
                    // Envoyer l'ensemble d'images
                    if let Err(e) = frame_sender.send(frame_set).await {
                        error!("Erreur lors de l'envoi des images: {}", e);
                        break;
                    }
                    
                    // Incrémenter le compteur de métriques
                    counter!("gige.frames.acquired", 1);
                }
                
                // Petite pause pour éviter de surcharger le CPU
                time::sleep(Duration::from_micros(100)).await;
            }
        });
        
        Ok(())
    }
    
    /// Arrête l'acquisition d'images
    pub async fn stop_acquisition(&mut self) -> Result<()> {
        if !self.is_acquiring {
            warn!("L'acquisition n'est pas en cours");
            return Ok(());
        }
        
        info!("Arrêt de l'acquisition");
        
        // Arrêter le gestionnaire de synchronisation
        {
            let mut sync_manager = self.sync_manager.write().unwrap();
            sync_manager.stop()?;
        }
        
        // Arrêter l'acquisition sur toutes les caméras en parallèle
        let mut stop_futures = Vec::new();
        
        for (camera_id, camera) in &self.cameras {
            let camera_id = camera_id.clone();
            let camera = Arc::clone(camera);
            
            let future = async move {
                let mut camera = camera.lock().unwrap();
                camera.stop_acquisition().await
                    .with_context(|| format!("Erreur lors de l'arrêt de l'acquisition sur la caméra {}", camera_id))
            };
            
            stop_futures.push(future);
        }
        
        // Attendre que toutes les caméras soient arrêtées
        let results = join_all(stop_futures).await;
        
        // Vérifier les résultats
        for result in results {
            if let Err(e) = result {
                error!("Erreur lors de l'arrêt de l'acquisition: {}", e);
                // Continuer malgré l'erreur
            }
        }
        
        self.is_acquiring = false;
        self.frame_channel = None;
        
        // Calculer les statistiques d'acquisition
        if let Some(start_time) = self.acquisition_start_time {
            let elapsed = start_time.elapsed();
            let frame_count = *self.frame_counter.lock().unwrap();
            
            if elapsed.as_secs() > 0 && frame_count > 0 {
                let fps = frame_count as f64 / elapsed.as_secs_f64();
                info!("Statistiques d'acquisition: {} images en {:?} ({:.2} FPS)", frame_count, elapsed, fps);
            }
        }
        
        self.acquisition_start_time = None;
        
        info!("Acquisition arrêtée avec succès");
        
        Ok(())
    }
    
    /// Acquiert un ensemble d'images de toutes les caméras
    pub async fn acquire_frames(&mut self) -> Result<FrameSet> {
        if !self.is_acquiring {
            return Err(anyhow!("L'acquisition n'est pas en cours"));
        }
        
        trace!("Acquisition d'images");
        
        // Si on est en mode continu, récupérer les images du canal
        if self.sync_manager.read().unwrap().get_mode() == SyncMode::Freerun {
            if let Some((_, ref mut rx)) = self.frame_channel {
                match rx.recv().await {
                    Some(frame_set) => return Ok(frame_set),
                    None => return Err(anyhow!("Le canal d'acquisition est fermé")),
                }
            } else {
                return Err(anyhow!("Canal d'acquisition non initialisé"));
            }
        }
        
        // En mode déclenché, déclencher l'acquisition
        let sync_mode = self.sync_manager.read().unwrap().get_mode();
        
        if sync_mode == SyncMode::Software {
            // Déclencher toutes les caméras en parallèle
            let mut trigger_futures = Vec::new();
            
            for (camera_id, camera) in &self.cameras {
                let camera_id = camera_id.clone();
                let camera = Arc::clone(camera);
                
                let future = async move {
                    let mut camera = camera.lock().unwrap();
                    camera.trigger().await
                        .with_context(|| format!("Erreur lors du déclenchement de la caméra {}", camera_id))
                };
                
                trigger_futures.push(future);
            }
            
            // Attendre que tous les déclenchements soient terminés
            let results = join_all(trigger_futures).await;
            
            // Vérifier les résultats
            for result in results {
                if let Err(e) = result {
                    return Err(e);
                }
            }
        } else if sync_mode == SyncMode::Hardware {
            // En mode hardware, déclencher via le gestionnaire de synchronisation
            let mut sync_manager = self.sync_manager.write().unwrap();
            sync_manager.trigger()?;
        }
        
        // Acquérir les images de toutes les caméras en parallèle
        let mut acquire_futures = Vec::new();
        
        for (camera_id, camera) in &self.cameras {
            let camera_id = camera_id.clone();
            let camera = Arc::clone(camera);
            let acquisition_semaphore = Arc::clone(&self.acquisition_semaphore);
            
            let future = async move {
                // Acquérir un permit du sémaphore
                let _permit = acquisition_semaphore.acquire().await;
                
                let backoff = ExponentialBackoff {
                    max_elapsed_time: Some(Duration::from_millis(100)),
                    ..Default::default()
                };
                
                let result = backoff::future::retry(backoff, || async {
                    let mut camera = camera.lock().unwrap();
                    camera.acquire_frame().await
                        .map_err(|e| {
                            warn!("Erreur d'acquisition sur caméra {}: {}", camera_id, e);
                            backoff::Error::transient(e)
                        })
                }).await;
                
                result.map(|frame| (camera_id, frame))
            };
            
            acquire_futures.push(future);
        }
        
        // Attendre que toutes les acquisitions soient terminées
        let results = join_all(acquire_futures).await;
        
        // Traiter les résultats
        let mut frames = HashMap::new();
        
        for result in results {
            match result {
                Ok((camera_id, frame)) => {
                    frames.insert(camera_id, frame);
                },
                Err(e) => return Err(e),
            }
        }
        
        // Incrémenter le compteur de trames
        let frame_id = {
            let mut counter = self.frame_counter.lock().unwrap();
            *counter += 1;
            *counter
        };
        
        // Créer l'ensemble d'images
        let frame_set = FrameSet {
            frames,
            timestamp: SystemTime::now(),
            frame_id,
        };
        
        // Incrémenter le compteur de métriques
        counter!("gige.frames.acquired", 1);
        
        Ok(frame_set)
    }
    
    /// Optimise les paramètres de caméra pour l'inspection de bouteilles
    pub async fn optimize_camera_parameters(&mut self) -> Result<()> {
        info!("Optimisation des paramètres de caméra pour l'inspection de bouteilles");
        
        // Optimiser chaque caméra en parallèle
        let mut optimize_futures = Vec::new();
        
        for (camera_id, camera) in &self.cameras {
            let camera_id = camera_id.clone();
            let camera = Arc::clone(camera);
            
            let future = async move {
                let mut camera = camera.lock().unwrap();
                camera.optimize_parameters_for_bottle_inspection().await
                    .with_context(|| format!("Erreur lors de l'optimisation des paramètres de la caméra {}", camera_id))
            };
            
            optimize_futures.push(future);
        }
        
        // Attendre que toutes les optimisations soient terminées
        let results = join_all(optimize_futures).await;
        
        // Vérifier les résultats
        for result in results {
            if let Err(e) = result {
                return Err(e);
            }
        }
        
        info!("Paramètres de caméra optimisés avec succès");
        
        Ok(())
    }
    
    /// Exécute un diagnostic complet du système
    pub async fn run_diagnostics(&self) -> Result<diagnostics::DiagnosticReport> {
        info!("Exécution du diagnostic du système");
        
        let mut report = diagnostics::DiagnosticReport::new();
        
        // Vérifier la connectivité réseau
        report.add_test("network_connectivity", diagnostics::test_network_connectivity().await);
        
        // Vérifier les caméras
        for (camera_id, camera) in &self.cameras {
            let camera = camera.lock().unwrap();
            let camera_status = camera.get_status().await?;
            
            report.add_camera_status(camera_id, camera_status);
        }
        
        // Vérifier la synchronisation
        let sync_status = self.sync_manager.read().unwrap().get_status();
        report.add_sync_status(sync_status);
        
        // Vérifier les performances
        if self.is_acquiring && self.acquisition_start_time.is_some() {
            let elapsed = self.acquisition_start_time.unwrap().elapsed();
            let frame_count = *self.frame_counter.lock().unwrap();
            
            if elapsed.as_secs() > 0 && frame_count > 0 {
                let fps = frame_count as f64 / elapsed.as_secs_f64();
                report.add_performance_metric("fps", fps);
            }
        }
        
        info!("Diagnostic terminé: {}", report.summary());
        
        Ok(report)
    }
    
    /// Obtient la liste des caméras connectées
    pub fn get_connected_cameras(&self) -> Vec<String> {
        self.cameras.keys().cloned().collect()
    }
    
    /// Obtient la configuration actuelle
    pub fn get_config(&self) -> &SystemConfig {
        &self.config
    }
    
    /// Définit une nouvelle configuration
    pub fn set_config(&mut self, config: SystemConfig) {
        self.config = config;
    }
    
    /// Vérifie si l'acquisition est en cours
    pub fn is_acquiring(&self) -> bool {
        self.is_acquiring
    }
    
    /// Obtient le nombre d'images acquises
    pub fn get_frame_count(&self) -> u64 {
        *self.frame_counter.lock().unwrap()
    }
}

impl Drop for GigESystem {
    fn drop(&mut self) {
        // Arrêter l'acquisition si elle est en cours
        if self.is_acquiring {
            let _ = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(self.stop_acquisition());
        }
        
        info!("Système GigE Vision libéré");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_system_creation() {
        let system = GigESystem::new();
        assert!(system.is_ok());
    }
    
    #[tokio::test]
    async fn test_system_with_config() {
        let config = SystemConfig {
            frame_rate: 60.0,
            exposure_time_us: 5000,
            gain_db: 2.0,
            ..Default::default()
        };
        
        let system = GigESystem::with_config(config);
        assert!(system.is_ok());
        
        let system = system.unwrap();
        assert_eq!(system.config.frame_rate, 60.0);
        assert_eq!(system.config.exposure_time_us, 5000);
        assert_eq!(system.config.gain_db, 2.0);
    }
}
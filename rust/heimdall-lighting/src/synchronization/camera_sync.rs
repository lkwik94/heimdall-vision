use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use log::{debug, error, info, warn};
use tokio::time;
use crossbeam_channel::{bounded, Receiver, Sender};

use heimdall_camera::{Camera, CameraConfig, CameraError, TriggerMode};

use crate::{
    LightingController, LightingConfig, LightChannelConfig, LightChannelState,
    LightingError, SyncMode, LightingSynchronizer, SyncEvent, SyncStats
};

/// Synchroniseur entre caméra et éclairage
pub struct CameraSynchronizer {
    /// Synchroniseur de base
    synchronizer: LightingSynchronizer,
    
    /// Canal d'envoi d'événements
    event_sender: Sender<SyncEvent>,
    
    /// Configuration de synchronisation
    config: CameraSyncConfig,
}

/// Configuration de synchronisation caméra
#[derive(Debug, Clone)]
pub struct CameraSyncConfig {
    /// ID de la caméra
    pub camera_id: String,
    
    /// Mode de déclenchement
    pub trigger_mode: TriggerMode,
    
    /// Délai avant l'acquisition (µs)
    pub pre_trigger_delay_us: u64,
    
    /// Délai après l'acquisition (µs)
    pub post_trigger_delay_us: u64,
    
    /// Durée d'exposition (µs)
    pub exposure_time_us: u64,
    
    /// Délai de sécurité (µs)
    pub safety_margin_us: u64,
}

impl Default for CameraSyncConfig {
    fn default() -> Self {
        Self {
            camera_id: "default".to_string(),
            trigger_mode: TriggerMode::Software,
            pre_trigger_delay_us: 100,
            post_trigger_delay_us: 100,
            exposure_time_us: 10000,
            safety_margin_us: 500,
        }
    }
}

impl CameraSynchronizer {
    /// Crée un nouveau synchroniseur caméra
    pub fn new(
        controller: Box<dyn LightingController>,
        config: CameraSyncConfig
    ) -> Self {
        let synchronizer = LightingSynchronizer::new(controller, SyncMode::CameraTrigger);
        let event_sender = synchronizer.send_event.clone();
        
        Self {
            synchronizer,
            event_sender,
            config,
        }
    }
    
    /// Démarre la synchronisation
    pub fn start(&mut self) -> Result<(), LightingError> {
        self.synchronizer.start()
    }
    
    /// Arrête la synchronisation
    pub fn stop(&mut self) -> Result<(), LightingError> {
        self.synchronizer.stop()
    }
    
    /// Obtient les statistiques de synchronisation
    pub fn get_stats(&self) -> SyncStats {
        self.synchronizer.get_stats()
    }
    
    /// Envoie un événement de déclenchement de caméra
    pub fn trigger_camera(&self) -> Result<(), LightingError> {
        let event = SyncEvent::CameraTrigger {
            timestamp: Instant::now(),
            camera_id: self.config.camera_id.clone(),
        };
        
        self.event_sender.send(event).map_err(|e| {
            LightingError::SynchronizationError(format!("Erreur lors de l'envoi de l'événement: {}", e))
        })
    }
    
    /// Calcule le délai optimal pour l'éclairage
    pub fn calculate_optimal_timing(&self) -> (u64, u64) {
        // Calculer le délai avant l'illumination
        // Pour une synchronisation optimale, l'éclairage doit être activé juste avant
        // le début de l'exposition, en tenant compte du délai de réponse de l'éclairage
        let pre_delay = if self.config.pre_trigger_delay_us > self.config.safety_margin_us {
            self.config.pre_trigger_delay_us - self.config.safety_margin_us
        } else {
            0
        };
        
        // Calculer la durée d'illumination
        // L'éclairage doit rester actif pendant toute la durée de l'exposition,
        // plus une marge de sécurité
        let duration = self.config.exposure_time_us + 2 * self.config.safety_margin_us;
        
        (pre_delay, duration)
    }
    
    /// Configure les canaux d'éclairage pour une synchronisation optimale
    pub async fn configure_lighting_channels(&self) -> Result<(), LightingError> {
        let (pre_delay, duration) = self.calculate_optimal_timing();
        
        let controller = self.synchronizer.controller.lock().unwrap();
        let config = controller.get_config();
        
        // Mettre à jour la configuration de chaque canal
        for channel in &config.channels {
            controller.set_parameter(&channel.id, "delay_us", &pre_delay.to_string()).await?;
            controller.set_parameter(&channel.id, "duration_us", &duration.to_string()).await?;
        }
        
        Ok(())
    }
    
    /// Enregistre un callback pour les événements de déclenchement de caméra
    pub fn register_camera_trigger_callback<F>(&self, camera: Arc<Mutex<Box<dyn Camera>>>, mut callback: F)
    where
        F: FnMut() -> Result<(), LightingError> + Send + 'static
    {
        let event_sender = self.event_sender.clone();
        let camera_id = self.config.camera_id.clone();
        
        // Démarrer une tâche en arrière-plan pour surveiller les déclenchements de caméra
        tokio::spawn(async move {
            loop {
                // Attendre un court instant pour éviter de surcharger le CPU
                time::sleep(Duration::from_millis(1)).await;
                
                // Vérifier si un déclenchement a eu lieu
                if let Ok(mut camera) = camera.try_lock() {
                    // Dans un système réel, il faudrait un mécanisme pour détecter
                    // les déclenchements de caméra, par exemple en surveillant un signal
                    // matériel ou en interrogeant la caméra
                    
                    // Pour cette démonstration, nous simulons un déclenchement périodique
                    if rand::random::<u8>() < 5 {  // ~2% de chance de déclenchement
                        // Envoyer l'événement de déclenchement
                        let event = SyncEvent::CameraTrigger {
                            timestamp: Instant::now(),
                            camera_id: camera_id.clone(),
                        };
                        
                        if let Err(e) = event_sender.send(event) {
                            error!("Erreur lors de l'envoi de l'événement de déclenchement: {}", e);
                            break;
                        }
                        
                        // Exécuter le callback
                        if let Err(e) = callback() {
                            error!("Erreur lors de l'exécution du callback: {}", e);
                        }
                    }
                }
            }
        });
    }
}
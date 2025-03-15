use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use thiserror::Error;
use log::{debug, error, info, warn};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::time;
use crossbeam_channel::{bounded, Receiver, Sender};

// Modules internes
pub mod controllers;
pub mod synchronization;
pub mod calibration;
pub mod diagnostics;

/// Erreur liée au système d'éclairage
#[derive(Error, Debug)]
pub enum LightingError {
    #[error("Erreur d'initialisation du contrôleur d'éclairage: {0}")]
    InitError(String),

    #[error("Erreur de configuration de l'éclairage: {0}")]
    ConfigError(String),

    #[error("Erreur de communication avec le contrôleur: {0}")]
    CommunicationError(String),

    #[error("Erreur de synchronisation: {0}")]
    SynchronizationError(String),

    #[error("Erreur de calibration: {0}")]
    CalibrationError(String),

    #[error("Contrôleur d'éclairage non trouvé: {0}")]
    NotFound(String),

    #[error("Erreur matérielle: {0}")]
    HardwareError(String),

    #[error("Erreur de temporisation: {0}")]
    TimingError(String),
}

/// Type d'éclairage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightingType {
    /// Éclairage diffus (dôme)
    Diffuse,
    
    /// Rétro-éclairage (backlight)
    Backlight,
    
    /// Éclairage directionnel
    Directional,
    
    /// Éclairage coaxial
    Coaxial,
    
    /// Éclairage structuré (motifs)
    Structured,
    
    /// Éclairage stroboscopique
    Strobe,
}

/// Mode de synchronisation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMode {
    /// Pas de synchronisation (éclairage continu)
    Continuous,
    
    /// Synchronisation avec le déclenchement de la caméra
    CameraTrigger,
    
    /// Synchronisation avec un signal externe
    ExternalTrigger,
    
    /// Synchronisation logicielle
    Software,
}

/// Configuration d'un canal d'éclairage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightChannelConfig {
    /// Identifiant du canal
    pub id: String,
    
    /// Type d'éclairage
    pub lighting_type: LightingType,
    
    /// Intensité (0-100%)
    pub intensity: f64,
    
    /// Durée d'illumination en microsecondes (pour mode stroboscopique)
    pub duration_us: u64,
    
    /// Délai avant illumination en microsecondes (pour mode stroboscopique)
    pub delay_us: u64,
    
    /// Paramètres spécifiques au contrôleur
    pub controller_params: HashMap<String, String>,
}

impl Default for LightChannelConfig {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            lighting_type: LightingType::Diffuse,
            intensity: 50.0,
            duration_us: 1000,
            delay_us: 0,
            controller_params: HashMap::new(),
        }
    }
}

/// Configuration du système d'éclairage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingConfig {
    /// Identifiant du contrôleur
    pub controller_id: String,
    
    /// Type de contrôleur (série, Ethernet, etc.)
    pub controller_type: String,
    
    /// Mode de synchronisation
    pub sync_mode: SyncMode,
    
    /// Canaux d'éclairage
    pub channels: Vec<LightChannelConfig>,
    
    /// Paramètres de connexion
    pub connection_params: HashMap<String, String>,
}

impl Default for LightingConfig {
    fn default() -> Self {
        Self {
            controller_id: "default".to_string(),
            controller_type: "serial".to_string(),
            sync_mode: SyncMode::Continuous,
            channels: vec![LightChannelConfig::default()],
            connection_params: HashMap::new(),
        }
    }
}

/// État d'un canal d'éclairage
#[derive(Debug, Clone)]
pub struct LightChannelState {
    /// Configuration du canal
    pub config: LightChannelConfig,
    
    /// État actuel (allumé/éteint)
    pub is_on: bool,
    
    /// Intensité actuelle (0-100%)
    pub current_intensity: f64,
    
    /// Horodatage de la dernière activation
    pub last_activation: Option<Instant>,
    
    /// Compteur d'activations
    pub activation_count: u64,
    
    /// Durée cumulée d'activation (en ms)
    pub total_on_time_ms: u64,
}

/// Interface de contrôleur d'éclairage
#[async_trait]
pub trait LightingController: Send + Sync {
    /// Initialise le contrôleur avec la configuration spécifiée
    async fn initialize(&mut self, config: LightingConfig) -> Result<(), LightingError>;
    
    /// Active un canal d'éclairage
    async fn turn_on(&mut self, channel_id: &str) -> Result<(), LightingError>;
    
    /// Désactive un canal d'éclairage
    async fn turn_off(&mut self, channel_id: &str) -> Result<(), LightingError>;
    
    /// Définit l'intensité d'un canal d'éclairage
    async fn set_intensity(&mut self, channel_id: &str, intensity: f64) -> Result<(), LightingError>;
    
    /// Active un canal d'éclairage pour une durée spécifiée (mode stroboscopique)
    async fn strobe(&mut self, channel_id: &str, duration_us: u64) -> Result<(), LightingError>;
    
    /// Déclenche tous les canaux configurés en mode stroboscopique
    async fn trigger_all(&mut self) -> Result<(), LightingError>;
    
    /// Obtient l'état actuel d'un canal
    fn get_channel_state(&self, channel_id: &str) -> Option<LightChannelState>;
    
    /// Obtient la configuration actuelle
    fn get_config(&self) -> LightingConfig;
    
    /// Définit un paramètre spécifique
    async fn set_parameter(&mut self, channel_id: &str, name: &str, value: &str) -> Result<(), LightingError>;
    
    /// Obtient un paramètre spécifique
    async fn get_parameter(&self, channel_id: &str, name: &str) -> Result<String, LightingError>;
}

/// Fabrique de contrôleurs d'éclairage
pub struct LightingControllerFactory;

impl LightingControllerFactory {
    /// Crée une nouvelle instance de contrôleur d'éclairage
    pub fn create(controller_type: &str, id: &str) -> Result<Box<dyn LightingController>, LightingError> {
        match controller_type {
            "serial" => {
                info!("Création d'un contrôleur d'éclairage série avec ID: {}", id);
                Ok(Box::new(controllers::serial::SerialLightingController::new(id)?))
            },
            "ethernet" => {
                info!("Création d'un contrôleur d'éclairage Ethernet avec ID: {}", id);
                Ok(Box::new(controllers::ethernet::EthernetLightingController::new(id)?))
            },
            "simulator" => {
                info!("Création d'un contrôleur d'éclairage simulé avec ID: {}", id);
                Ok(Box::new(controllers::simulator::SimulatedLightingController::new(id)))
            },
            #[cfg(feature = "raspberry_pi")]
            "gpio" => {
                info!("Création d'un contrôleur d'éclairage GPIO avec ID: {}", id);
                Ok(Box::new(controllers::gpio::GpioLightingController::new(id)?))
            },
            _ => {
                error!("Type de contrôleur d'éclairage non supporté: {}", controller_type);
                Err(LightingError::InitError(format!("Type de contrôleur non supporté: {}", controller_type)))
            }
        }
    }
}

/// Gestionnaire de synchronisation entre caméra et éclairage
pub struct LightingSynchronizer {
    /// Contrôleur d'éclairage
    controller: Arc<Mutex<Box<dyn LightingController>>>,
    
    /// Mode de synchronisation
    sync_mode: SyncMode,
    
    /// Canal de communication pour les événements de synchronisation
    sync_channel: (Sender<SyncEvent>, Receiver<SyncEvent>),
    
    /// Tâche de synchronisation en arrière-plan
    sync_task: Option<tokio::task::JoinHandle<()>>,
    
    /// Statistiques de synchronisation
    stats: Arc<Mutex<SyncStats>>,
}

/// Événement de synchronisation
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// Déclenchement de l'acquisition d'image
    CameraTrigger { timestamp: Instant, camera_id: String },
    
    /// Déclenchement externe
    ExternalTrigger { timestamp: Instant, source: String },
    
    /// Déclenchement logiciel
    SoftwareTrigger { timestamp: Instant },
    
    /// Arrêt de la synchronisation
    Stop,
}

/// Statistiques de synchronisation
#[derive(Debug, Clone)]
pub struct SyncStats {
    /// Nombre total d'événements de synchronisation
    pub total_events: u64,
    
    /// Nombre d'événements réussis
    pub successful_events: u64,
    
    /// Nombre d'événements en échec
    pub failed_events: u64,
    
    /// Délai moyen de synchronisation (en microsecondes)
    pub average_delay_us: f64,
    
    /// Délai maximal de synchronisation (en microsecondes)
    pub max_delay_us: u64,
    
    /// Horodatage du dernier événement
    pub last_event_timestamp: Option<Instant>,
}

impl LightingSynchronizer {
    /// Crée un nouveau synchroniseur
    pub fn new(controller: Box<dyn LightingController>, sync_mode: SyncMode) -> Self {
        let controller = Arc::new(Mutex::new(controller));
        let sync_channel = bounded(100); // Buffer de 100 événements
        let stats = Arc::new(Mutex::new(SyncStats {
            total_events: 0,
            successful_events: 0,
            failed_events: 0,
            average_delay_us: 0.0,
            max_delay_us: 0,
            last_event_timestamp: None,
        }));
        
        Self {
            controller,
            sync_mode,
            sync_channel,
            sync_task: None,
            stats,
        }
    }
    
    /// Démarre la synchronisation
    pub fn start(&mut self) -> Result<(), LightingError> {
        if self.sync_task.is_some() {
            return Err(LightingError::SynchronizationError("Synchronisation déjà démarrée".to_string()));
        }
        
        let controller = self.controller.clone();
        let receiver = self.sync_channel.1.clone();
        let stats = self.stats.clone();
        
        // Démarrer la tâche de synchronisation en arrière-plan
        self.sync_task = Some(tokio::spawn(async move {
            Self::sync_task(controller, receiver, stats).await;
        }));
        
        Ok(())
    }
    
    /// Arrête la synchronisation
    pub fn stop(&mut self) -> Result<(), LightingError> {
        if let Some(task) = self.sync_task.take() {
            // Envoyer un événement d'arrêt
            if let Err(e) = self.sync_channel.0.send(SyncEvent::Stop) {
                error!("Erreur lors de l'envoi de l'événement d'arrêt: {}", e);
            }
            
            // Attendre la fin de la tâche
            tokio::spawn(async move {
                if let Err(e) = task.await {
                    error!("Erreur lors de l'arrêt de la tâche de synchronisation: {}", e);
                }
            });
        }
        
        Ok(())
    }
    
    /// Envoie un événement de synchronisation
    pub fn send_event(&self, event: SyncEvent) -> Result<(), LightingError> {
        self.sync_channel.0.send(event).map_err(|e| {
            LightingError::SynchronizationError(format!("Erreur lors de l'envoi de l'événement: {}", e))
        })
    }
    
    /// Obtient les statistiques de synchronisation
    pub fn get_stats(&self) -> SyncStats {
        self.stats.lock().unwrap().clone()
    }
    
    /// Tâche de synchronisation en arrière-plan
    async fn sync_task(
        controller: Arc<Mutex<Box<dyn LightingController>>>,
        receiver: Receiver<SyncEvent>,
        stats: Arc<Mutex<SyncStats>>
    ) {
        while let Ok(event) = receiver.recv() {
            match event {
                SyncEvent::Stop => break,
                _ => {
                    let event_time = Instant::now();
                    let mut stats = stats.lock().unwrap();
                    stats.total_events += 1;
                    stats.last_event_timestamp = Some(event_time);
                    
                    // Traiter l'événement de synchronisation
                    let result = {
                        let mut controller = controller.lock().unwrap();
                        tokio::task::spawn_blocking(move || {
                            let rt = tokio::runtime::Handle::current();
                            rt.block_on(async {
                                controller.trigger_all().await
                            })
                        }).await
                    };
                    
                    // Mettre à jour les statistiques
                    match result {
                        Ok(Ok(())) => {
                            stats.successful_events += 1;
                            let delay = event_time.elapsed();
                            let delay_us = delay.as_micros() as u64;
                            
                            // Mettre à jour le délai moyen
                            let total_successful = stats.successful_events as f64;
                            stats.average_delay_us = (stats.average_delay_us * (total_successful - 1.0) + delay_us as f64) / total_successful;
                            
                            // Mettre à jour le délai maximal
                            if delay_us > stats.max_delay_us {
                                stats.max_delay_us = delay_us;
                            }
                        },
                        _ => {
                            stats.failed_events += 1;
                            error!("Erreur lors du déclenchement de l'éclairage");
                        }
                    }
                }
            }
        }
    }
}

/// Gestionnaire d'ajustement automatique d'intensité
pub struct AutoIntensityAdjuster {
    /// Contrôleur d'éclairage
    controller: Arc<Mutex<Box<dyn LightingController>>>,
    
    /// Canal d'éclairage à ajuster
    channel_id: String,
    
    /// Intensité cible (valeur moyenne de l'image)
    target_intensity: f64,
    
    /// Tolérance d'intensité
    tolerance: f64,
    
    /// Pas d'ajustement
    adjustment_step: f64,
    
    /// Intensité minimale
    min_intensity: f64,
    
    /// Intensité maximale
    max_intensity: f64,
}

impl AutoIntensityAdjuster {
    /// Crée un nouveau gestionnaire d'ajustement automatique
    pub fn new(
        controller: Box<dyn LightingController>,
        channel_id: String,
        target_intensity: f64,
        tolerance: f64,
        adjustment_step: f64,
        min_intensity: f64,
        max_intensity: f64,
    ) -> Self {
        Self {
            controller: Arc::new(Mutex::new(controller)),
            channel_id,
            target_intensity,
            tolerance,
            adjustment_step,
            min_intensity,
            max_intensity,
        }
    }
    
    /// Ajuste l'intensité en fonction de l'image acquise
    pub async fn adjust(&self, image_mean: f64) -> Result<f64, LightingError> {
        let mut controller = self.controller.lock().unwrap();
        
        // Obtenir l'état actuel du canal
        let channel_state = controller.get_channel_state(&self.channel_id)
            .ok_or_else(|| LightingError::ConfigError(format!("Canal non trouvé: {}", self.channel_id)))?;
        
        let current_intensity = channel_state.current_intensity;
        
        // Calculer l'erreur
        let error = self.target_intensity - image_mean;
        
        // Si l'erreur est dans la tolérance, ne rien faire
        if error.abs() <= self.tolerance {
            return Ok(current_intensity);
        }
        
        // Calculer la nouvelle intensité
        let mut new_intensity = current_intensity;
        
        if error > 0.0 {
            // L'image est trop sombre, augmenter l'intensité
            new_intensity += self.adjustment_step;
        } else {
            // L'image est trop claire, diminuer l'intensité
            new_intensity -= self.adjustment_step;
        }
        
        // Limiter l'intensité
        new_intensity = new_intensity.max(self.min_intensity).min(self.max_intensity);
        
        // Appliquer la nouvelle intensité
        controller.set_intensity(&self.channel_id, new_intensity).await?;
        
        Ok(new_intensity)
    }
}

/// Gestionnaire de diagnostic d'éclairage
pub struct LightingDiagnostics {
    /// Contrôleur d'éclairage
    controller: Arc<Mutex<Box<dyn LightingController>>>,
    
    /// Seuil d'alerte pour la durée d'utilisation (en heures)
    usage_threshold_hours: f64,
    
    /// Seuil d'alerte pour l'intensité minimale (%)
    min_intensity_threshold: f64,
    
    /// Historique des diagnostics
    history: Vec<DiagnosticResult>,
}

/// Résultat de diagnostic
#[derive(Debug, Clone)]
pub struct DiagnosticResult {
    /// Horodatage
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Statut global
    pub status: DiagnosticStatus,
    
    /// Résultats par canal
    pub channel_results: HashMap<String, ChannelDiagnostic>,
    
    /// Messages d'erreur ou d'avertissement
    pub messages: Vec<String>,
}

/// Statut de diagnostic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStatus {
    /// Tout est normal
    Ok,
    
    /// Avertissement (fonctionnement dégradé)
    Warning,
    
    /// Erreur (fonctionnement impossible)
    Error,
}

/// Diagnostic d'un canal d'éclairage
#[derive(Debug, Clone)]
pub struct ChannelDiagnostic {
    /// Identifiant du canal
    pub channel_id: String,
    
    /// Statut du canal
    pub status: DiagnosticStatus,
    
    /// Durée d'utilisation (en heures)
    pub usage_hours: f64,
    
    /// Intensité maximale atteignable (%)
    pub max_intensity: f64,
    
    /// Uniformité de l'éclairage (%)
    pub uniformity: f64,
}

impl LightingDiagnostics {
    /// Crée un nouveau gestionnaire de diagnostic
    pub fn new(
        controller: Box<dyn LightingController>,
        usage_threshold_hours: f64,
        min_intensity_threshold: f64,
    ) -> Self {
        Self {
            controller: Arc::new(Mutex::new(controller)),
            usage_threshold_hours,
            min_intensity_threshold,
            history: Vec::new(),
        }
    }
    
    /// Exécute un diagnostic complet
    pub async fn run_diagnostic(&mut self) -> Result<DiagnosticResult, LightingError> {
        let controller = self.controller.lock().unwrap();
        let config = controller.get_config();
        
        let mut result = DiagnosticResult {
            timestamp: chrono::Utc::now(),
            status: DiagnosticStatus::Ok,
            channel_results: HashMap::new(),
            messages: Vec::new(),
        };
        
        // Vérifier chaque canal
        for channel_config in &config.channels {
            let channel_id = &channel_config.id;
            
            // Obtenir l'état du canal
            if let Some(channel_state) = controller.get_channel_state(channel_id) {
                // Calculer la durée d'utilisation en heures
                let usage_hours = channel_state.total_on_time_ms as f64 / (1000.0 * 60.0 * 60.0);
                
                // Déterminer le statut du canal
                let mut channel_status = DiagnosticStatus::Ok;
                
                // Vérifier la durée d'utilisation
                if usage_hours > self.usage_threshold_hours {
                    channel_status = DiagnosticStatus::Warning;
                    result.messages.push(format!(
                        "Canal {} : durée d'utilisation élevée ({:.1} heures)",
                        channel_id, usage_hours
                    ));
                }
                
                // Vérifier l'intensité maximale
                // Dans un système réel, cela nécessiterait un test d'intensité
                let max_intensity = channel_state.current_intensity;
                if max_intensity < self.min_intensity_threshold {
                    channel_status = DiagnosticStatus::Warning;
                    result.messages.push(format!(
                        "Canal {} : intensité maximale faible ({}%)",
                        channel_id, max_intensity
                    ));
                }
                
                // Vérifier l'uniformité (simulée ici)
                // Dans un système réel, cela nécessiterait une analyse d'image
                let uniformity = 95.0 - (usage_hours / 1000.0) * 5.0;
                let uniformity = uniformity.max(0.0).min(100.0);
                
                if uniformity < 80.0 {
                    channel_status = DiagnosticStatus::Warning;
                    result.messages.push(format!(
                        "Canal {} : uniformité d'éclairage faible ({}%)",
                        channel_id, uniformity
                    ));
                }
                
                // Mettre à jour le statut global
                if channel_status == DiagnosticStatus::Error {
                    result.status = DiagnosticStatus::Error;
                } else if channel_status == DiagnosticStatus::Warning && result.status == DiagnosticStatus::Ok {
                    result.status = DiagnosticStatus::Warning;
                }
                
                // Ajouter le diagnostic du canal
                result.channel_results.insert(channel_id.clone(), ChannelDiagnostic {
                    channel_id: channel_id.clone(),
                    status: channel_status,
                    usage_hours,
                    max_intensity,
                    uniformity,
                });
            } else {
                // Canal non trouvé
                result.status = DiagnosticStatus::Error;
                result.messages.push(format!("Canal {} non trouvé", channel_id));
            }
        }
        
        // Ajouter le résultat à l'historique
        self.history.push(result.clone());
        
        Ok(result)
    }
    
    /// Obtient l'historique des diagnostics
    pub fn get_history(&self) -> &[DiagnosticResult] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_lighting_controller_simulator() {
        // Créer un contrôleur simulé
        let controller_result = LightingControllerFactory::create("simulator", "test");
        assert!(controller_result.is_ok());
        
        let mut controller = controller_result.unwrap();
        
        // Créer une configuration
        let config = LightingConfig {
            controller_id: "test".to_string(),
            controller_type: "simulator".to_string(),
            sync_mode: SyncMode::Software,
            channels: vec![
                LightChannelConfig {
                    id: "channel1".to_string(),
                    lighting_type: LightingType::Diffuse,
                    intensity: 50.0,
                    duration_us: 1000,
                    delay_us: 0,
                    controller_params: HashMap::new(),
                },
                LightChannelConfig {
                    id: "channel2".to_string(),
                    lighting_type: LightingType::Backlight,
                    intensity: 75.0,
                    duration_us: 2000,
                    delay_us: 100,
                    controller_params: HashMap::new(),
                },
            ],
            connection_params: HashMap::new(),
        };
        
        // Initialiser le contrôleur
        let init_result = controller.initialize(config.clone()).await;
        assert!(init_result.is_ok());
        
        // Vérifier la configuration
        let controller_config = controller.get_config();
        assert_eq!(controller_config.controller_id, "test");
        assert_eq!(controller_config.channels.len(), 2);
        
        // Activer un canal
        let turn_on_result = controller.turn_on("channel1").await;
        assert!(turn_on_result.is_ok());
        
        // Vérifier l'état du canal
        let channel_state = controller.get_channel_state("channel1");
        assert!(channel_state.is_some());
        assert!(channel_state.unwrap().is_on);
        
        // Désactiver un canal
        let turn_off_result = controller.turn_off("channel1").await;
        assert!(turn_off_result.is_ok());
        
        // Vérifier l'état du canal
        let channel_state = controller.get_channel_state("channel1");
        assert!(channel_state.is_some());
        assert!(!channel_state.unwrap().is_on);
        
        // Définir l'intensité
        let set_intensity_result = controller.set_intensity("channel2", 80.0).await;
        assert!(set_intensity_result.is_ok());
        
        // Vérifier l'intensité
        let channel_state = controller.get_channel_state("channel2");
        assert!(channel_state.is_some());
        assert_eq!(channel_state.unwrap().current_intensity, 80.0);
        
        // Tester le mode stroboscopique
        let strobe_result = controller.strobe("channel1", 500).await;
        assert!(strobe_result.is_ok());
    }
}
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use log::{debug, error, info, warn};
use tokio::time;
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::{
    LightingController, LightingConfig, LightChannelConfig, LightChannelState,
    LightingError, SyncMode, LightingSynchronizer, SyncEvent, SyncStats
};

/// Source de synchronisation externe
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSyncSource {
    /// Entrée GPIO
    GPIO,
    
    /// Signal de déclenchement externe
    TriggerInput,
    
    /// Encodeur
    Encoder,
    
    /// Capteur de proximité
    ProximitySensor,
    
    /// Horloge externe
    ExternalClock,
}

/// Configuration de synchronisation externe
#[derive(Debug, Clone)]
pub struct ExternalSyncConfig {
    /// Source de synchronisation
    pub source: ExternalSyncSource,
    
    /// Identifiant de la source (numéro de broche, adresse, etc.)
    pub source_id: String,
    
    /// Délai avant l'activation (µs)
    pub pre_trigger_delay_us: u64,
    
    /// Délai après l'activation (µs)
    pub post_trigger_delay_us: u64,
    
    /// Durée d'activation (µs)
    pub activation_duration_us: u64,
    
    /// Délai de sécurité (µs)
    pub safety_margin_us: u64,
    
    /// Niveau logique actif
    pub active_high: bool,
    
    /// Débounce (µs)
    pub debounce_us: u64,
}

impl Default for ExternalSyncConfig {
    fn default() -> Self {
        Self {
            source: ExternalSyncSource::GPIO,
            source_id: "0".to_string(),
            pre_trigger_delay_us: 100,
            post_trigger_delay_us: 100,
            activation_duration_us: 1000,
            safety_margin_us: 500,
            active_high: true,
            debounce_us: 1000,
        }
    }
}

/// Synchroniseur avec source externe
pub struct ExternalSynchronizer {
    /// Synchroniseur de base
    synchronizer: LightingSynchronizer,
    
    /// Canal d'envoi d'événements
    event_sender: Sender<SyncEvent>,
    
    /// Configuration de synchronisation
    config: ExternalSyncConfig,
    
    /// Dernier état de l'entrée
    last_input_state: bool,
    
    /// Horodatage du dernier changement d'état
    last_change_time: Instant,
}

impl ExternalSynchronizer {
    /// Crée un nouveau synchroniseur externe
    pub fn new(
        controller: Box<dyn LightingController>,
        config: ExternalSyncConfig
    ) -> Self {
        let synchronizer = LightingSynchronizer::new(controller, SyncMode::ExternalTrigger);
        let event_sender = synchronizer.send_event.clone();
        
        Self {
            synchronizer,
            event_sender,
            config,
            last_input_state: false,
            last_change_time: Instant::now(),
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
    
    /// Traite un changement d'état de l'entrée
    pub fn process_input_change(&mut self, state: bool) -> Result<(), LightingError> {
        let now = Instant::now();
        
        // Vérifier le debounce
        if now.duration_since(self.last_change_time).as_micros() < self.config.debounce_us as u128 {
            return Ok(());
        }
        
        // Vérifier si l'état a changé
        if state != self.last_input_state {
            self.last_input_state = state;
            self.last_change_time = now;
            
            // Vérifier si c'est un front actif
            let is_active_edge = if self.config.active_high {
                state
            } else {
                !state
            };
            
            if is_active_edge {
                // Envoyer l'événement de déclenchement
                let event = SyncEvent::ExternalTrigger {
                    timestamp: now,
                    source: self.config.source_id.clone(),
                };
                
                self.event_sender.send(event).map_err(|e| {
                    LightingError::SynchronizationError(format!("Erreur lors de l'envoi de l'événement: {}", e))
                })?;
            }
        }
        
        Ok(())
    }
    
    /// Simule un déclenchement externe
    pub fn simulate_trigger(&self) -> Result<(), LightingError> {
        let event = SyncEvent::ExternalTrigger {
            timestamp: Instant::now(),
            source: self.config.source_id.clone(),
        };
        
        self.event_sender.send(event).map_err(|e| {
            LightingError::SynchronizationError(format!("Erreur lors de l'envoi de l'événement: {}", e))
        })
    }
    
    /// Configure les canaux d'éclairage pour une synchronisation optimale
    pub async fn configure_lighting_channels(&self) -> Result<(), LightingError> {
        let controller = self.synchronizer.controller.lock().unwrap();
        let config = controller.get_config();
        
        // Mettre à jour la configuration de chaque canal
        for channel in &config.channels {
            controller.set_parameter(&channel.id, "delay_us", &self.config.pre_trigger_delay_us.to_string()).await?;
            controller.set_parameter(&channel.id, "duration_us", &self.config.activation_duration_us.to_string()).await?;
        }
        
        Ok(())
    }
    
    /// Démarre la surveillance d'une entrée GPIO
    #[cfg(feature = "raspberry_pi")]
    pub fn start_gpio_monitoring(&mut self) -> Result<(), LightingError> {
        use rppal::gpio::{Gpio, InputPin, Level, Trigger};
        use std::error::Error;
        
        // Convertir l'ID de la source en numéro de broche
        let pin_number = self.config.source_id.parse::<u8>()
            .map_err(|e| LightingError::ConfigError(format!(
                "Numéro de broche GPIO invalide: {}", e
            )))?;
        
        // Initialiser le GPIO
        let gpio = Gpio::new()
            .map_err(|e| LightingError::HardwareError(format!(
                "Erreur d'initialisation du GPIO: {}", e
            )))?;
        
        // Configurer la broche en entrée
        let mut pin = gpio.get(pin_number)
            .map_err(|e| LightingError::HardwareError(format!(
                "Erreur de configuration de la broche GPIO {}: {}", pin_number, e
            )))?
            .into_input();
        
        // Configurer le pull-up/pull-down
        if self.config.active_high {
            pin.set_pulldown()
                .map_err(|e| LightingError::HardwareError(format!(
                    "Erreur de configuration du pull-down: {}", e
                )))?;
        } else {
            pin.set_pullup()
                .map_err(|e| LightingError::HardwareError(format!(
                    "Erreur de configuration du pull-up: {}", e
                )))?;
        }
        
        // Lire l'état initial
        let initial_level = pin.read();
        self.last_input_state = match initial_level {
            Level::High => true,
            Level::Low => false,
        };
        self.last_change_time = Instant::now();
        
        // Configurer l'interruption
        let event_sender = self.event_sender.clone();
        let source_id = self.config.source_id.clone();
        let active_high = self.config.active_high;
        
        pin.set_async_interrupt(Trigger::Both, move |level| {
            let is_high = match level {
                Level::High => true,
                Level::Low => false,
            };
            
            // Vérifier si c'est un front actif
            let is_active_edge = if active_high {
                is_high
            } else {
                !is_high
            };
            
            if is_active_edge {
                // Envoyer l'événement de déclenchement
                let event = SyncEvent::ExternalTrigger {
                    timestamp: Instant::now(),
                    source: source_id.clone(),
                };
                
                if let Err(e) = event_sender.send(event) {
                    error!("Erreur lors de l'envoi de l'événement: {}", e);
                }
            }
        })
        .map_err(|e| LightingError::HardwareError(format!(
            "Erreur de configuration de l'interruption: {}", e
        )))?;
        
        Ok(())
    }
    
    /// Démarre la surveillance d'un encodeur
    #[cfg(feature = "raspberry_pi")]
    pub fn start_encoder_monitoring(&mut self) -> Result<(), LightingError> {
        use rppal::gpio::{Gpio, InputPin, Level, Trigger};
        use std::error::Error;
        use std::str::FromStr;
        
        // Extraire les numéros de broches (format "pinA,pinB")
        let pins: Vec<&str> = self.config.source_id.split(',').collect();
        if pins.len() != 2 {
            return Err(LightingError::ConfigError(
                "Format d'ID d'encodeur invalide. Utilisez 'pinA,pinB'".to_string()
            ));
        }
        
        let pin_a = u8::from_str(pins[0])
            .map_err(|e| LightingError::ConfigError(format!(
                "Numéro de broche A invalide: {}", e
            )))?;
            
        let pin_b = u8::from_str(pins[1])
            .map_err(|e| LightingError::ConfigError(format!(
                "Numéro de broche B invalide: {}", e
            )))?;
        
        // Initialiser le GPIO
        let gpio = Gpio::new()
            .map_err(|e| LightingError::HardwareError(format!(
                "Erreur d'initialisation du GPIO: {}", e
            )))?;
        
        // Configurer les broches en entrée
        let mut pin_a_input = gpio.get(pin_a)
            .map_err(|e| LightingError::HardwareError(format!(
                "Erreur de configuration de la broche GPIO {}: {}", pin_a, e
            )))?
            .into_input();
            
        let pin_b_input = gpio.get(pin_b)
            .map_err(|e| LightingError::HardwareError(format!(
                "Erreur de configuration de la broche GPIO {}: {}", pin_b, e
            )))?
            .into_input();
        
        // Configurer le pull-up
        pin_a_input.set_pullup()
            .map_err(|e| LightingError::HardwareError(format!(
                "Erreur de configuration du pull-up: {}", e
            )))?;
            
        pin_b_input.set_pullup()
            .map_err(|e| LightingError::HardwareError(format!(
                "Erreur de configuration du pull-up: {}", e
            )))?;
        
        // Variables pour suivre l'état de l'encodeur
        let encoder_state = Arc::new(Mutex::new((false, false)));
        let encoder_state_clone = encoder_state.clone();
        
        // Configurer l'interruption sur la broche A
        let event_sender = self.event_sender.clone();
        let source_id = self.config.source_id.clone();
        
        pin_a_input.set_async_interrupt(Trigger::Both, move |level| {
            let is_high_a = match level {
                Level::High => true,
                Level::Low => false,
            };
            
            let is_high_b = match pin_b_input.read() {
                Level::High => true,
                Level::Low => false,
            };
            
            // Mettre à jour l'état de l'encodeur
            let mut state = encoder_state_clone.lock().unwrap();
            let old_state = *state;
            *state = (is_high_a, is_high_b);
            
            // Détecter la rotation
            if old_state.0 != is_high_a {
                // Front sur A
                if is_high_a {
                    // Front montant sur A
                    if is_high_b {
                        // Rotation dans le sens horaire
                        let event = SyncEvent::ExternalTrigger {
                            timestamp: Instant::now(),
                            source: source_id.clone(),
                        };
                        
                        if let Err(e) = event_sender.send(event) {
                            error!("Erreur lors de l'envoi de l'événement: {}", e);
                        }
                    }
                }
            }
        })
        .map_err(|e| LightingError::HardwareError(format!(
            "Erreur de configuration de l'interruption: {}", e
        )))?;
        
        Ok(())
    }
}
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use tokio::time;

use crate::{
    LightingController, LightingConfig, LightChannelConfig, LightChannelState,
    LightingError, SyncMode, LightingType
};

/// Contrôleur d'éclairage simulé pour les tests et le développement
pub struct SimulatedLightingController {
    /// Identifiant du contrôleur
    id: String,
    
    /// Configuration actuelle
    config: LightingConfig,
    
    /// État des canaux
    channel_states: HashMap<String, LightChannelState>,
    
    /// Horodatage de la dernière commande
    last_command_time: Instant,
}

impl SimulatedLightingController {
    /// Crée un nouveau contrôleur simulé
    pub fn new(id: &str) -> Self {
        info!("Initialisation du contrôleur d'éclairage simulé: {}", id);
        
        Self {
            id: id.to_string(),
            config: LightingConfig::default(),
            channel_states: HashMap::new(),
            last_command_time: Instant::now(),
        }
    }
    
    /// Simule un délai de communication
    async fn simulate_delay(&mut self) {
        // Simuler un délai aléatoire entre 1 et 5 ms
        let delay_ms = rand::random::<u64>() % 5 + 1;
        time::sleep(Duration::from_millis(delay_ms)).await;
        self.last_command_time = Instant::now();
    }
}

#[async_trait]
impl LightingController for SimulatedLightingController {
    async fn initialize(&mut self, config: LightingConfig) -> Result<(), LightingError> {
        info!("Initialisation du contrôleur simulé avec {} canaux", config.channels.len());
        
        self.config = config.clone();
        self.channel_states.clear();
        
        // Initialiser l'état de chaque canal
        for channel in &config.channels {
            let state = LightChannelState {
                config: channel.clone(),
                is_on: false,
                current_intensity: channel.intensity,
                last_activation: None,
                activation_count: 0,
                total_on_time_ms: 0,
            };
            
            self.channel_states.insert(channel.id.clone(), state);
        }
        
        self.simulate_delay().await;
        
        Ok(())
    }
    
    async fn turn_on(&mut self, channel_id: &str) -> Result<(), LightingError> {
        debug!("Activation du canal: {}", channel_id);
        
        if let Some(state) = self.channel_states.get_mut(channel_id) {
            state.is_on = true;
            state.last_activation = Some(Instant::now());
            state.activation_count += 1;
        } else {
            return Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)));
        }
        
        self.simulate_delay().await;
        
        Ok(())
    }
    
    async fn turn_off(&mut self, channel_id: &str) -> Result<(), LightingError> {
        debug!("Désactivation du canal: {}", channel_id);
        
        if let Some(state) = self.channel_states.get_mut(channel_id) {
            if state.is_on {
                state.is_on = false;
                
                // Mettre à jour la durée d'activation
                if let Some(last_activation) = state.last_activation {
                    let duration = last_activation.elapsed();
                    state.total_on_time_ms += duration.as_millis() as u64;
                }
            }
        } else {
            return Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)));
        }
        
        self.simulate_delay().await;
        
        Ok(())
    }
    
    async fn set_intensity(&mut self, channel_id: &str, intensity: f64) -> Result<(), LightingError> {
        debug!("Réglage de l'intensité du canal {} à {}%", channel_id, intensity);
        
        // Vérifier que l'intensité est dans la plage valide
        if intensity < 0.0 || intensity > 100.0 {
            return Err(LightingError::ConfigError(format!(
                "Intensité invalide: {}%. Doit être entre 0 et 100", intensity
            )));
        }
        
        if let Some(state) = self.channel_states.get_mut(channel_id) {
            state.current_intensity = intensity;
            state.config.intensity = intensity;
        } else {
            return Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)));
        }
        
        self.simulate_delay().await;
        
        Ok(())
    }
    
    async fn strobe(&mut self, channel_id: &str, duration_us: u64) -> Result<(), LightingError> {
        debug!("Stroboscope du canal {} pendant {} µs", channel_id, duration_us);
        
        if let Some(state) = self.channel_states.get_mut(channel_id) {
            // Activer le canal
            state.is_on = true;
            state.last_activation = Some(Instant::now());
            state.activation_count += 1;
            
            // Simuler la durée du stroboscope
            time::sleep(Duration::from_micros(duration_us)).await;
            
            // Désactiver le canal
            state.is_on = false;
            
            // Mettre à jour la durée d'activation
            if let Some(last_activation) = state.last_activation {
                let duration = last_activation.elapsed();
                state.total_on_time_ms += duration.as_millis() as u64;
            }
        } else {
            return Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)));
        }
        
        Ok(())
    }
    
    async fn trigger_all(&mut self) -> Result<(), LightingError> {
        debug!("Déclenchement de tous les canaux");
        
        // Collecter les canaux à déclencher
        let channel_ids: Vec<String> = self.channel_states.keys().cloned().collect();
        
        // Déclencher chaque canal avec sa configuration
        for channel_id in channel_ids {
            if let Some(state) = self.channel_states.get(&channel_id) {
                let duration_us = state.config.duration_us;
                let delay_us = state.config.delay_us;
                
                // Appliquer le délai si nécessaire
                if delay_us > 0 {
                    time::sleep(Duration::from_micros(delay_us)).await;
                }
                
                // Déclencher le canal
                self.strobe(&channel_id, duration_us).await?;
            }
        }
        
        Ok(())
    }
    
    fn get_channel_state(&self, channel_id: &str) -> Option<LightChannelState> {
        self.channel_states.get(channel_id).cloned()
    }
    
    fn get_config(&self) -> LightingConfig {
        self.config.clone()
    }
    
    async fn set_parameter(&mut self, channel_id: &str, name: &str, value: &str) -> Result<(), LightingError> {
        debug!("Réglage du paramètre {} = {} pour le canal {}", name, value, channel_id);
        
        if let Some(state) = self.channel_states.get_mut(channel_id) {
            state.config.controller_params.insert(name.to_string(), value.to_string());
        } else {
            return Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)));
        }
        
        self.simulate_delay().await;
        
        Ok(())
    }
    
    async fn get_parameter(&self, channel_id: &str, name: &str) -> Result<String, LightingError> {
        if let Some(state) = self.channel_states.get(channel_id) {
            if let Some(value) = state.config.controller_params.get(name) {
                return Ok(value.clone());
            } else {
                return Err(LightingError::ConfigError(format!(
                    "Paramètre non trouvé: {} pour le canal {}", name, channel_id
                )));
            }
        } else {
            return Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)));
        }
    }
}
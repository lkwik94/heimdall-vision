use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use serialport::{SerialPort, SerialPortSettings, DataBits, FlowControl, Parity, StopBits};
use tokio::time;

use crate::{
    LightingController, LightingConfig, LightChannelConfig, LightChannelState,
    LightingError, SyncMode, LightingType
};

/// Protocole de communication série
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialProtocol {
    /// Protocole simple (commandes ASCII)
    Simple,
    
    /// Protocole binaire
    Binary,
    
    /// Protocole Modbus RTU
    ModbusRTU,
    
    /// Protocole DMX512
    DMX512,
}

impl Default for SerialProtocol {
    fn default() -> Self {
        Self::Simple
    }
}

/// Contrôleur d'éclairage série
pub struct SerialLightingController {
    /// Identifiant du contrôleur
    id: String,
    
    /// Configuration actuelle
    config: LightingConfig,
    
    /// État des canaux
    channel_states: HashMap<String, LightChannelState>,
    
    /// Port série
    port: Option<Box<dyn SerialPort>>,
    
    /// Protocole de communication
    protocol: SerialProtocol,
    
    /// Délai de communication (ms)
    communication_delay_ms: u64,
}

impl SerialLightingController {
    /// Crée un nouveau contrôleur série
    pub fn new(id: &str) -> Result<Self, LightingError> {
        info!("Initialisation du contrôleur d'éclairage série: {}", id);
        
        Ok(Self {
            id: id.to_string(),
            config: LightingConfig::default(),
            channel_states: HashMap::new(),
            port: None,
            protocol: SerialProtocol::default(),
            communication_delay_ms: 10,
        })
    }
    
    /// Ouvre le port série
    fn open_port(&mut self, port_name: &str, baud_rate: u32) -> Result<(), LightingError> {
        info!("Ouverture du port série {} à {} bauds", port_name, baud_rate);
        
        let settings = SerialPortSettings {
            baud_rate,
            data_bits: DataBits::Eight,
            flow_control: FlowControl::None,
            parity: Parity::None,
            stop_bits: StopBits::One,
            timeout: Duration::from_millis(100),
        };
        
        match serialport::open_with_settings(port_name, &settings) {
            Ok(port) => {
                self.port = Some(port);
                Ok(())
            },
            Err(e) => {
                error!("Erreur lors de l'ouverture du port série {}: {}", port_name, e);
                Err(LightingError::CommunicationError(format!(
                    "Erreur lors de l'ouverture du port série {}: {}", port_name, e
                )))
            }
        }
    }
    
    /// Envoie une commande au contrôleur
    fn send_command(&mut self, command: &[u8]) -> Result<Vec<u8>, LightingError> {
        if let Some(port) = &mut self.port {
            // Envoyer la commande
            match port.write(command) {
                Ok(bytes_written) => {
                    if bytes_written != command.len() {
                        return Err(LightingError::CommunicationError(format!(
                            "Erreur d'écriture: {} octets écrits sur {} attendus",
                            bytes_written, command.len()
                        )));
                    }
                    
                    // Attendre la réponse
                    std::thread::sleep(Duration::from_millis(self.communication_delay_ms));
                    
                    // Lire la réponse
                    let mut response = vec![0u8; 1024];
                    match port.read(response.as_mut_slice()) {
                        Ok(bytes_read) => {
                            response.truncate(bytes_read);
                            Ok(response)
                        },
                        Err(e) => {
                            error!("Erreur lors de la lecture de la réponse: {}", e);
                            Err(LightingError::CommunicationError(format!(
                                "Erreur lors de la lecture de la réponse: {}", e
                            )))
                        }
                    }
                },
                Err(e) => {
                    error!("Erreur lors de l'envoi de la commande: {}", e);
                    Err(LightingError::CommunicationError(format!(
                        "Erreur lors de l'envoi de la commande: {}", e
                    )))
                }
            }
        } else {
            Err(LightingError::CommunicationError("Port série non ouvert".to_string()))
        }
    }
    
    /// Construit une commande selon le protocole
    fn build_command(&self, action: &str, channel_id: &str, value: Option<f64>) -> Vec<u8> {
        match self.protocol {
            SerialProtocol::Simple => {
                // Protocole simple: ACTION,CHANNEL,VALUE\r\n
                let command = match value {
                    Some(val) => format!("{},{},{:.2}\r\n", action, channel_id, val),
                    None => format!("{},{}\r\n", action, channel_id),
                };
                command.into_bytes()
            },
            SerialProtocol::Binary => {
                // Protocole binaire: [STX][ACTION][CHANNEL][VALUE][CHECKSUM][ETX]
                let mut command = Vec::new();
                
                // STX (Start of Text)
                command.push(0x02);
                
                // Action
                let action_code = match action {
                    "ON" => 0x01,
                    "OFF" => 0x02,
                    "INTENSITY" => 0x03,
                    "STROBE" => 0x04,
                    "TRIGGER" => 0x05,
                    _ => 0x00,
                };
                command.push(action_code);
                
                // Channel (convertir en nombre)
                let channel_num = channel_id.parse::<u8>().unwrap_or(0);
                command.push(channel_num);
                
                // Value (si présente)
                if let Some(val) = value {
                    let value_int = (val * 255.0 / 100.0) as u8;
                    command.push(value_int);
                } else {
                    command.push(0);
                }
                
                // Checksum (XOR de tous les octets)
                let checksum = command.iter().skip(1).fold(0u8, |acc, &x| acc ^ x);
                command.push(checksum);
                
                // ETX (End of Text)
                command.push(0x03);
                
                command
            },
            SerialProtocol::ModbusRTU => {
                // Implémentation simplifiée de Modbus RTU
                // Dans un système réel, il faudrait utiliser une bibliothèque Modbus
                let mut command = Vec::new();
                
                // Adresse de l'esclave (1 par défaut)
                command.push(0x01);
                
                // Fonction (écriture de registre unique)
                command.push(0x06);
                
                // Adresse du registre (dépend du canal)
                let register = channel_id.parse::<u16>().unwrap_or(0);
                command.push((register >> 8) as u8);
                command.push(register as u8);
                
                // Valeur à écrire
                let value_int = match value {
                    Some(val) => (val * 65535.0 / 100.0) as u16,
                    None => 0,
                };
                command.push((value_int >> 8) as u8);
                command.push(value_int as u8);
                
                // CRC16 (simplifié)
                command.push(0x00);
                command.push(0x00);
                
                command
            },
            SerialProtocol::DMX512 => {
                // Implémentation simplifiée de DMX512
                // Dans un système réel, il faudrait utiliser une bibliothèque DMX
                let mut command = Vec::new();
                
                // Break et MAB (Mark After Break)
                command.push(0x00);
                command.push(0x00);
                
                // Start code
                command.push(0x00);
                
                // Canal DMX (1-512)
                let channel = channel_id.parse::<u16>().unwrap_or(1);
                
                // Remplir avec des zéros jusqu'au canal
                for _ in 1..channel {
                    command.push(0x00);
                }
                
                // Valeur du canal
                let value_int = match value {
                    Some(val) => (val * 255.0 / 100.0) as u8,
                    None => 0,
                };
                command.push(value_int);
                
                command
            },
        }
    }
    
    /// Analyse une réponse selon le protocole
    fn parse_response(&self, response: &[u8]) -> Result<(), LightingError> {
        match self.protocol {
            SerialProtocol::Simple => {
                // Protocole simple: OK\r\n ou ERROR,message\r\n
                let response_str = String::from_utf8_lossy(response);
                if response_str.starts_with("OK") {
                    Ok(())
                } else if response_str.starts_with("ERROR") {
                    Err(LightingError::CommunicationError(format!(
                        "Erreur du contrôleur: {}", response_str
                    )))
                } else {
                    Err(LightingError::CommunicationError(format!(
                        "Réponse invalide: {}", response_str
                    )))
                }
            },
            SerialProtocol::Binary => {
                // Protocole binaire: [ACK/NAK][STATUS][CHECKSUM]
                if response.len() < 3 {
                    return Err(LightingError::CommunicationError(
                        "Réponse trop courte".to_string()
                    ));
                }
                
                if response[0] == 0x06 {  // ACK
                    Ok(())
                } else if response[0] == 0x15 {  // NAK
                    Err(LightingError::CommunicationError(format!(
                        "Erreur du contrôleur: code {}", response[1]
                    )))
                } else {
                    Err(LightingError::CommunicationError(
                        "Réponse invalide".to_string()
                    ))
                }
            },
            SerialProtocol::ModbusRTU => {
                // Implémentation simplifiée de Modbus RTU
                if response.len() < 5 {
                    return Err(LightingError::CommunicationError(
                        "Réponse Modbus trop courte".to_string()
                    ));
                }
                
                // Vérifier le code de fonction
                if response[1] & 0x80 != 0 {
                    // Erreur Modbus
                    Err(LightingError::CommunicationError(format!(
                        "Erreur Modbus: code {}", response[2]
                    )))
                } else {
                    Ok(())
                }
            },
            SerialProtocol::DMX512 => {
                // DMX512 est unidirectionnel, pas de réponse attendue
                Ok(())
            },
        }
    }
}

#[async_trait]
impl LightingController for SerialLightingController {
    async fn initialize(&mut self, config: LightingConfig) -> Result<(), LightingError> {
        info!("Initialisation du contrôleur série avec {} canaux", config.channels.len());
        
        self.config = config.clone();
        self.channel_states.clear();
        
        // Extraire les paramètres de connexion
        let port_name = config.connection_params.get("port")
            .ok_or_else(|| LightingError::ConfigError("Paramètre 'port' manquant".to_string()))?;
            
        let baud_rate = config.connection_params.get("baud_rate")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(9600);
            
        let protocol = config.connection_params.get("protocol")
            .map(|s| match s.as_str() {
                "simple" => SerialProtocol::Simple,
                "binary" => SerialProtocol::Binary,
                "modbus" => SerialProtocol::ModbusRTU,
                "dmx512" => SerialProtocol::DMX512,
                _ => SerialProtocol::Simple,
            })
            .unwrap_or(SerialProtocol::Simple);
            
        self.protocol = protocol;
        
        // Extraire le délai de communication
        self.communication_delay_ms = config.connection_params.get("delay_ms")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10);
        
        // Ouvrir le port série
        self.open_port(port_name, baud_rate)?;
        
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
        
        Ok(())
    }
    
    async fn turn_on(&mut self, channel_id: &str) -> Result<(), LightingError> {
        debug!("Activation du canal: {}", channel_id);
        
        if let Some(state) = self.channel_states.get_mut(channel_id) {
            // Construire et envoyer la commande
            let command = self.build_command("ON", channel_id, None);
            let response = self.send_command(&command)?;
            self.parse_response(&response)?;
            
            // Mettre à jour l'état
            state.is_on = true;
            state.last_activation = Some(Instant::now());
            state.activation_count += 1;
            
            Ok(())
        } else {
            Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)))
        }
    }
    
    async fn turn_off(&mut self, channel_id: &str) -> Result<(), LightingError> {
        debug!("Désactivation du canal: {}", channel_id);
        
        if let Some(state) = self.channel_states.get_mut(channel_id) {
            if state.is_on {
                // Construire et envoyer la commande
                let command = self.build_command("OFF", channel_id, None);
                let response = self.send_command(&command)?;
                self.parse_response(&response)?;
                
                // Mettre à jour l'état
                state.is_on = false;
                
                // Mettre à jour la durée d'activation
                if let Some(last_activation) = state.last_activation {
                    let duration = last_activation.elapsed();
                    state.total_on_time_ms += duration.as_millis() as u64;
                }
            }
            
            Ok(())
        } else {
            Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)))
        }
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
            // Construire et envoyer la commande
            let command = self.build_command("INTENSITY", channel_id, Some(intensity));
            let response = self.send_command(&command)?;
            self.parse_response(&response)?;
            
            // Mettre à jour l'état
            state.current_intensity = intensity;
            state.config.intensity = intensity;
            
            Ok(())
        } else {
            Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)))
        }
    }
    
    async fn strobe(&mut self, channel_id: &str, duration_us: u64) -> Result<(), LightingError> {
        debug!("Stroboscope du canal {} pendant {} µs", channel_id, duration_us);
        
        if let Some(state) = self.channel_states.get_mut(channel_id) {
            // Convertir la durée en millisecondes pour la commande
            let duration_ms = duration_us as f64 / 1000.0;
            
            // Construire et envoyer la commande
            let command = self.build_command("STROBE", channel_id, Some(duration_ms));
            let response = self.send_command(&command)?;
            self.parse_response(&response)?;
            
            // Mettre à jour l'état
            state.is_on = true;
            state.last_activation = Some(Instant::now());
            state.activation_count += 1;
            
            // Attendre la fin du stroboscope
            time::sleep(Duration::from_micros(duration_us)).await;
            
            // Mettre à jour l'état
            state.is_on = false;
            
            // Mettre à jour la durée d'activation
            if let Some(last_activation) = state.last_activation {
                let duration = last_activation.elapsed();
                state.total_on_time_ms += duration.as_millis() as u64;
            }
            
            Ok(())
        } else {
            Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)))
        }
    }
    
    async fn trigger_all(&mut self) -> Result<(), LightingError> {
        debug!("Déclenchement de tous les canaux");
        
        // Construire et envoyer la commande
        let command = self.build_command("TRIGGER", "ALL", None);
        let response = self.send_command(&command)?;
        self.parse_response(&response)?;
        
        // Mettre à jour l'état de tous les canaux
        let now = Instant::now();
        for (_, state) in self.channel_states.iter_mut() {
            state.is_on = true;
            state.last_activation = Some(now);
            state.activation_count += 1;
        }
        
        // Attendre la durée maximale de stroboscope
        let max_duration = self.config.channels.iter()
            .map(|c| c.duration_us)
            .max()
            .unwrap_or(1000);
            
        time::sleep(Duration::from_micros(max_duration)).await;
        
        // Mettre à jour l'état de tous les canaux
        let elapsed = now.elapsed();
        for (_, state) in self.channel_states.iter_mut() {
            state.is_on = false;
            state.total_on_time_ms += elapsed.as_millis() as u64;
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
            // Mettre à jour le paramètre dans la configuration
            state.config.controller_params.insert(name.to_string(), value.to_string());
            
            // Construire et envoyer la commande
            let command = format!("PARAM,{},{},{}\r\n", channel_id, name, value).into_bytes();
            let response = self.send_command(&command)?;
            self.parse_response(&response)?;
            
            Ok(())
        } else {
            Err(LightingError::ConfigError(format!("Canal non trouvé: {}", channel_id)))
        }
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
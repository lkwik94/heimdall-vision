use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use tokio::time;
use tokio::net::TcpStream as AsyncTcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    LightingController, LightingConfig, LightChannelConfig, LightChannelState,
    LightingError, SyncMode, LightingType
};

/// Protocole de communication Ethernet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthernetProtocol {
    /// Protocole TCP simple (commandes ASCII)
    TCP,
    
    /// Protocole UDP
    UDP,
    
    /// Protocole Modbus TCP
    ModbusTCP,
    
    /// Protocole ArtNet (DMX sur Ethernet)
    ArtNet,
}

impl Default for EthernetProtocol {
    fn default() -> Self {
        Self::TCP
    }
}

/// Contrôleur d'éclairage Ethernet
pub struct EthernetLightingController {
    /// Identifiant du contrôleur
    id: String,
    
    /// Configuration actuelle
    config: LightingConfig,
    
    /// État des canaux
    channel_states: HashMap<String, LightChannelState>,
    
    /// Connexion TCP
    connection: Option<AsyncTcpStream>,
    
    /// Protocole de communication
    protocol: EthernetProtocol,
    
    /// Adresse du contrôleur
    address: String,
    
    /// Port du contrôleur
    port: u16,
    
    /// Délai de communication (ms)
    communication_delay_ms: u64,
}

impl EthernetLightingController {
    /// Crée un nouveau contrôleur Ethernet
    pub fn new(id: &str) -> Result<Self, LightingError> {
        info!("Initialisation du contrôleur d'éclairage Ethernet: {}", id);
        
        Ok(Self {
            id: id.to_string(),
            config: LightingConfig::default(),
            channel_states: HashMap::new(),
            connection: None,
            protocol: EthernetProtocol::default(),
            address: "127.0.0.1".to_string(),
            port: 502,
            communication_delay_ms: 10,
        })
    }
    
    /// Établit la connexion avec le contrôleur
    async fn connect(&mut self) -> Result<(), LightingError> {
        info!("Connexion au contrôleur Ethernet {}:{}", self.address, self.port);
        
        let addr = format!("{}:{}", self.address, self.port);
        
        match AsyncTcpStream::connect(&addr).await {
            Ok(stream) => {
                self.connection = Some(stream);
                Ok(())
            },
            Err(e) => {
                error!("Erreur lors de la connexion à {}:{}: {}", self.address, self.port, e);
                Err(LightingError::CommunicationError(format!(
                    "Erreur lors de la connexion à {}:{}: {}", self.address, self.port, e
                )))
            }
        }
    }
    
    /// Envoie une commande au contrôleur
    async fn send_command(&mut self, command: &[u8]) -> Result<Vec<u8>, LightingError> {
        if let Some(connection) = &mut self.connection {
            // Envoyer la commande
            match connection.write_all(command).await {
                Ok(_) => {
                    // Attendre la réponse
                    time::sleep(Duration::from_millis(self.communication_delay_ms)).await;
                    
                    // Lire la réponse
                    let mut response = vec![0u8; 1024];
                    match connection.read(&mut response).await {
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
            Err(LightingError::CommunicationError("Non connecté au contrôleur".to_string()))
        }
    }
    
    /// Construit une commande selon le protocole
    fn build_command(&self, action: &str, channel_id: &str, value: Option<f64>) -> Vec<u8> {
        match self.protocol {
            EthernetProtocol::TCP => {
                // Protocole TCP simple: ACTION,CHANNEL,VALUE\r\n
                let command = match value {
                    Some(val) => format!("{},{},{:.2}\r\n", action, channel_id, val),
                    None => format!("{},{}\r\n", action, channel_id),
                };
                command.into_bytes()
            },
            EthernetProtocol::UDP => {
                // Protocole UDP: similaire à TCP mais sans retour à la ligne
                let command = match value {
                    Some(val) => format!("{},{},{:.2}", action, channel_id, val),
                    None => format!("{},{}", action, channel_id),
                };
                command.into_bytes()
            },
            EthernetProtocol::ModbusTCP => {
                // Implémentation simplifiée de Modbus TCP
                // Dans un système réel, il faudrait utiliser une bibliothèque Modbus
                let mut command = Vec::new();
                
                // En-tête Modbus TCP
                // Transaction ID (2 octets)
                command.push(0x00);
                command.push(0x01);
                
                // Protocol ID (2 octets) - toujours 0 pour Modbus
                command.push(0x00);
                command.push(0x00);
                
                // Length (2 octets) - longueur des données suivantes
                command.push(0x00);
                command.push(0x06);  // 6 octets de données
                
                // Unit ID (1 octet)
                command.push(0x01);
                
                // Function code (1 octet)
                command.push(0x06);  // Write Single Register
                
                // Register address (2 octets)
                let register = channel_id.parse::<u16>().unwrap_or(0);
                command.push((register >> 8) as u8);
                command.push(register as u8);
                
                // Register value (2 octets)
                let value_int = match value {
                    Some(val) => (val * 65535.0 / 100.0) as u16,
                    None => 0,
                };
                command.push((value_int >> 8) as u8);
                command.push(value_int as u8);
                
                command
            },
            EthernetProtocol::ArtNet => {
                // Implémentation simplifiée d'ArtNet
                // Dans un système réel, il faudrait utiliser une bibliothèque ArtNet
                let mut command = Vec::new();
                
                // En-tête ArtNet
                // ID (8 octets) - "Art-Net"
                command.extend_from_slice(b"Art-Net\0");
                
                // OpCode (2 octets) - ArtDmx = 0x5000
                command.push(0x00);
                command.push(0x50);
                
                // Protocol Version (2 octets)
                command.push(0x00);
                command.push(0x0e);  // Version 14
                
                // Sequence (1 octet)
                command.push(0x00);
                
                // Physical (1 octet)
                command.push(0x00);
                
                // Universe (2 octets)
                command.push(0x00);
                command.push(0x00);
                
                // Length (2 octets) - nombre de canaux DMX
                command.push(0x02);
                command.push(0x00);  // 512 canaux
                
                // DMX data (512 octets)
                let mut dmx_data = vec![0u8; 512];
                
                // Définir la valeur du canal
                let channel = channel_id.parse::<usize>().unwrap_or(1);
                if channel > 0 && channel <= 512 {
                    let value_int = match value {
                        Some(val) => (val * 255.0 / 100.0) as u8,
                        None => 0,
                    };
                    dmx_data[channel - 1] = value_int;
                }
                
                command.extend_from_slice(&dmx_data);
                
                command
            },
        }
    }
    
    /// Analyse une réponse selon le protocole
    fn parse_response(&self, response: &[u8]) -> Result<(), LightingError> {
        match self.protocol {
            EthernetProtocol::TCP => {
                // Protocole TCP simple: OK\r\n ou ERROR,message\r\n
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
            EthernetProtocol::UDP => {
                // Pour UDP, on ne vérifie pas la réponse (protocole sans connexion)
                Ok(())
            },
            EthernetProtocol::ModbusTCP => {
                // Implémentation simplifiée de Modbus TCP
                if response.len() < 9 {
                    return Err(LightingError::CommunicationError(
                        "Réponse Modbus TCP trop courte".to_string()
                    ));
                }
                
                // Vérifier le code de fonction
                if response[7] & 0x80 != 0 {
                    // Erreur Modbus
                    Err(LightingError::CommunicationError(format!(
                        "Erreur Modbus: code {}", response[8]
                    )))
                } else {
                    Ok(())
                }
            },
            EthernetProtocol::ArtNet => {
                // ArtNet est généralement unidirectionnel, pas de réponse attendue
                Ok(())
            },
        }
    }
}

#[async_trait]
impl LightingController for EthernetLightingController {
    async fn initialize(&mut self, config: LightingConfig) -> Result<(), LightingError> {
        info!("Initialisation du contrôleur Ethernet avec {} canaux", config.channels.len());
        
        self.config = config.clone();
        self.channel_states.clear();
        
        // Extraire les paramètres de connexion
        self.address = config.connection_params.get("address")
            .cloned()
            .unwrap_or_else(|| "127.0.0.1".to_string());
            
        self.port = config.connection_params.get("port")
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(502);
            
        let protocol = config.connection_params.get("protocol")
            .map(|s| match s.as_str() {
                "tcp" => EthernetProtocol::TCP,
                "udp" => EthernetProtocol::UDP,
                "modbus_tcp" => EthernetProtocol::ModbusTCP,
                "artnet" => EthernetProtocol::ArtNet,
                _ => EthernetProtocol::TCP,
            })
            .unwrap_or(EthernetProtocol::TCP);
            
        self.protocol = protocol;
        
        // Extraire le délai de communication
        self.communication_delay_ms = config.connection_params.get("delay_ms")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10);
        
        // Établir la connexion
        self.connect().await?;
        
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
            let response = self.send_command(&command).await?;
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
                let response = self.send_command(&command).await?;
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
            let response = self.send_command(&command).await?;
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
            let response = self.send_command(&command).await?;
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
        let response = self.send_command(&command).await?;
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
            let response = self.send_command(&command).await?;
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
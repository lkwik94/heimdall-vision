use crate::{Camera, CameraConfig, CameraError, CameraFrame, PixelFormat, TriggerMode};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

/// Caméra simulée pour les tests
pub struct SimulatedCamera {
    /// Identifiant de la caméra
    id: String,
    
    /// Configuration actuelle
    config: CameraConfig,
    
    /// État d'acquisition
    is_acquiring: bool,
    
    /// Compteur de trames
    frame_counter: u64,
    
    /// Paramètres
    parameters: HashMap<String, String>,
}

impl SimulatedCamera {
    /// Crée une nouvelle instance de caméra simulée
    pub fn new(id: &str) -> Self {
        info!("Création d'une caméra simulée: {}", id);
        
        Self {
            id: id.to_string(),
            config: CameraConfig::default(),
            is_acquiring: false,
            frame_counter: 0,
            parameters: HashMap::new(),
        }
    }
    
    /// Génère une image simulée
    fn generate_image(&mut self) -> CameraFrame {
        // Calculer la taille de l'image
        let channels = match self.config.pixel_format {
            PixelFormat::Mono8 => 1,
            PixelFormat::Mono16 => 2,
            PixelFormat::RGB8 | PixelFormat::BGR8 => 3,
            PixelFormat::RGBA8 | PixelFormat::BGRA8 => 4,
            _ => 1,
        };
        
        let size = (self.config.width * self.config.height * channels) as usize;
        
        // Créer des données d'image simulées
        let mut data = vec![0u8; size];
        
        // Simuler un motif simple (damier)
        let block_size = 32;
        for y in 0..self.config.height {
            for x in 0..self.config.width {
                let block_x = (x / block_size) as usize;
                let block_y = (y / block_size) as usize;
                let is_white = (block_x + block_y) % 2 == 0;
                
                let index = ((y * self.config.width + x) * channels) as usize;
                
                let value = if is_white { 200 } else { 50 };
                
                match self.config.pixel_format {
                    PixelFormat::Mono8 => {
                        data[index] = value;
                    },
                    PixelFormat::Mono16 => {
                        data[index] = value;
                        data[index + 1] = value;
                    },
                    PixelFormat::RGB8 | PixelFormat::BGR8 => {
                        data[index] = value;
                        data[index + 1] = value;
                        data[index + 2] = value;
                    },
                    PixelFormat::RGBA8 | PixelFormat::BGRA8 => {
                        data[index] = value;
                        data[index + 1] = value;
                        data[index + 2] = value;
                        data[index + 3] = 255; // Alpha
                    },
                    _ => {
                        data[index] = value;
                    },
                }
            }
        }
        
        // Simuler une bouteille au centre
        let center_x = self.config.width / 2;
        let center_y = self.config.height / 2;
        let bottle_width = self.config.width / 5;
        let bottle_height = self.config.height / 2;
        
        for y in (center_y - bottle_height / 2)..(center_y + bottle_height / 2) {
            for x in (center_x - bottle_width / 2)..(center_x + bottle_width / 2) {
                if x < self.config.width && y < self.config.height {
                    let index = ((y * self.config.width + x) * channels) as usize;
                    
                    // Dessiner la bouteille
                    let bottle_value = 150;
                    
                    match self.config.pixel_format {
                        PixelFormat::Mono8 => {
                            data[index] = bottle_value;
                        },
                        PixelFormat::Mono16 => {
                            data[index] = bottle_value;
                            data[index + 1] = bottle_value;
                        },
                        PixelFormat::RGB8 | PixelFormat::BGR8 => {
                            // Couleur bleutée pour la bouteille
                            data[index] = 100;
                            data[index + 1] = 100;
                            data[index + 2] = 180;
                        },
                        PixelFormat::RGBA8 | PixelFormat::BGRA8 => {
                            data[index] = 100;
                            data[index + 1] = 100;
                            data[index + 2] = 180;
                            data[index + 3] = 255;
                        },
                        _ => {
                            data[index] = bottle_value;
                        },
                    }
                }
            }
        }
        
        // Simuler un défaut aléatoire (1 chance sur 5)
        if self.frame_counter % 5 == 0 {
            let defect_x = center_x + bottle_width / 4;
            let defect_y = center_y;
            let defect_size = 10;
            
            for y in (defect_y - defect_size)..(defect_y + defect_size) {
                for x in (defect_x - defect_size)..(defect_x + defect_size) {
                    if x < self.config.width && y < self.config.height {
                        let dx = x as i32 - defect_x as i32;
                        let dy = y as i32 - defect_y as i32;
                        let distance = (dx * dx + dy * dy) as f32;
                        
                        if distance < (defect_size * defect_size) as f32 {
                            let index = ((y * self.config.width + x) * channels) as usize;
                            
                            // Dessiner un défaut sombre
                            match self.config.pixel_format {
                                PixelFormat::Mono8 => {
                                    data[index] = 20;
                                },
                                PixelFormat::Mono16 => {
                                    data[index] = 20;
                                    data[index + 1] = 20;
                                },
                                PixelFormat::RGB8 | PixelFormat::BGR8 => {
                                    data[index] = 20;
                                    data[index + 1] = 20;
                                    data[index + 2] = 20;
                                },
                                PixelFormat::RGBA8 | PixelFormat::BGRA8 => {
                                    data[index] = 20;
                                    data[index + 1] = 20;
                                    data[index + 2] = 20;
                                    data[index + 3] = 255;
                                },
                                _ => {
                                    data[index] = 20;
                                },
                            }
                        }
                    }
                }
            }
        }
        
        // Créer des métadonnées
        let mut metadata = HashMap::new();
        metadata.insert("SimulatedCamera".to_string(), "true".to_string());
        metadata.insert("FrameCounter".to_string(), self.frame_counter.to_string());
        
        // Créer l'image
        let frame = CameraFrame {
            data,
            width: self.config.width,
            height: self.config.height,
            pixel_format: self.config.pixel_format,
            timestamp: SystemTime::now(),
            frame_id: self.frame_counter,
            metadata,
        };
        
        // Incrémenter le compteur de trames
        self.frame_counter += 1;
        
        frame
    }
}

#[async_trait]
impl Camera for SimulatedCamera {
    async fn initialize(&mut self, config: CameraConfig) -> Result<(), CameraError> {
        info!("Initialisation de la caméra simulée {} avec config: {:?}", self.id, config);
        
        self.config = config;
        
        // Initialiser les paramètres
        self.parameters.insert("Width".to_string(), self.config.width.to_string());
        self.parameters.insert("Height".to_string(), self.config.height.to_string());
        self.parameters.insert("FrameRate".to_string(), self.config.frame_rate.to_string());
        self.parameters.insert("ExposureTime".to_string(), self.config.exposure_time_us.to_string());
        self.parameters.insert("Gain".to_string(), self.config.gain_db.to_string());
        
        Ok(())
    }
    
    async fn start_acquisition(&mut self) -> Result<(), CameraError> {
        if self.is_acquiring {
            warn!("La caméra {} est déjà en cours d'acquisition", self.id);
            return Ok(());
        }
        
        info!("Démarrage de l'acquisition pour la caméra simulée {}", self.id);
        
        self.is_acquiring = true;
        
        Ok(())
    }
    
    async fn stop_acquisition(&mut self) -> Result<(), CameraError> {
        if !self.is_acquiring {
            warn!("La caméra {} n'est pas en cours d'acquisition", self.id);
            return Ok(());
        }
        
        info!("Arrêt de l'acquisition pour la caméra simulée {}", self.id);
        
        self.is_acquiring = false;
        
        Ok(())
    }
    
    async fn acquire_frame(&mut self) -> Result<CameraFrame, CameraError> {
        if !self.is_acquiring {
            return Err(CameraError::AcquisitionError(
                "La caméra n'est pas en cours d'acquisition".to_string()
            ));
        }
        
        debug!("Acquisition d'une image depuis la caméra simulée {}", self.id);
        
        // Simuler un délai d'acquisition basé sur la fréquence d'images
        if self.config.frame_rate > 0.0 {
            let frame_time_ms = (1000.0 / self.config.frame_rate) as u64;
            sleep(Duration::from_millis(frame_time_ms)).await;
        }
        
        // Générer une image simulée
        let frame = self.generate_image();
        
        Ok(frame)
    }
    
    async fn trigger(&mut self) -> Result<(), CameraError> {
        if !self.is_acquiring {
            return Err(CameraError::AcquisitionError(
                "La caméra n'est pas en cours d'acquisition".to_string()
            ));
        }
        
        if self.config.trigger_mode != TriggerMode::Software {
            return Err(CameraError::ConfigError(
                "La caméra n'est pas en mode de déclenchement logiciel".to_string()
            ));
        }
        
        info!("Déclenchement logiciel pour la caméra simulée {}", self.id);
        
        Ok(())
    }
    
    fn get_config(&self) -> CameraConfig {
        self.config.clone()
    }
    
    async fn set_parameter(&mut self, name: &str, value: &str) -> Result<(), CameraError> {
        debug!("Définition du paramètre '{}' à '{}' pour la caméra simulée {}", name, value, self.id);
        
        self.parameters.insert(name.to_string(), value.to_string());
        
        // Mettre à jour la configuration si nécessaire
        match name {
            "Width" => {
                if let Ok(width) = value.parse::<u32>() {
                    self.config.width = width;
                }
            },
            "Height" => {
                if let Ok(height) = value.parse::<u32>() {
                    self.config.height = height;
                }
            },
            "FrameRate" => {
                if let Ok(frame_rate) = value.parse::<f64>() {
                    self.config.frame_rate = frame_rate;
                }
            },
            "ExposureTime" => {
                if let Ok(exposure_time) = value.parse::<u64>() {
                    self.config.exposure_time_us = exposure_time;
                }
            },
            "Gain" => {
                if let Ok(gain) = value.parse::<f64>() {
                    self.config.gain_db = gain;
                }
            },
            "TriggerMode" => {
                match value {
                    "Off" => self.config.trigger_mode = TriggerMode::Continuous,
                    "Software" => self.config.trigger_mode = TriggerMode::Software,
                    "Hardware" => self.config.trigger_mode = TriggerMode::Hardware,
                    _ => {},
                }
            },
            _ => {},
        }
        
        Ok(())
    }
    
    async fn get_parameter(&self, name: &str) -> Result<String, CameraError> {
        debug!("Obtention du paramètre '{}' pour la caméra simulée {}", name, self.id);
        
        if let Some(value) = self.parameters.get(name) {
            return Ok(value.clone());
        }
        
        Err(CameraError::ConfigError(format!("Paramètre non trouvé: {}", name)))
    }
}
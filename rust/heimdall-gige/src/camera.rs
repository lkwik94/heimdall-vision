//! Module de gestion des caméras GigE Vision
//!
//! Ce module fournit les fonctionnalités pour initialiser, configurer
//! et contrôler les caméras GigE Vision.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use log::{debug, error, info, trace, warn};
use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};

use crate::config::CameraConfig;
use crate::diagnostics::CameraStatus;
use crate::error::{GigEError, with_recovery};
use crate::frame::{Frame, FrameMetadata};
use crate::sync::{SyncManager, SyncMode};

/// Initialise la bibliothèque Aravis
pub fn init_aravis() -> Result<(), GigEError> {
    info!("Initialisation de la bibliothèque Aravis");
    
    // En production, ceci initialiserait la bibliothèque Aravis
    // via aravis-rs
    
    // Simuler un délai d'initialisation
    std::thread::sleep(Duration::from_millis(100));
    
    Ok(())
}

/// Découvre les caméras GigE Vision disponibles sur le réseau
pub async fn discover_cameras() -> Result<Vec<CameraInfo>, GigEError> {
    info!("Découverte des caméras GigE Vision");
    
    // En production, ceci utiliserait aravis-rs pour découvrir les caméras
    
    // Simuler un délai de découverte
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Simuler la découverte de caméras
    let cameras = vec![
        CameraInfo {
            id: "GigE-Camera-1".to_string(),
            model: "Basler acA1920-50gm".to_string(),
            vendor: "Basler".to_string(),
            serial: "12345678".to_string(),
            ip_address: "169.254.1.1".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            firmware_version: "1.2.3".to_string(),
            capabilities: CameraCapabilities {
                pixel_formats: vec![
                    heimdall_camera::PixelFormat::Mono8,
                    heimdall_camera::PixelFormat::Mono16,
                ],
                max_width: 1920,
                max_height: 1080,
                min_exposure_us: 10,
                max_exposure_us: 1000000,
                min_gain_db: 0.0,
                max_gain_db: 24.0,
                max_frame_rate: 50.0,
                has_hardware_trigger: true,
                has_strobe_output: true,
            },
        },
        CameraInfo {
            id: "GigE-Camera-2".to_string(),
            model: "Basler acA1920-50gm".to_string(),
            vendor: "Basler".to_string(),
            serial: "12345679".to_string(),
            ip_address: "169.254.1.2".to_string(),
            mac_address: "00:11:22:33:44:56".to_string(),
            firmware_version: "1.2.3".to_string(),
            capabilities: CameraCapabilities {
                pixel_formats: vec![
                    heimdall_camera::PixelFormat::Mono8,
                    heimdall_camera::PixelFormat::Mono16,
                ],
                max_width: 1920,
                max_height: 1080,
                min_exposure_us: 10,
                max_exposure_us: 1000000,
                min_gain_db: 0.0,
                max_gain_db: 24.0,
                max_frame_rate: 50.0,
                has_hardware_trigger: true,
                has_strobe_output: true,
            },
        },
        CameraInfo {
            id: "GigE-Camera-3".to_string(),
            model: "Basler acA1920-50gm".to_string(),
            vendor: "Basler".to_string(),
            serial: "12345680".to_string(),
            ip_address: "169.254.1.3".to_string(),
            mac_address: "00:11:22:33:44:57".to_string(),
            firmware_version: "1.2.3".to_string(),
            capabilities: CameraCapabilities {
                pixel_formats: vec![
                    heimdall_camera::PixelFormat::Mono8,
                    heimdall_camera::PixelFormat::Mono16,
                ],
                max_width: 1920,
                max_height: 1080,
                min_exposure_us: 10,
                max_exposure_us: 1000000,
                min_gain_db: 0.0,
                max_gain_db: 24.0,
                max_frame_rate: 50.0,
                has_hardware_trigger: true,
                has_strobe_output: true,
            },
        },
        CameraInfo {
            id: "GigE-Camera-4".to_string(),
            model: "Basler acA1920-50gm".to_string(),
            vendor: "Basler".to_string(),
            serial: "12345681".to_string(),
            ip_address: "169.254.1.4".to_string(),
            mac_address: "00:11:22:33:44:58".to_string(),
            firmware_version: "1.2.3".to_string(),
            capabilities: CameraCapabilities {
                pixel_formats: vec![
                    heimdall_camera::PixelFormat::Mono8,
                    heimdall_camera::PixelFormat::Mono16,
                ],
                max_width: 1920,
                max_height: 1080,
                min_exposure_us: 10,
                max_exposure_us: 1000000,
                min_gain_db: 0.0,
                max_gain_db: 24.0,
                max_frame_rate: 50.0,
                has_hardware_trigger: true,
                has_strobe_output: true,
            },
        },
    ];
    
    Ok(cameras)
}

/// Informations sur une caméra GigE Vision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraInfo {
    /// Identifiant de la caméra
    pub id: String,
    
    /// Modèle de la caméra
    pub model: String,
    
    /// Fabricant de la caméra
    pub vendor: String,
    
    /// Numéro de série
    pub serial: String,
    
    /// Adresse IP
    pub ip_address: String,
    
    /// Adresse MAC
    pub mac_address: String,
    
    /// Version du firmware
    pub firmware_version: String,
    
    /// Capacités de la caméra
    pub capabilities: CameraCapabilities,
}

/// Capacités d'une caméra GigE Vision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraCapabilities {
    /// Formats de pixel supportés
    pub pixel_formats: Vec<heimdall_camera::PixelFormat>,
    
    /// Largeur maximale
    pub max_width: u32,
    
    /// Hauteur maximale
    pub max_height: u32,
    
    /// Temps d'exposition minimal (en microsecondes)
    pub min_exposure_us: u64,
    
    /// Temps d'exposition maximal (en microsecondes)
    pub max_exposure_us: u64,
    
    /// Gain minimal (en dB)
    pub min_gain_db: f64,
    
    /// Gain maximal (en dB)
    pub max_gain_db: f64,
    
    /// Fréquence d'acquisition maximale (en images par seconde)
    pub max_frame_rate: f64,
    
    /// Support du déclenchement matériel
    pub has_hardware_trigger: bool,
    
    /// Support de la sortie stroboscopique
    pub has_strobe_output: bool,
}

/// Caméra GigE Vision
pub struct GigECamera {
    /// Identifiant de la caméra
    id: String,
    
    /// Informations sur la caméra
    info: CameraInfo,
    
    /// Configuration actuelle
    config: CameraConfig,
    
    /// État d'acquisition
    is_acquiring: bool,
    
    /// Compteur de trames
    frame_counter: u64,
    
    /// Compteur d'erreurs
    error_counter: u64,
    
    /// Horodatage de la dernière image
    last_frame_time: Option<SystemTime>,
    
    /// Contexte Aravis (simulé ici, utiliserait aravis-rs en production)
    #[allow(dead_code)]
    context: Option<Arc<Mutex<AravisContext>>>,
    
    /// Statistiques de performance
    perf_stats: PerfStats,
}

/// Contexte Aravis (simulé pour l'exemple)
struct AravisContext {
    #[allow(dead_code)]
    device: String,
    #[allow(dead_code)]
    stream: String,
    #[allow(dead_code)]
    parameters: HashMap<String, String>,
}

/// Statistiques de performance
#[derive(Debug, Clone)]
struct PerfStats {
    /// Temps d'acquisition moyen (en millisecondes)
    acquisition_time_ms: f64,
    
    /// Nombre d'acquisitions
    acquisition_count: u64,
    
    /// Taux de perte de paquets (en pourcentage)
    packet_loss_rate: f64,
    
    /// Utilisation de la bande passante (en Mo/s)
    bandwidth_usage: f64,
    
    /// Température du capteur (en degrés Celsius)
    sensor_temperature: Option<f32>,
}

impl Default for PerfStats {
    fn default() -> Self {
        Self {
            acquisition_time_ms: 0.0,
            acquisition_count: 0,
            packet_loss_rate: 0.0,
            bandwidth_usage: 0.0,
            sensor_temperature: None,
        }
    }
}

impl GigECamera {
    /// Crée une nouvelle instance de caméra GigE Vision
    pub fn new(id: &str, info: CameraInfo) -> Result<Self, GigEError> {
        info!("Initialisation de la caméra GigE Vision: {}", id);
        
        Ok(Self {
            id: id.to_string(),
            info,
            config: CameraConfig::default(),
            is_acquiring: false,
            frame_counter: 0,
            error_counter: 0,
            last_frame_time: None,
            context: None,
            perf_stats: PerfStats::default(),
        })
    }
    
    /// Configure la caméra avec les paramètres spécifiés
    pub async fn configure(&mut self, config: CameraConfig) -> Result<(), GigEError> {
        info!("Configuration de la caméra {} avec {:?}", self.id, config);
        
        // Vérifier que la caméra n'est pas en cours d'acquisition
        if self.is_acquiring {
            return Err(GigEError::ConfigError(
                "Impossible de configurer la caméra pendant l'acquisition".to_string()
            ));
        }
        
        // Vérifier que le format de pixel est supporté
        if !self.info.capabilities.pixel_formats.contains(&config.pixel_format) {
            return Err(GigEError::ConfigError(format!(
                "Format de pixel non supporté: {:?}",
                config.pixel_format
            )));
        }
        
        // Vérifier les dimensions
        if config.width > self.info.capabilities.max_width || 
           config.height > self.info.capabilities.max_height {
            return Err(GigEError::ConfigError(format!(
                "Dimensions trop grandes: {}x{} (max: {}x{})",
                config.width, config.height,
                self.info.capabilities.max_width, self.info.capabilities.max_height
            )));
        }
        
        // Vérifier le temps d'exposition
        if config.exposure_time_us < self.info.capabilities.min_exposure_us || 
           config.exposure_time_us > self.info.capabilities.max_exposure_us {
            return Err(GigEError::ConfigError(format!(
                "Temps d'exposition hors limites: {} µs (min: {} µs, max: {} µs)",
                config.exposure_time_us,
                self.info.capabilities.min_exposure_us,
                self.info.capabilities.max_exposure_us
            )));
        }
        
        // Vérifier le gain
        if config.gain_db < self.info.capabilities.min_gain_db || 
           config.gain_db > self.info.capabilities.max_gain_db {
            return Err(GigEError::ConfigError(format!(
                "Gain hors limites: {:.1} dB (min: {:.1} dB, max: {:.1} dB)",
                config.gain_db,
                self.info.capabilities.min_gain_db,
                self.info.capabilities.max_gain_db
            )));
        }
        
        // Vérifier le mode de déclenchement
        if config.trigger_mode == heimdall_camera::TriggerMode::Hardware && 
           !self.info.capabilities.has_hardware_trigger {
            return Err(GigEError::ConfigError(
                "Le déclenchement matériel n'est pas supporté par cette caméra".to_string()
            ));
        }
        
        // Initialiser le contexte Aravis
        self.init_context(&config)?;
        
        // Enregistrer la configuration
        self.config = config;
        
        info!("Caméra {} configurée avec succès", self.id);
        
        Ok(())
    }
    
    /// Initialise le contexte Aravis
    fn init_context(&mut self, config: &CameraConfig) -> Result<(), GigEError> {
        // En production, ceci configurerait la caméra Aravis
        
        info!("Initialisation du contexte Aravis pour la caméra: {}", self.id);
        
        let context = AravisContext {
            device: self.id.clone(),
            stream: "stream0".to_string(),
            parameters: HashMap::new(),
        };
        
        self.context = Some(Arc::new(Mutex::new(context)));
        
        // Configurer les paramètres de base
        self.set_parameter_sync("Width", &config.width.to_string())?;
        self.set_parameter_sync("Height", &config.height.to_string())?;
        self.set_parameter_sync("FrameRate", &config.frame_rate.to_string())?;
        self.set_parameter_sync("ExposureTime", &config.exposure_time_us.to_string())?;
        self.set_parameter_sync("Gain", &config.gain_db.to_string())?;
        
        // Configurer le mode de déclenchement
        match config.trigger_mode {
            heimdall_camera::TriggerMode::Continuous => {
                self.set_parameter_sync("TriggerMode", "Off")?;
            },
            heimdall_camera::TriggerMode::Software => {
                self.set_parameter_sync("TriggerMode", "On")?;
                self.set_parameter_sync("TriggerSource", "Software")?;
            },
            heimdall_camera::TriggerMode::Hardware => {
                self.set_parameter_sync("TriggerMode", "On")?;
                self.set_parameter_sync("TriggerSource", "Line1")?;
            },
        }
        
        // Configurer le format de pixel
        let pixel_format_str = match config.pixel_format {
            heimdall_camera::PixelFormat::Mono8 => "Mono8",
            heimdall_camera::PixelFormat::Mono16 => "Mono16",
            heimdall_camera::PixelFormat::RGB8 => "RGB8",
            heimdall_camera::PixelFormat::BGR8 => "BGR8",
            heimdall_camera::PixelFormat::RGBA8 => "RGBA8",
            heimdall_camera::PixelFormat::BGRA8 => "BGRA8",
            heimdall_camera::PixelFormat::YUV422 => "YUV422",
            heimdall_camera::PixelFormat::YUV422Packed => "YUV422Packed",
            heimdall_camera::PixelFormat::BayerRG8 => "BayerRG8",
            heimdall_camera::PixelFormat::BayerGB8 => "BayerGB8",
            heimdall_camera::PixelFormat::BayerGR8 => "BayerGR8",
            heimdall_camera::PixelFormat::BayerBG8 => "BayerBG8",
        };
        self.set_parameter_sync("PixelFormat", pixel_format_str)?;
        
        // Configurer la région d'intérêt
        if config.roi_enabled {
            self.set_parameter_sync("OffsetX", &config.roi_x.to_string())?;
            self.set_parameter_sync("OffsetY", &config.roi_y.to_string())?;
            self.set_parameter_sync("Width", &config.roi_width.to_string())?;
            self.set_parameter_sync("Height", &config.roi_height.to_string())?;
        }
        
        // Configurer les paramètres réseau
        self.set_parameter_sync("GevSCPSPacketSize", &config.packet_size.to_string())?;
        self.set_parameter_sync("GevSCPD", &config.packet_delay.to_string())?;
        
        // Configurer le nombre de tampons
        self.set_parameter_sync("NumBuffers", &config.buffer_count.to_string())?;
        
        Ok(())
    }
    
    /// Optimise les paramètres réseau pour les performances
    pub async fn optimize_network_parameters(&mut self) -> Result<(), GigEError> {
        info!("Optimisation des paramètres réseau pour la caméra {}", self.id);
        
        // En production, ceci déterminerait la MTU optimale et configurerait
        // la taille des paquets et le délai inter-paquets
        
        // Configurer pour Jumbo Frames (9000 octets)
        self.set_parameter_sync("GevSCPSPacketSize", "9000")?;
        
        // Réduire le délai inter-paquets pour maximiser la bande passante
        self.set_parameter_sync("GevSCPD", "0")?;
        
        // Augmenter le nombre de tampons pour éviter les pertes de trames
        self.set_parameter_sync("NumBuffers", "20")?;
        
        // Activer le streaming en continu
        self.set_parameter_sync("StreamHoldEnable", "false")?;
        
        // Configurer la priorité de paquet
        self.set_parameter_sync("GevGVSPExtendedIDMode", "On")?;
        
        info!("Paramètres réseau optimisés pour la caméra {}", self.id);
        
        Ok(())
    }
    
    /// Configure la synchronisation matérielle
    pub async fn configure_hardware_sync(&mut self, sync_manager: &SyncManager) -> Result<(), GigEError> {
        info!("Configuration de la synchronisation matérielle pour la caméra {}", self.id);
        
        // Vérifier que la caméra supporte le déclenchement matériel
        if !self.info.capabilities.has_hardware_trigger {
            return Err(GigEError::ConfigError(
                "Le déclenchement matériel n'est pas supporté par cette caméra".to_string()
            ));
        }
        
        // Obtenir la configuration de synchronisation
        let sync_config = sync_manager.get_config();
        
        // Configurer le mode de déclenchement
        self.set_parameter_sync("TriggerMode", "On")?;
        
        // Configurer la source de déclenchement
        let trigger_source = match sync_config.trigger_source {
            crate::sync::TriggerSource::Line1 => "Line1",
            crate::sync::TriggerSource::Line2 => "Line2",
            crate::sync::TriggerSource::Line3 => "Line3",
            crate::sync::TriggerSource::Line4 => "Line4",
            crate::sync::TriggerSource::Encoder => "Encoder",
            crate::sync::TriggerSource::Timer => "Timer",
        };
        self.set_parameter_sync("TriggerSource", trigger_source)?;
        
        // Configurer le délai de déclenchement
        self.set_parameter_sync("TriggerDelay", &sync_config.trigger_delay_us.to_string())?;
        
        // Configurer la sortie stroboscopique si supportée
        if self.info.capabilities.has_strobe_output {
            self.set_parameter_sync("StrobeEnable", "true")?;
            self.set_parameter_sync("StrobeSource", "ExposureActive")?;
        }
        
        info!("Synchronisation matérielle configurée pour la caméra {}", self.id);
        
        Ok(())
    }
    
    /// Optimise les paramètres de caméra pour l'inspection de bouteilles
    pub async fn optimize_parameters_for_bottle_inspection(&mut self) -> Result<(), GigEError> {
        info!("Optimisation des paramètres pour l'inspection de bouteilles sur la caméra {}", self.id);
        
        // Configurer le temps d'exposition pour les bouteilles en mouvement
        // (court pour éviter le flou de mouvement)
        self.set_parameter_sync("ExposureTime", "2000")?; // 2 ms
        
        // Augmenter le gain pour compenser l'exposition courte
        self.set_parameter_sync("Gain", "6.0")?; // 6 dB
        
        // Configurer la région d'intérêt pour se concentrer sur la zone de la bouteille
        self.set_parameter_sync("OffsetX", "400")?;
        self.set_parameter_sync("OffsetY", "200")?;
        self.set_parameter_sync("Width", "1120")?;
        self.set_parameter_sync("Height", "800")?;
        
        // Activer l'amélioration de contraste
        self.set_parameter_sync("ContrastEnable", "true")?;
        self.set_parameter_sync("ContrastLevel", "2")?;
        
        // Configurer la correction gamma
        self.set_parameter_sync("GammaEnable", "true")?;
        self.set_parameter_sync("Gamma", "0.7")?;
        
        // Mettre à jour la configuration locale
        self.config.exposure_time_us = 2000;
        self.config.gain_db = 6.0;
        self.config.roi_enabled = true;
        self.config.roi_x = 400;
        self.config.roi_y = 200;
        self.config.roi_width = 1120;
        self.config.roi_height = 800;
        
        info!("Paramètres optimisés pour l'inspection de bouteilles sur la caméra {}", self.id);
        
        Ok(())
    }
    
    /// Démarre l'acquisition d'images
    pub async fn start_acquisition(&mut self) -> Result<(), GigEError> {
        if self.is_acquiring {
            warn!("La caméra {} est déjà en cours d'acquisition", self.id);
            return Ok(());
        }
        
        info!("Démarrage de l'acquisition pour la caméra {}", self.id);
        
        // Vérifier que la caméra est configurée
        if self.context.is_none() {
            return Err(GigEError::InitError(
                "La caméra n'est pas initialisée".to_string()
            ));
        }
        
        // En production, ceci démarrerait le flux d'acquisition Aravis
        self.is_acquiring = true;
        self.frame_counter = 0;
        self.error_counter = 0;
        self.last_frame_time = None;
        
        // Réinitialiser les statistiques de performance
        self.perf_stats = PerfStats::default();
        
        // Enregistrer les métriques
        gauge!("gige.camera.acquiring", 1.0, "camera" => self.id.clone());
        
        Ok(())
    }
    
    /// Arrête l'acquisition d'images
    pub async fn stop_acquisition(&mut self) -> Result<(), GigEError> {
        if !self.is_acquiring {
            warn!("La caméra {} n'est pas en cours d'acquisition", self.id);
            return Ok(());
        }
        
        info!("Arrêt de l'acquisition pour la caméra {}", self.id);
        
        // En production, ceci arrêterait le flux d'acquisition Aravis
        self.is_acquiring = false;
        
        // Enregistrer les métriques
        gauge!("gige.camera.acquiring", 0.0, "camera" => self.id.clone());
        
        Ok(())
    }
    
    /// Acquiert une image
    pub async fn acquire_frame(&mut self) -> Result<Frame, GigEError> {
        if !self.is_acquiring {
            return Err(GigEError::AcquisitionError(
                "La caméra n'est pas en cours d'acquisition".to_string()
            ));
        }
        
        trace!("Acquisition d'une image depuis la caméra {}", self.id);
        
        // Mesurer le temps d'acquisition
        let start_time = Instant::now();
        
        // En production, ceci attendrait une image du flux Aravis
        // Ici, nous simulons une image
        
        // Utiliser la fonction de reprise en cas d'erreur
        let frame = with_recovery(
            || async {
                // Simuler un délai d'acquisition
                tokio::time::sleep(Duration::from_millis(5)).await;
                
                // Simuler une erreur occasionnelle (1% de chance)
                if rand::random::<f32>() < 0.01 {
                    return Err(anyhow!("Erreur d'acquisition simulée"));
                }
                
                // Calculer la taille de l'image
                let width = if self.config.roi_enabled { self.config.roi_width } else { self.config.width };
                let height = if self.config.roi_enabled { self.config.roi_height } else { self.config.height };
                
                let channels = match self.config.pixel_format {
                    heimdall_camera::PixelFormat::Mono8 => 1,
                    heimdall_camera::PixelFormat::Mono16 => 2,
                    heimdall_camera::PixelFormat::RGB8 | heimdall_camera::PixelFormat::BGR8 => 3,
                    heimdall_camera::PixelFormat::RGBA8 | heimdall_camera::PixelFormat::BGRA8 => 4,
                    _ => 1,
                };
                
                let size = (width * height * channels) as usize;
                
                // Créer des données d'image simulées
                let mut data = vec![0u8; size];
                
                // Simuler un motif simple (gradient)
                for y in 0..height {
                    for x in 0..width {
                        let index = ((y * width + x) * channels) as usize;
                        
                        // Créer un motif de gradient
                        let value = ((x * 255) / width) as u8;
                        
                        match self.config.pixel_format {
                            heimdall_camera::PixelFormat::Mono8 => {
                                data[index] = value;
                            },
                            heimdall_camera::PixelFormat::Mono16 => {
                                data[index] = value;
                                data[index + 1] = value;
                            },
                            heimdall_camera::PixelFormat::RGB8 | heimdall_camera::PixelFormat::BGR8 => {
                                data[index] = value;
                                data[index + 1] = ((y * 255) / height) as u8;
                                data[index + 2] = ((x + y) * 255 / (width + height)) as u8;
                            },
                            heimdall_camera::PixelFormat::RGBA8 | heimdall_camera::PixelFormat::BGRA8 => {
                                data[index] = value;
                                data[index + 1] = ((y * 255) / height) as u8;
                                data[index + 2] = ((x + y) * 255 / (width + height)) as u8;
                                data[index + 3] = 255; // Alpha
                            },
                            _ => {
                                data[index] = value;
                            },
                        }
                    }
                }
                
                // Simuler une bouteille au centre
                let center_x = width / 2;
                let center_y = height / 2;
                let bottle_width = width / 5;
                let bottle_height = height / 2;
                
                for y in (center_y - bottle_height / 2)..(center_y + bottle_height / 2) {
                    for x in (center_x - bottle_width / 2)..(center_x + bottle_width / 2) {
                        if x < width && y < height {
                            let index = ((y * width + x) * channels) as usize;
                            
                            // Dessiner la bouteille
                            match self.config.pixel_format {
                                heimdall_camera::PixelFormat::Mono8 => {
                                    data[index] = 200;
                                },
                                heimdall_camera::PixelFormat::Mono16 => {
                                    data[index] = 200;
                                    data[index + 1] = 200;
                                },
                                heimdall_camera::PixelFormat::RGB8 | heimdall_camera::PixelFormat::BGR8 => {
                                    data[index] = 200;
                                    data[index + 1] = 200;
                                    data[index + 2] = 200;
                                },
                                heimdall_camera::PixelFormat::RGBA8 | heimdall_camera::PixelFormat::BGRA8 => {
                                    data[index] = 200;
                                    data[index + 1] = 200;
                                    data[index + 2] = 200;
                                    data[index + 3] = 255;
                                },
                                _ => {
                                    data[index] = 200;
                                },
                            }
                        }
                    }
                }
                
                // Simuler un défaut aléatoire sur la bouteille (10% de chance)
                if rand::random::<f32>() < 0.1 {
                    let defect_x = center_x + (rand::random::<i32>() % (bottle_width as i32 / 2)) as u32;
                    let defect_y = center_y + (rand::random::<i32>() % (bottle_height as i32 / 2)) as u32;
                    let defect_size = 5 + (rand::random::<u32>() % 10);
                    
                    for y in (defect_y - defect_size)..(defect_y + defect_size) {
                        for x in (defect_x - defect_size)..(defect_x + defect_size) {
                            if x < width && y < height {
                                let index = ((y * width + x) * channels) as usize;
                                
                                // Dessiner le défaut
                                match self.config.pixel_format {
                                    heimdall_camera::PixelFormat::Mono8 => {
                                        data[index] = 50;
                                    },
                                    heimdall_camera::PixelFormat::Mono16 => {
                                        data[index] = 50;
                                        data[index + 1] = 50;
                                    },
                                    heimdall_camera::PixelFormat::RGB8 | heimdall_camera::PixelFormat::BGR8 => {
                                        data[index] = 50;
                                        data[index + 1] = 50;
                                        data[index + 2] = 50;
                                    },
                                    heimdall_camera::PixelFormat::RGBA8 | heimdall_camera::PixelFormat::BGRA8 => {
                                        data[index] = 50;
                                        data[index + 1] = 50;
                                        data[index + 2] = 50;
                                        data[index + 3] = 255;
                                    },
                                    _ => {
                                        data[index] = 50;
                                    },
                                }
                            }
                        }
                    }
                }
                
                // Créer les métadonnées
                let mut metadata = FrameMetadata::new(&self.id);
                metadata.frame_id = self.frame_counter;
                metadata.exposure_time_us = self.config.exposure_time_us;
                metadata.gain_db = self.config.gain_db;
                metadata.sensor_temperature = Some(35.0 + (rand::random::<f32>() * 2.0 - 1.0));
                
                // Ajouter des métadonnées supplémentaires
                metadata.add_extra("model", &self.info.model);
                metadata.add_extra("vendor", &self.info.vendor);
                metadata.add_extra("serial", &self.info.serial);
                
                // Créer l'image
                let frame = Frame::new(
                    data,
                    width,
                    height,
                    self.config.pixel_format,
                    metadata,
                );
                
                Ok::<Frame, anyhow::Error>(frame)
            },
            |e| GigEError::AcquisitionError(format!("Erreur d'acquisition: {}", e)),
        ).await?;
        
        // Mettre à jour les compteurs et statistiques
        self.frame_counter += 1;
        self.last_frame_time = Some(SystemTime::now());
        
        // Calculer le temps d'acquisition
        let acquisition_time = start_time.elapsed();
        
        // Mettre à jour les statistiques de performance
        self.perf_stats.acquisition_count += 1;
        self.perf_stats.acquisition_time_ms = (self.perf_stats.acquisition_time_ms * (self.perf_stats.acquisition_count - 1) as f64
            + acquisition_time.as_secs_f64() * 1000.0) / self.perf_stats.acquisition_count as f64;
        
        // Simuler des statistiques réseau
        self.perf_stats.packet_loss_rate = 0.01 * rand::random::<f64>();
        self.perf_stats.bandwidth_usage = (width * height * 8) as f64 / 1_000_000.0 * self.config.frame_rate;
        self.perf_stats.sensor_temperature = frame.metadata.sensor_temperature;
        
        // Enregistrer les métriques
        counter!("gige.camera.frames", 1, "camera" => self.id.clone());
        histogram!("gige.camera.acquisition_time_ms", acquisition_time.as_secs_f64() * 1000.0, "camera" => self.id.clone());
        gauge!("gige.camera.packet_loss_rate", self.perf_stats.packet_loss_rate, "camera" => self.id.clone());
        gauge!("gige.camera.bandwidth_usage", self.perf_stats.bandwidth_usage, "camera" => self.id.clone());
        
        if let Some(temp) = self.perf_stats.sensor_temperature {
            gauge!("gige.camera.sensor_temperature", temp as f64, "camera" => self.id.clone());
        }
        
        Ok(frame)
    }
    
    /// Déclenche l'acquisition d'une image (mode Software)
    pub async fn trigger(&mut self) -> Result<(), GigEError> {
        if !self.is_acquiring {
            return Err(GigEError::AcquisitionError(
                "La caméra n'est pas en cours d'acquisition".to_string()
            ));
        }
        
        if self.config.trigger_mode != heimdall_camera::TriggerMode::Software {
            return Err(GigEError::ConfigError(
                "La caméra n'est pas en mode de déclenchement logiciel".to_string()
            ));
        }
        
        debug!("Déclenchement logiciel pour la caméra {}", self.id);
        
        // En production, ceci enverrait un déclenchement logiciel via Aravis
        self.set_parameter_sync("TriggerSoftware", "1")?;
        
        Ok(())
    }
    
    /// Obtient l'état actuel de la caméra
    pub async fn get_status(&self) -> Result<CameraStatus, GigEError> {
        let status = CameraStatus {
            id: self.id.clone(),
            connected: self.context.is_some(),
            acquiring: self.is_acquiring,
            sensor_temperature: self.perf_stats.sensor_temperature,
            housing_temperature: None,
            bandwidth_usage: Some(self.perf_stats.bandwidth_usage),
            packet_loss_rate: Some(self.perf_stats.packet_loss_rate),
            frame_count: self.frame_counter,
            error_count: self.error_counter,
            last_frame_time: self.last_frame_time,
            exposure_time_us: self.config.exposure_time_us,
            gain_db: self.config.gain_db,
            image_stats: None,
            recent_errors: Vec::new(),
        };
        
        Ok(status)
    }
    
    /// Définit un paramètre de manière synchrone
    fn set_parameter_sync(&self, name: &str, value: &str) -> Result<(), GigEError> {
        // En production, ceci utiliserait aravis-rs pour définir un paramètre
        
        debug!("Définition du paramètre '{}' à '{}' pour la caméra {}", name, value, self.id);
        
        if let Some(context) = &self.context {
            let mut context = context.lock().unwrap();
            context.parameters.insert(name.to_string(), value.to_string());
        } else {
            return Err(GigEError::ConfigError(
                "Le contexte Aravis n'est pas initialisé".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Obtient un paramètre de manière synchrone
    fn get_parameter_sync(&self, name: &str) -> Result<String, GigEError> {
        // En production, ceci utiliserait aravis-rs pour obtenir un paramètre
        
        debug!("Obtention du paramètre '{}' pour la caméra {}", name, self.id);
        
        if let Some(context) = &self.context {
            let context = context.lock().unwrap();
            if let Some(value) = context.parameters.get(name) {
                return Ok(value.clone());
            }
        }
        
        Err(GigEError::ConfigError(format!("Paramètre non trouvé: {}", name)))
    }
    
    /// Obtient l'identifiant de la caméra
    pub fn get_id(&self) -> &str {
        &self.id
    }
    
    /// Obtient les informations sur la caméra
    pub fn get_info(&self) -> &CameraInfo {
        &self.info
    }
    
    /// Obtient la configuration actuelle
    pub fn get_config(&self) -> &CameraConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_camera() -> Result<GigECamera, GigEError> {
        let info = CameraInfo {
            id: "test-camera".to_string(),
            model: "Test Model".to_string(),
            vendor: "Test Vendor".to_string(),
            serial: "12345".to_string(),
            ip_address: "192.168.1.100".to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            firmware_version: "1.0.0".to_string(),
            capabilities: CameraCapabilities {
                pixel_formats: vec![
                    heimdall_camera::PixelFormat::Mono8,
                    heimdall_camera::PixelFormat::Mono16,
                ],
                max_width: 1920,
                max_height: 1080,
                min_exposure_us: 10,
                max_exposure_us: 1000000,
                min_gain_db: 0.0,
                max_gain_db: 24.0,
                max_frame_rate: 50.0,
                has_hardware_trigger: true,
                has_strobe_output: true,
            },
        };
        
        GigECamera::new("test-camera", info)
    }
    
    #[tokio::test]
    async fn test_camera_creation() {
        let camera = create_test_camera();
        assert!(camera.is_ok());
        
        let camera = camera.unwrap();
        assert_eq!(camera.get_id(), "test-camera");
        assert_eq!(camera.get_info().model, "Test Model");
        assert_eq!(camera.get_info().vendor, "Test Vendor");
    }
    
    #[tokio::test]
    async fn test_camera_configuration() {
        let mut camera = create_test_camera().unwrap();
        
        let config = CameraConfig {
            pixel_format: heimdall_camera::PixelFormat::Mono8,
            width: 1280,
            height: 720,
            frame_rate: 30.0,
            exposure_time_us: 10000,
            gain_db: 6.0,
            trigger_mode: heimdall_camera::TriggerMode::Continuous,
            roi_enabled: false,
            roi_x: 0,
            roi_y: 0,
            roi_width: 1280,
            roi_height: 720,
            packet_size: 1500,
            packet_delay: 0,
            buffer_count: 10,
        };
        
        let result = camera.configure(config.clone()).await;
        assert!(result.is_ok());
        
        assert_eq!(camera.get_config().pixel_format, heimdall_camera::PixelFormat::Mono8);
        assert_eq!(camera.get_config().width, 1280);
        assert_eq!(camera.get_config().height, 720);
        assert_eq!(camera.get_config().frame_rate, 30.0);
        assert_eq!(camera.get_config().exposure_time_us, 10000);
        assert_eq!(camera.get_config().gain_db, 6.0);
    }
    
    #[tokio::test]
    async fn test_camera_acquisition() {
        let mut camera = create_test_camera().unwrap();
        
        let config = CameraConfig {
            pixel_format: heimdall_camera::PixelFormat::Mono8,
            width: 640,
            height: 480,
            frame_rate: 30.0,
            exposure_time_us: 10000,
            gain_db: 0.0,
            trigger_mode: heimdall_camera::TriggerMode::Continuous,
            roi_enabled: false,
            roi_x: 0,
            roi_y: 0,
            roi_width: 640,
            roi_height: 480,
            packet_size: 1500,
            packet_delay: 0,
            buffer_count: 10,
        };
        
        camera.configure(config).await.unwrap();
        
        // Démarrer l'acquisition
        let result = camera.start_acquisition().await;
        assert!(result.is_ok());
        
        // Acquérir une image
        let frame = camera.acquire_frame().await;
        assert!(frame.is_ok());
        
        let frame = frame.unwrap();
        assert_eq!(frame.width, 640);
        assert_eq!(frame.height, 480);
        assert_eq!(frame.pixel_format, heimdall_camera::PixelFormat::Mono8);
        assert_eq!(frame.data.len(), 640 * 480);
        
        // Vérifier les métadonnées
        assert_eq!(frame.metadata.camera_id, "test-camera");
        assert_eq!(frame.metadata.frame_id, 0);
        assert_eq!(frame.metadata.exposure_time_us, 10000);
        assert_eq!(frame.metadata.gain_db, 0.0);
        
        // Arrêter l'acquisition
        let result = camera.stop_acquisition().await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_camera_trigger() {
        let mut camera = create_test_camera().unwrap();
        
        let config = CameraConfig {
            pixel_format: heimdall_camera::PixelFormat::Mono8,
            width: 640,
            height: 480,
            frame_rate: 30.0,
            exposure_time_us: 10000,
            gain_db: 0.0,
            trigger_mode: heimdall_camera::TriggerMode::Software,
            roi_enabled: false,
            roi_x: 0,
            roi_y: 0,
            roi_width: 640,
            roi_height: 480,
            packet_size: 1500,
            packet_delay: 0,
            buffer_count: 10,
        };
        
        camera.configure(config).await.unwrap();
        camera.start_acquisition().await.unwrap();
        
        // Déclencher l'acquisition
        let result = camera.trigger().await;
        assert!(result.is_ok());
        
        // Acquérir l'image déclenchée
        let frame = camera.acquire_frame().await;
        assert!(frame.is_ok());
        
        camera.stop_acquisition().await.unwrap();
    }
}
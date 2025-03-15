use std::sync::Arc;
use thiserror::Error;
use log::{debug, error, info, warn};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod aravis;
pub mod simulator;

/// Erreur liée à la caméra
#[derive(Error, Debug)]
pub enum CameraError {
    #[error("Erreur d'initialisation de la caméra: {0}")]
    InitError(String),

    #[error("Erreur de configuration de la caméra: {0}")]
    ConfigError(String),

    #[error("Erreur d'acquisition d'image: {0}")]
    AcquisitionError(String),

    #[error("Caméra non trouvée: {0}")]
    NotFound(String),

    #[error("Erreur de conversion d'image: {0}")]
    ConversionError(String),

    #[error("Erreur d'aravis: {0}")]
    AravisError(String),
}

/// Format d'image supporté
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    Mono8,
    Mono16,
    RGB8,
    BGR8,
    RGBA8,
    BGRA8,
    YUV422,
    YUV422Packed,
    BayerRG8,
    BayerGB8,
    BayerGR8,
    BayerBG8,
}

/// Configuration de la caméra
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    /// Identifiant de la caméra (nom, adresse IP, ID, etc.)
    pub id: String,
    
    /// Format de pixel
    pub pixel_format: PixelFormat,
    
    /// Largeur de l'image
    pub width: u32,
    
    /// Hauteur de l'image
    pub height: u32,
    
    /// Fréquence d'acquisition (images par seconde)
    pub frame_rate: f64,
    
    /// Temps d'exposition en microsecondes
    pub exposure_time_us: u64,
    
    /// Gain en dB
    pub gain_db: f64,
    
    /// Mode de déclenchement (continu, logiciel, matériel)
    pub trigger_mode: TriggerMode,
    
    /// Paramètres spécifiques au fabricant
    pub vendor_params: std::collections::HashMap<String, String>,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            pixel_format: PixelFormat::Mono8,
            width: 1280,
            height: 1024,
            frame_rate: 30.0,
            exposure_time_us: 10000,
            gain_db: 0.0,
            trigger_mode: TriggerMode::Continuous,
            vendor_params: std::collections::HashMap::new(),
        }
    }
}

/// Mode de déclenchement de la caméra
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerMode {
    /// Acquisition continue
    Continuous,
    
    /// Déclenchement logiciel
    Software,
    
    /// Déclenchement matériel (ligne d'entrée)
    Hardware,
}

/// Image acquise par la caméra
#[derive(Debug, Clone)]
pub struct CameraFrame {
    /// Données brutes de l'image
    pub data: Vec<u8>,
    
    /// Largeur de l'image
    pub width: u32,
    
    /// Hauteur de l'image
    pub height: u32,
    
    /// Format de pixel
    pub pixel_format: PixelFormat,
    
    /// Horodatage de l'acquisition
    pub timestamp: std::time::SystemTime,
    
    /// Numéro de trame
    pub frame_id: u64,
    
    /// Métadonnées supplémentaires
    pub metadata: std::collections::HashMap<String, String>,
}

/// Interface de caméra
#[async_trait]
pub trait Camera: Send + Sync {
    /// Initialise la caméra avec la configuration spécifiée
    async fn initialize(&mut self, config: CameraConfig) -> Result<(), CameraError>;
    
    /// Démarre l'acquisition d'images
    async fn start_acquisition(&mut self) -> Result<(), CameraError>;
    
    /// Arrête l'acquisition d'images
    async fn stop_acquisition(&mut self) -> Result<(), CameraError>;
    
    /// Acquiert une image (bloquant)
    async fn acquire_frame(&mut self) -> Result<CameraFrame, CameraError>;
    
    /// Déclenche l'acquisition d'une image (mode Software)
    async fn trigger(&mut self) -> Result<(), CameraError>;
    
    /// Obtient la configuration actuelle
    fn get_config(&self) -> CameraConfig;
    
    /// Définit un paramètre spécifique
    async fn set_parameter(&mut self, name: &str, value: &str) -> Result<(), CameraError>;
    
    /// Obtient un paramètre spécifique
    async fn get_parameter(&self, name: &str) -> Result<String, CameraError>;
}

/// Fabrique de caméras
pub struct CameraFactory;

impl CameraFactory {
    /// Crée une nouvelle instance de caméra
    pub fn create(camera_type: &str, id: &str) -> Result<Box<dyn Camera>, CameraError> {
        match camera_type {
            "aravis" => {
                info!("Création d'une caméra Aravis avec ID: {}", id);
                Ok(Box::new(aravis::AravisCamera::new(id)?))
            },
            "simulator" => {
                info!("Création d'une caméra simulée avec ID: {}", id);
                Ok(Box::new(simulator::SimulatedCamera::new(id)))
            },
            _ => {
                error!("Type de caméra non supporté: {}", camera_type);
                Err(CameraError::InitError(format!("Type de caméra non supporté: {}", camera_type)))
            }
        }
    }
    
    /// Énumère les caméras disponibles
    pub fn enumerate() -> Vec<(String, String)> {
        let mut cameras = Vec::new();
        
        // Ajouter les caméras Aravis
        match aravis::AravisCamera::enumerate() {
            Ok(aravis_cameras) => cameras.extend(aravis_cameras),
            Err(e) => warn!("Erreur lors de l'énumération des caméras Aravis: {}", e),
        }
        
        // Ajouter une caméra simulée
        cameras.push(("simulator".to_string(), "simulated_camera".to_string()));
        
        cameras
    }
}

/// Convertit une image de caméra en image OpenCV
#[cfg(feature = "opencv")]
pub fn to_opencv_mat(frame: &CameraFrame) -> Result<opencv::core::Mat, CameraError> {
    use opencv::{core, imgproc, prelude::*};
    
    let mat_type = match frame.pixel_format {
        PixelFormat::Mono8 => core::CV_8UC1,
        PixelFormat::Mono16 => core::CV_16UC1,
        PixelFormat::RGB8 => core::CV_8UC3,
        PixelFormat::BGR8 => core::CV_8UC3,
        PixelFormat::RGBA8 => core::CV_8UC4,
        PixelFormat::BGRA8 => core::CV_8UC4,
        _ => return Err(CameraError::ConversionError(
            format!("Format de pixel non supporté pour la conversion OpenCV: {:?}", frame.pixel_format)
        )),
    };
    
    let mut mat = unsafe {
        let size = core::Size::new(frame.width as i32, frame.height as i32);
        let mut mat = Mat::new_size(size, mat_type)?;
        let data_ptr = mat.data_mut() as *mut u8;
        std::ptr::copy_nonoverlapping(frame.data.as_ptr(), data_ptr, frame.data.len());
        mat
    };
    
    // Conversion si nécessaire
    let result = match frame.pixel_format {
        PixelFormat::BayerRG8 => {
            let mut rgb = Mat::default();
            imgproc::cvt_color(&mat, &mut rgb, imgproc::COLOR_BayerRG2RGB, 0)?;
            rgb
        },
        PixelFormat::BayerGB8 => {
            let mut rgb = Mat::default();
            imgproc::cvt_color(&mat, &mut rgb, imgproc::COLOR_BayerGB2RGB, 0)?;
            rgb
        },
        PixelFormat::BayerGR8 => {
            let mut rgb = Mat::default();
            imgproc::cvt_color(&mat, &mut rgb, imgproc::COLOR_BayerGR2RGB, 0)?;
            rgb
        },
        PixelFormat::BayerBG8 => {
            let mut rgb = Mat::default();
            imgproc::cvt_color(&mat, &mut rgb, imgproc::COLOR_BayerBG2RGB, 0)?;
            rgb
        },
        PixelFormat::YUV422 | PixelFormat::YUV422Packed => {
            let mut rgb = Mat::default();
            imgproc::cvt_color(&mat, &mut rgb, imgproc::COLOR_YUV2RGB_YUYV, 0)?;
            rgb
        },
        _ => mat,
    };
    
    Ok(result)
}

/// Convertit une image de caméra en image ndarray
pub fn to_ndarray(frame: &CameraFrame) -> Result<ndarray::Array3<u8>, CameraError> {
    let channels = match frame.pixel_format {
        PixelFormat::Mono8 => 1,
        PixelFormat::RGB8 | PixelFormat::BGR8 => 3,
        PixelFormat::RGBA8 | PixelFormat::BGRA8 => 4,
        _ => return Err(CameraError::ConversionError(
            format!("Format de pixel non supporté pour la conversion ndarray: {:?}", frame.pixel_format)
        )),
    };
    
    let shape = (frame.height as usize, frame.width as usize, channels);
    let data = frame.data.clone();
    
    // Créer un tableau ndarray à partir des données
    let array = ndarray::Array3::from_shape_vec(shape, data)
        .map_err(|e| CameraError::ConversionError(format!("Erreur de conversion en ndarray: {}", e)))?;
    
    Ok(array)
}
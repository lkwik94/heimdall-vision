//! Structures de données pour représenter les images et métadonnées
//!
//! Ce module définit les structures pour représenter les images acquises
//! par les caméras GigE Vision, ainsi que leurs métadonnées associées.

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::path::Path;
use std::time::SystemTime;

use image::{ImageBuffer, Luma};
use ndarray::{Array2, Array3};
use serde::{Deserialize, Serialize};

use crate::error::GigEError;

/// Image acquise par une caméra
#[derive(Debug, Clone)]
pub struct Frame {
    /// Données brutes de l'image
    pub data: Vec<u8>,
    
    /// Largeur de l'image
    pub width: u32,
    
    /// Hauteur de l'image
    pub height: u32,
    
    /// Format de pixel
    pub pixel_format: heimdall_camera::PixelFormat,
    
    /// Métadonnées de l'image
    pub metadata: FrameMetadata,
}

/// Métadonnées associées à une image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameMetadata {
    /// Horodatage de l'acquisition
    pub timestamp: SystemTime,
    
    /// Numéro de trame
    pub frame_id: u64,
    
    /// Identifiant de la caméra
    pub camera_id: String,
    
    /// Temps d'exposition utilisé (en microsecondes)
    pub exposure_time_us: u64,
    
    /// Gain utilisé (en dB)
    pub gain_db: f64,
    
    /// Température du capteur (en degrés Celsius)
    pub sensor_temperature: Option<f32>,
    
    /// Métadonnées supplémentaires
    pub extra: HashMap<String, String>,
}

impl Default for FrameMetadata {
    fn default() -> Self {
        Self {
            timestamp: SystemTime::now(),
            frame_id: 0,
            camera_id: String::new(),
            exposure_time_us: 0,
            gain_db: 0.0,
            sensor_temperature: None,
            extra: HashMap::new(),
        }
    }
}

impl FrameMetadata {
    /// Crée de nouvelles métadonnées avec un ID de caméra
    pub fn new(camera_id: &str) -> Self {
        Self {
            camera_id: camera_id.to_string(),
            ..Default::default()
        }
    }
    
    /// Ajoute une métadonnée supplémentaire
    pub fn add_extra(&mut self, key: &str, value: &str) {
        self.extra.insert(key.to_string(), value.to_string());
    }
    
    /// Obtient une métadonnée supplémentaire
    pub fn get_extra(&self, key: &str) -> Option<&String> {
        self.extra.get(key)
    }
    
    /// Calcule le temps écoulé depuis l'acquisition
    pub fn elapsed(&self) -> std::time::Duration {
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
    }
}

impl fmt::Display for FrameMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Caméra: {}", self.camera_id)?;
        writeln!(f, "Trame: {}", self.frame_id)?;
        writeln!(f, "Exposition: {} µs", self.exposure_time_us)?;
        writeln!(f, "Gain: {:.2} dB", self.gain_db)?;
        
        if let Some(temp) = self.sensor_temperature {
            writeln!(f, "Température: {:.1} °C", temp)?;
        }
        
        if !self.extra.is_empty() {
            writeln!(f, "Métadonnées supplémentaires:")?;
            for (key, value) in &self.extra {
                writeln!(f, "  {}: {}", key, value)?;
            }
        }
        
        Ok(())
    }
}

/// Ensemble d'images acquises simultanément par plusieurs caméras
#[derive(Debug, Clone)]
pub struct FrameSet {
    /// Images par caméra (clé = ID de caméra)
    pub frames: HashMap<String, Frame>,
    
    /// Horodatage de l'acquisition
    pub timestamp: SystemTime,
    
    /// Numéro de trame global
    pub frame_id: u64,
}

impl FrameSet {
    /// Crée un nouvel ensemble d'images vide
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
            timestamp: SystemTime::now(),
            frame_id: 0,
        }
    }
    
    /// Ajoute une image à l'ensemble
    pub fn add_frame(&mut self, camera_id: &str, frame: Frame) {
        self.frames.insert(camera_id.to_string(), frame);
    }
    
    /// Obtient une image par ID de caméra
    pub fn get_frame(&self, camera_id: &str) -> Option<&Frame> {
        self.frames.get(camera_id)
    }
    
    /// Nombre d'images dans l'ensemble
    pub fn len(&self) -> usize {
        self.frames.len()
    }
    
    /// Vérifie si l'ensemble est vide
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
    
    /// Vérifie si toutes les caméras spécifiées sont présentes
    pub fn has_all_cameras(&self, camera_ids: &[&str]) -> bool {
        camera_ids.iter().all(|id| self.frames.contains_key(*id))
    }
    
    /// Calcule le temps écoulé depuis l'acquisition
    pub fn elapsed(&self) -> std::time::Duration {
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
    }
}

impl Default for FrameSet {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    /// Crée une nouvelle image
    pub fn new(
        data: Vec<u8>,
        width: u32,
        height: u32,
        pixel_format: heimdall_camera::PixelFormat,
        metadata: FrameMetadata,
    ) -> Self {
        Self {
            data,
            width,
            height,
            pixel_format,
            metadata,
        }
    }
    
    /// Convertit l'image en tableau ndarray 2D (pour images en niveaux de gris)
    pub fn to_ndarray2(&self) -> Result<Array2<u8>, GigEError> {
        if self.pixel_format != heimdall_camera::PixelFormat::Mono8 {
            return Err(GigEError::ConversionError(
                "Seul le format Mono8 est supporté pour la conversion en Array2".to_string(),
            ));
        }
        
        let shape = (self.height as usize, self.width as usize);
        let data = self.data.clone();
        
        Array2::from_shape_vec(shape, data)
            .map_err(|e| GigEError::ConversionError(format!("Erreur de conversion en ndarray: {}", e)))
    }
    
    /// Convertit l'image en tableau ndarray 3D (pour images en couleur)
    pub fn to_ndarray3(&self) -> Result<Array3<u8>, GigEError> {
        let channels = match self.pixel_format {
            heimdall_camera::PixelFormat::Mono8 => 1,
            heimdall_camera::PixelFormat::RGB8 | heimdall_camera::PixelFormat::BGR8 => 3,
            heimdall_camera::PixelFormat::RGBA8 | heimdall_camera::PixelFormat::BGRA8 => 4,
            _ => {
                return Err(GigEError::ConversionError(format!(
                    "Format de pixel non supporté pour la conversion ndarray: {:?}",
                    self.pixel_format
                )))
            }
        };
        
        let shape = (self.height as usize, self.width as usize, channels);
        let data = self.data.clone();
        
        Array3::from_shape_vec(shape, data)
            .map_err(|e| GigEError::ConversionError(format!("Erreur de conversion en ndarray: {}", e)))
    }
    
    /// Convertit l'image en ImageBuffer (crate image)
    pub fn to_image_buffer(&self) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>, GigEError> {
        if self.pixel_format != heimdall_camera::PixelFormat::Mono8 {
            return Err(GigEError::ConversionError(
                "Seul le format Mono8 est supporté pour la conversion en ImageBuffer".to_string(),
            ));
        }
        
        ImageBuffer::from_raw(self.width, self.height, self.data.clone())
            .ok_or_else(|| GigEError::ConversionError("Erreur de conversion en ImageBuffer".to_string()))
    }
    
    /// Enregistre l'image dans un fichier
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), GigEError> {
        match self.pixel_format {
            heimdall_camera::PixelFormat::Mono8 => {
                let img = self.to_image_buffer()?;
                img.save(path)
                    .map_err(|e| GigEError::IoError(io::Error::new(io::ErrorKind::Other, e)))
            }
            _ => {
                return Err(GigEError::ConversionError(format!(
                    "Format de pixel non supporté pour l'enregistrement: {:?}",
                    self.pixel_format
                )))
            }
        }
    }
    
    /// Calcule l'histogramme de l'image
    pub fn histogram(&self) -> Result<[u32; 256], GigEError> {
        if self.pixel_format != heimdall_camera::PixelFormat::Mono8 {
            return Err(GigEError::ConversionError(
                "Seul le format Mono8 est supporté pour le calcul d'histogramme".to_string(),
            ));
        }
        
        let mut hist = [0u32; 256];
        
        for &pixel in &self.data {
            hist[pixel as usize] += 1;
        }
        
        Ok(hist)
    }
    
    /// Calcule la valeur moyenne de l'image
    pub fn mean(&self) -> Result<f64, GigEError> {
        if self.pixel_format != heimdall_camera::PixelFormat::Mono8 {
            return Err(GigEError::ConversionError(
                "Seul le format Mono8 est supporté pour le calcul de moyenne".to_string(),
            ));
        }
        
        let sum: u64 = self.data.iter().map(|&p| p as u64).sum();
        let mean = sum as f64 / self.data.len() as f64;
        
        Ok(mean)
    }
    
    /// Calcule l'écart-type de l'image
    pub fn std_dev(&self) -> Result<f64, GigEError> {
        if self.pixel_format != heimdall_camera::PixelFormat::Mono8 {
            return Err(GigEError::ConversionError(
                "Seul le format Mono8 est supporté pour le calcul d'écart-type".to_string(),
            ));
        }
        
        let mean = self.mean()?;
        let variance: f64 = self.data.iter()
            .map(|&p| {
                let diff = p as f64 - mean;
                diff * diff
            })
            .sum::<f64>() / self.data.len() as f64;
        
        Ok(variance.sqrt())
    }
    
    /// Écrit les métadonnées dans un flux
    pub fn write_metadata<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writeln!(writer, "Image: {}x{} {:?}", self.width, self.height, self.pixel_format)?;
        write!(writer, "{}", self.metadata)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};
    
    fn create_test_frame() -> Frame {
        let width = 10;
        let height = 10;
        let data = (0..100).map(|i| i as u8).collect();
        
        let mut metadata = FrameMetadata::new("test_camera");
        metadata.frame_id = 42;
        metadata.exposure_time_us = 10000;
        metadata.gain_db = 2.0;
        metadata.timestamp = SystemTime::now() - Duration::from_secs(1);
        
        Frame::new(
            data,
            width,
            height,
            heimdall_camera::PixelFormat::Mono8,
            metadata,
        )
    }
    
    #[test]
    fn test_frame_creation() {
        let frame = create_test_frame();
        
        assert_eq!(frame.width, 10);
        assert_eq!(frame.height, 10);
        assert_eq!(frame.data.len(), 100);
        assert_eq!(frame.metadata.camera_id, "test_camera");
        assert_eq!(frame.metadata.frame_id, 42);
    }
    
    #[test]
    fn test_frame_to_ndarray2() {
        let frame = create_test_frame();
        let array = frame.to_ndarray2().unwrap();
        
        assert_eq!(array.shape(), &[10, 10]);
        assert_eq!(array[[0, 0]], 0);
        assert_eq!(array[[9, 9]], 99);
    }
    
    #[test]
    fn test_frame_statistics() {
        let frame = create_test_frame();
        
        let mean = frame.mean().unwrap();
        assert!((mean - 49.5).abs() < 0.001);
        
        let std_dev = frame.std_dev().unwrap();
        assert!(std_dev > 0.0);
        
        let hist = frame.histogram().unwrap();
        for i in 0..100 {
            assert_eq!(hist[i], 1);
        }
    }
    
    #[test]
    fn test_frameset_operations() {
        let frame1 = create_test_frame();
        let frame2 = create_test_frame();
        
        let mut frameset = FrameSet::new();
        frameset.add_frame("camera1", frame1);
        frameset.add_frame("camera2", frame2);
        
        assert_eq!(frameset.len(), 2);
        assert!(frameset.has_all_cameras(&["camera1", "camera2"]));
        assert!(!frameset.has_all_cameras(&["camera1", "camera2", "camera3"]));
        
        let retrieved_frame = frameset.get_frame("camera1").unwrap();
        assert_eq!(retrieved_frame.width, 10);
        assert_eq!(retrieved_frame.height, 10);
    }
}
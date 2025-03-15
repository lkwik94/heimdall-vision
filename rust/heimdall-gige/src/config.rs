//! Configuration du système GigE Vision
//!
//! Ce module définit les structures de configuration pour le système GigE Vision
//! et les caméras individuelles.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration du système GigE Vision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Fréquence d'acquisition (images par seconde)
    pub frame_rate: f64,
    
    /// Temps d'exposition en microsecondes
    pub exposure_time_us: u64,
    
    /// Gain en dB
    pub gain_db: f64,
    
    /// Activer la région d'intérêt
    pub roi_enabled: bool,
    
    /// Position X de la région d'intérêt
    pub roi_x: u32,
    
    /// Position Y de la région d'intérêt
    pub roi_y: u32,
    
    /// Largeur de la région d'intérêt
    pub roi_width: u32,
    
    /// Hauteur de la région d'intérêt
    pub roi_height: u32,
    
    /// Taille des paquets réseau (en octets)
    pub packet_size: u32,
    
    /// Délai entre les paquets (en microsecondes)
    pub packet_delay: u32,
    
    /// Nombre de tampons d'image
    pub buffer_count: u32,
    
    /// Délai maximal d'acquisition (en millisecondes)
    pub acquisition_timeout_ms: u64,
    
    /// Nombre de tentatives de reprise en cas d'erreur
    pub retry_count: u32,
    
    /// Délai entre les tentatives (en millisecondes)
    pub retry_delay_ms: u64,
    
    /// Chemin du fichier de log
    pub log_file: Option<PathBuf>,
    
    /// Niveau de log
    pub log_level: LogLevel,
    
    /// Activer les métriques
    pub metrics_enabled: bool,
    
    /// Port pour l'exportation des métriques
    pub metrics_port: u16,
    
    /// Paramètres spécifiques au fabricant
    pub vendor_params: HashMap<String, String>,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            frame_rate: 30.0,
            exposure_time_us: 10000,
            gain_db: 0.0,
            roi_enabled: false,
            roi_x: 0,
            roi_y: 0,
            roi_width: 1920,
            roi_height: 1080,
            packet_size: 1500,
            packet_delay: 0,
            buffer_count: 10,
            acquisition_timeout_ms: 1000,
            retry_count: 3,
            retry_delay_ms: 100,
            log_file: None,
            log_level: LogLevel::Info,
            metrics_enabled: true,
            metrics_port: 9090,
            vendor_params: HashMap::new(),
        }
    }
}

impl SystemConfig {
    /// Charge la configuration à partir d'un fichier
    pub fn from_file(path: &str) -> Result<Self, config::ConfigError> {
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name(path))?;
        settings.try_into()
    }
    
    /// Enregistre la configuration dans un fichier
    pub fn save_to_file(&self, path: &str) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }
    
    /// Fusionne avec une autre configuration
    pub fn merge(&mut self, other: SystemConfig) {
        self.frame_rate = other.frame_rate;
        self.exposure_time_us = other.exposure_time_us;
        self.gain_db = other.gain_db;
        self.roi_enabled = other.roi_enabled;
        self.roi_x = other.roi_x;
        self.roi_y = other.roi_y;
        self.roi_width = other.roi_width;
        self.roi_height = other.roi_height;
        self.packet_size = other.packet_size;
        self.packet_delay = other.packet_delay;
        self.buffer_count = other.buffer_count;
        self.acquisition_timeout_ms = other.acquisition_timeout_ms;
        self.retry_count = other.retry_count;
        self.retry_delay_ms = other.retry_delay_ms;
        self.log_file = other.log_file;
        self.log_level = other.log_level;
        self.metrics_enabled = other.metrics_enabled;
        self.metrics_port = other.metrics_port;
        
        // Fusionner les paramètres spécifiques au fabricant
        for (key, value) in other.vendor_params {
            self.vendor_params.insert(key, value);
        }
    }
}

/// Configuration d'une caméra GigE Vision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    /// Format de pixel
    pub pixel_format: heimdall_camera::PixelFormat,
    
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
    
    /// Mode de déclenchement
    pub trigger_mode: heimdall_camera::TriggerMode,
    
    /// Activer la région d'intérêt
    pub roi_enabled: bool,
    
    /// Position X de la région d'intérêt
    pub roi_x: u32,
    
    /// Position Y de la région d'intérêt
    pub roi_y: u32,
    
    /// Largeur de la région d'intérêt
    pub roi_width: u32,
    
    /// Hauteur de la région d'intérêt
    pub roi_height: u32,
    
    /// Taille des paquets réseau (en octets)
    pub packet_size: u32,
    
    /// Délai entre les paquets (en microsecondes)
    pub packet_delay: u32,
    
    /// Nombre de tampons d'image
    pub buffer_count: u32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            pixel_format: heimdall_camera::PixelFormat::Mono8,
            width: 1920,
            height: 1080,
            frame_rate: 30.0,
            exposure_time_us: 10000,
            gain_db: 0.0,
            trigger_mode: heimdall_camera::TriggerMode::Continuous,
            roi_enabled: false,
            roi_x: 0,
            roi_y: 0,
            roi_width: 1920,
            roi_height: 1080,
            packet_size: 1500,
            packet_delay: 0,
            buffer_count: 10,
        }
    }
}

/// Niveau de log
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    /// Erreurs seulement
    Error,
    
    /// Avertissements et erreurs
    Warn,
    
    /// Informations, avertissements et erreurs
    Info,
    
    /// Débogage, informations, avertissements et erreurs
    Debug,
    
    /// Traces, débogage, informations, avertissements et erreurs
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_default_config() {
        let config = SystemConfig::default();
        assert_eq!(config.frame_rate, 30.0);
        assert_eq!(config.exposure_time_us, 10000);
        assert_eq!(config.gain_db, 0.0);
        assert_eq!(config.roi_enabled, false);
    }
    
    #[test]
    fn test_save_and_load_config() {
        let config = SystemConfig {
            frame_rate: 60.0,
            exposure_time_us: 5000,
            gain_db: 2.0,
            ..Default::default()
        };
        
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        
        config.save_to_file(path).unwrap();
        
        let loaded_config = SystemConfig::from_file(path).unwrap();
        
        assert_eq!(loaded_config.frame_rate, 60.0);
        assert_eq!(loaded_config.exposure_time_us, 5000);
        assert_eq!(loaded_config.gain_db, 2.0);
    }
    
    #[test]
    fn test_merge_configs() {
        let mut config1 = SystemConfig::default();
        let config2 = SystemConfig {
            frame_rate: 60.0,
            exposure_time_us: 5000,
            gain_db: 2.0,
            ..Default::default()
        };
        
        config1.merge(config2);
        
        assert_eq!(config1.frame_rate, 60.0);
        assert_eq!(config1.exposure_time_us, 5000);
        assert_eq!(config1.gain_db, 2.0);
    }
}
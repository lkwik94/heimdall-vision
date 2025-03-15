//! Mécanismes de synchronisation pour les caméras GigE Vision
//!
//! Ce module fournit les mécanismes de synchronisation hardware et software
//! pour garantir la capture d'images au bon moment.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};

use crate::error::GigEError;

/// Mode de synchronisation des caméras
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMode {
    /// Mode continu (freerun)
    Freerun,
    
    /// Déclenchement logiciel
    Software,
    
    /// Déclenchement matériel
    Hardware,
}

impl fmt::Display for SyncMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncMode::Freerun => write!(f, "Continu"),
            SyncMode::Software => write!(f, "Logiciel"),
            SyncMode::Hardware => write!(f, "Matériel"),
        }
    }
}

/// Source de déclenchement matériel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerSource {
    /// Ligne d'entrée 1
    Line1,
    
    /// Ligne d'entrée 2
    Line2,
    
    /// Ligne d'entrée 3
    Line3,
    
    /// Ligne d'entrée 4
    Line4,
    
    /// Encodeur
    Encoder,
    
    /// Horloge interne
    Timer,
}

impl fmt::Display for TriggerSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriggerSource::Line1 => write!(f, "Ligne 1"),
            TriggerSource::Line2 => write!(f, "Ligne 2"),
            TriggerSource::Line3 => write!(f, "Ligne 3"),
            TriggerSource::Line4 => write!(f, "Ligne 4"),
            TriggerSource::Encoder => write!(f, "Encodeur"),
            TriggerSource::Timer => write!(f, "Horloge"),
        }
    }
}

/// Configuration de synchronisation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Mode de synchronisation
    pub mode: SyncMode,
    
    /// Source de déclenchement (pour le mode Hardware)
    pub trigger_source: TriggerSource,
    
    /// Délai de déclenchement (en microsecondes)
    pub trigger_delay_us: u64,
    
    /// Délai entre les déclenchements (en microsecondes)
    pub trigger_interval_us: u64,
    
    /// Activer la synchronisation des expositions
    pub sync_exposures: bool,
    
    /// Activer la synchronisation des gains
    pub sync_gains: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            mode: SyncMode::Software,
            trigger_source: TriggerSource::Line1,
            trigger_delay_us: 0,
            trigger_interval_us: 33333, // ~30 Hz
            sync_exposures: true,
            sync_gains: true,
        }
    }
}

/// État de synchronisation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Mode de synchronisation actuel
    pub mode: SyncMode,
    
    /// Nombre de déclenchements effectués
    pub trigger_count: u64,
    
    /// Horodatage du dernier déclenchement
    pub last_trigger_time: Option<std::time::SystemTime>,
    
    /// Intervalle moyen entre les déclenchements (en microsecondes)
    pub average_interval_us: Option<u64>,
    
    /// Jitter de synchronisation (en microsecondes)
    pub sync_jitter_us: Option<u64>,
    
    /// État actif
    pub is_active: bool,
}

/// Gestionnaire de synchronisation
pub struct SyncManager {
    /// Configuration de synchronisation
    config: SyncConfig,
    
    /// Mode de synchronisation
    mode: SyncMode,
    
    /// Nombre de caméras
    camera_count: usize,
    
    /// État actif
    is_active: AtomicBool,
    
    /// Compteur de déclenchements
    trigger_count: AtomicU64,
    
    /// Horodatage du dernier déclenchement
    last_trigger_time: Option<Instant>,
    
    /// Historique des intervalles (pour calculer la moyenne et le jitter)
    trigger_intervals: Vec<Duration>,
}

impl SyncManager {
    /// Crée un nouveau gestionnaire de synchronisation
    pub fn new() -> Self {
        Self {
            config: SyncConfig::default(),
            mode: SyncMode::Software,
            camera_count: 0,
            is_active: AtomicBool::new(false),
            trigger_count: AtomicU64::new(0),
            last_trigger_time: None,
            trigger_intervals: Vec::with_capacity(100),
        }
    }
    
    /// Crée un nouveau gestionnaire avec une configuration spécifique
    pub fn with_config(config: SyncConfig) -> Self {
        Self {
            config,
            mode: config.mode,
            camera_count: 0,
            is_active: AtomicBool::new(false),
            trigger_count: AtomicU64::new(0),
            last_trigger_time: None,
            trigger_intervals: Vec::with_capacity(100),
        }
    }
    
    /// Définit le mode de synchronisation
    pub fn set_mode(&mut self, mode: SyncMode) {
        self.mode = mode;
        self.config.mode = mode;
    }
    
    /// Obtient le mode de synchronisation actuel
    pub fn get_mode(&self) -> SyncMode {
        self.mode
    }
    
    /// Définit le nombre de caméras
    pub fn set_camera_count(&mut self, count: usize) {
        self.camera_count = count;
    }
    
    /// Démarre la synchronisation
    pub fn start(&mut self) -> Result<(), GigEError> {
        if self.is_active.load(Ordering::SeqCst) {
            warn!("Le gestionnaire de synchronisation est déjà actif");
            return Ok(());
        }
        
        info!("Démarrage de la synchronisation en mode {}", self.mode);
        
        self.is_active.store(true, Ordering::SeqCst);
        self.trigger_count.store(0, Ordering::SeqCst);
        self.last_trigger_time = None;
        self.trigger_intervals.clear();
        
        // En mode Hardware, configurer le matériel
        if self.mode == SyncMode::Hardware {
            self.configure_hardware()?;
        }
        
        Ok(())
    }
    
    /// Arrête la synchronisation
    pub fn stop(&mut self) -> Result<(), GigEError> {
        if !self.is_active.load(Ordering::SeqCst) {
            warn!("Le gestionnaire de synchronisation n'est pas actif");
            return Ok(());
        }
        
        info!("Arrêt de la synchronisation");
        
        self.is_active.store(false, Ordering::SeqCst);
        
        // En mode Hardware, arrêter le matériel
        if self.mode == SyncMode::Hardware {
            self.stop_hardware()?;
        }
        
        Ok(())
    }
    
    /// Déclenche l'acquisition d'images
    pub fn trigger(&mut self) -> Result<(), GigEError> {
        if !self.is_active.load(Ordering::SeqCst) {
            return Err(GigEError::SyncError("Le gestionnaire de synchronisation n'est pas actif".to_string()));
        }
        
        if self.mode == SyncMode::Freerun {
            return Err(GigEError::SyncError("Impossible de déclencher en mode continu".to_string()));
        }
        
        let now = Instant::now();
        
        // Calculer l'intervalle depuis le dernier déclenchement
        if let Some(last_time) = self.last_trigger_time {
            let interval = now.duration_since(last_time);
            
            // Limiter la taille de l'historique
            if self.trigger_intervals.len() >= 100 {
                self.trigger_intervals.remove(0);
            }
            
            self.trigger_intervals.push(interval);
        }
        
        self.last_trigger_time = Some(now);
        self.trigger_count.fetch_add(1, Ordering::SeqCst);
        
        debug!("Déclenchement #{}", self.trigger_count.load(Ordering::SeqCst));
        
        // En mode Hardware, envoyer le signal de déclenchement
        if self.mode == SyncMode::Hardware {
            self.trigger_hardware()?;
        }
        
        Ok(())
    }
    
    /// Configure le matériel de synchronisation
    fn configure_hardware(&self) -> Result<(), GigEError> {
        // Cette fonction simule la configuration du matériel de synchronisation
        // En production, elle interagirait avec le matériel réel
        
        info!("Configuration du matériel de synchronisation");
        info!("Source de déclenchement: {}", self.config.trigger_source);
        info!("Délai de déclenchement: {} µs", self.config.trigger_delay_us);
        info!("Intervalle de déclenchement: {} µs", self.config.trigger_interval_us);
        
        // Simuler un délai de configuration
        std::thread::sleep(Duration::from_millis(50));
        
        Ok(())
    }
    
    /// Arrête le matériel de synchronisation
    fn stop_hardware(&self) -> Result<(), GigEError> {
        // Cette fonction simule l'arrêt du matériel de synchronisation
        // En production, elle interagirait avec le matériel réel
        
        info!("Arrêt du matériel de synchronisation");
        
        // Simuler un délai d'arrêt
        std::thread::sleep(Duration::from_millis(20));
        
        Ok(())
    }
    
    /// Envoie un signal de déclenchement matériel
    fn trigger_hardware(&self) -> Result<(), GigEError> {
        // Cette fonction simule l'envoi d'un signal de déclenchement matériel
        // En production, elle interagirait avec le matériel réel
        
        debug!("Envoi d'un signal de déclenchement matériel");
        
        // Simuler un délai de déclenchement
        if self.config.trigger_delay_us > 0 {
            std::thread::sleep(Duration::from_micros(self.config.trigger_delay_us));
        }
        
        Ok(())
    }
    
    /// Obtient l'état de synchronisation
    pub fn get_status(&self) -> SyncStatus {
        let trigger_count = self.trigger_count.load(Ordering::SeqCst);
        
        // Calculer l'intervalle moyen
        let average_interval_us = if !self.trigger_intervals.is_empty() {
            let sum: Duration = self.trigger_intervals.iter().sum();
            Some((sum.as_micros() as u64) / self.trigger_intervals.len() as u64)
        } else {
            None
        };
        
        // Calculer le jitter
        let sync_jitter_us = if self.trigger_intervals.len() > 1 {
            let avg = average_interval_us.unwrap_or(0) as f64;
            let variance = self.trigger_intervals.iter()
                .map(|&d| {
                    let diff = d.as_micros() as f64 - avg;
                    diff * diff
                })
                .sum::<f64>() / self.trigger_intervals.len() as f64;
            
            Some(variance.sqrt() as u64)
        } else {
            None
        };
        
        // Convertir l'horodatage du dernier déclenchement
        let last_trigger_time = self.last_trigger_time.map(|t| {
            let elapsed = t.elapsed();
            std::time::SystemTime::now().checked_sub(elapsed).unwrap_or_else(std::time::SystemTime::now)
        });
        
        SyncStatus {
            mode: self.mode,
            trigger_count,
            last_trigger_time,
            average_interval_us,
            sync_jitter_us,
            is_active: self.is_active.load(Ordering::SeqCst),
        }
    }
    
    /// Obtient la configuration de synchronisation
    pub fn get_config(&self) -> &SyncConfig {
        &self.config
    }
    
    /// Définit la configuration de synchronisation
    pub fn set_config(&mut self, config: SyncConfig) {
        self.config = config;
        self.mode = config.mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_sync_manager_creation() {
        let manager = SyncManager::new();
        assert_eq!(manager.get_mode(), SyncMode::Software);
        assert!(!manager.is_active.load(Ordering::SeqCst));
    }
    
    #[test]
    fn test_sync_manager_with_config() {
        let config = SyncConfig {
            mode: SyncMode::Hardware,
            trigger_source: TriggerSource::Line2,
            trigger_delay_us: 100,
            trigger_interval_us: 10000,
            sync_exposures: true,
            sync_gains: false,
        };
        
        let manager = SyncManager::with_config(config);
        assert_eq!(manager.get_mode(), SyncMode::Hardware);
        assert_eq!(manager.config.trigger_source, TriggerSource::Line2);
        assert_eq!(manager.config.trigger_delay_us, 100);
        assert_eq!(manager.config.trigger_interval_us, 10000);
    }
    
    #[test]
    fn test_sync_manager_start_stop() {
        let mut manager = SyncManager::new();
        
        // Démarrer
        let result = manager.start();
        assert!(result.is_ok());
        assert!(manager.is_active.load(Ordering::SeqCst));
        
        // Arrêter
        let result = manager.stop();
        assert!(result.is_ok());
        assert!(!manager.is_active.load(Ordering::SeqCst));
    }
    
    #[test]
    fn test_sync_manager_trigger() {
        let mut manager = SyncManager::new();
        
        // Démarrer
        manager.start().unwrap();
        
        // Déclencher plusieurs fois
        for _ in 0..5 {
            let result = manager.trigger();
            assert!(result.is_ok());
            thread::sleep(Duration::from_millis(10));
        }
        
        // Vérifier le compteur
        assert_eq!(manager.trigger_count.load(Ordering::SeqCst), 5);
        
        // Vérifier l'état
        let status = manager.get_status();
        assert_eq!(status.trigger_count, 5);
        assert!(status.average_interval_us.is_some());
        
        // Arrêter
        manager.stop().unwrap();
    }
    
    #[test]
    fn test_sync_manager_freerun_mode() {
        let mut manager = SyncManager::new();
        manager.set_mode(SyncMode::Freerun);
        
        // Démarrer
        manager.start().unwrap();
        
        // Essayer de déclencher en mode continu (devrait échouer)
        let result = manager.trigger();
        assert!(result.is_err());
        
        // Arrêter
        manager.stop().unwrap();
    }
}
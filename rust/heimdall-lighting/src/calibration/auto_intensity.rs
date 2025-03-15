use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use log::{debug, error, info, warn};
use tokio::time;
use ndarray::{Array2, Array3, ArrayView3, Axis};

use crate::{
    LightingController, LightingConfig, LightChannelConfig, LightChannelState,
    LightingError, SyncMode, LightingType, AutoIntensityAdjuster
};

/// Algorithme d'ajustement d'intensité
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntensityAlgorithm {
    /// Algorithme PID
    PID,
    
    /// Algorithme par recherche binaire
    BinarySearch,
    
    /// Algorithme par gradient
    Gradient,
    
    /// Algorithme par histogramme
    Histogram,
}

/// Configuration d'ajustement automatique d'intensité
#[derive(Debug, Clone)]
pub struct AutoIntensityConfig {
    /// Algorithme d'ajustement
    pub algorithm: IntensityAlgorithm,
    
    /// Intensité cible (valeur moyenne de l'image)
    pub target_intensity: f64,
    
    /// Tolérance d'intensité
    pub tolerance: f64,
    
    /// Pas d'ajustement
    pub adjustment_step: f64,
    
    /// Intensité minimale
    pub min_intensity: f64,
    
    /// Intensité maximale
    pub max_intensity: f64,
    
    /// Région d'intérêt (x, y, largeur, hauteur)
    pub roi: Option<(usize, usize, usize, usize)>,
    
    /// Paramètres PID (Kp, Ki, Kd)
    pub pid_params: Option<(f64, f64, f64)>,
}

impl Default for AutoIntensityConfig {
    fn default() -> Self {
        Self {
            algorithm: IntensityAlgorithm::PID,
            target_intensity: 128.0,  // Pour une image 8 bits
            tolerance: 5.0,
            adjustment_step: 2.0,
            min_intensity: 0.0,
            max_intensity: 100.0,
            roi: None,
            pid_params: Some((0.5, 0.1, 0.05)),
        }
    }
}

/// Contrôleur PID pour l'ajustement d'intensité
pub struct PIDController {
    /// Coefficient proportionnel
    kp: f64,
    
    /// Coefficient intégral
    ki: f64,
    
    /// Coefficient dérivé
    kd: f64,
    
    /// Valeur cible
    setpoint: f64,
    
    /// Erreur précédente
    previous_error: f64,
    
    /// Erreur intégrale
    integral: f64,
    
    /// Horodatage de la dernière mise à jour
    last_update: Instant,
}

impl PIDController {
    /// Crée un nouveau contrôleur PID
    pub fn new(kp: f64, ki: f64, kd: f64, setpoint: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            setpoint,
            previous_error: 0.0,
            integral: 0.0,
            last_update: Instant::now(),
        }
    }
    
    /// Calcule la sortie du contrôleur
    pub fn compute(&mut self, input: f64) -> f64 {
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;
        
        // Limiter dt pour éviter les divisions par zéro ou les valeurs trop grandes
        let dt = dt.max(0.001).min(1.0);
        
        // Calculer l'erreur
        let error = self.setpoint - input;
        
        // Calculer l'intégrale
        self.integral += error * dt;
        
        // Calculer la dérivée
        let derivative = if dt > 0.0 {
            (error - self.previous_error) / dt
        } else {
            0.0
        };
        
        // Mettre à jour l'erreur précédente
        self.previous_error = error;
        
        // Calculer la sortie
        let output = self.kp * error + self.ki * self.integral + self.kd * derivative;
        
        output
    }
    
    /// Réinitialise le contrôleur
    pub fn reset(&mut self) {
        self.previous_error = 0.0;
        self.integral = 0.0;
        self.last_update = Instant::now();
    }
    
    /// Définit la valeur cible
    pub fn set_setpoint(&mut self, setpoint: f64) {
        self.setpoint = setpoint;
        self.reset();
    }
}

/// Gestionnaire d'ajustement automatique d'intensité avancé
pub struct AdvancedAutoIntensityAdjuster {
    /// Contrôleur d'éclairage
    controller: Arc<Mutex<Box<dyn LightingController>>>,
    
    /// Canal d'éclairage à ajuster
    channel_id: String,
    
    /// Configuration d'ajustement
    config: AutoIntensityConfig,
    
    /// Contrôleur PID
    pid: Option<PIDController>,
    
    /// Historique des ajustements
    history: Vec<(f64, f64)>,  // (intensité, valeur mesurée)
}

impl AdvancedAutoIntensityAdjuster {
    /// Crée un nouveau gestionnaire d'ajustement automatique
    pub fn new(
        controller: Box<dyn LightingController>,
        channel_id: String,
        config: AutoIntensityConfig
    ) -> Self {
        let pid = if let Some((kp, ki, kd)) = config.pid_params {
            Some(PIDController::new(kp, ki, kd, config.target_intensity))
        } else {
            None
        };
        
        Self {
            controller: Arc::new(Mutex::new(controller)),
            channel_id,
            config,
            pid,
            history: Vec::new(),
        }
    }
    
    /// Ajuste l'intensité en fonction de l'image acquise
    pub async fn adjust(&mut self, image: &ArrayView3<u8>) -> Result<f64, LightingError> {
        // Calculer la valeur moyenne de l'image
        let image_mean = self.calculate_image_mean(image);
        
        // Obtenir l'état actuel du canal
        let mut controller = self.controller.lock().unwrap();
        let channel_state = controller.get_channel_state(&self.channel_id)
            .ok_or_else(|| LightingError::ConfigError(format!("Canal non trouvé: {}", self.channel_id)))?;
        
        let current_intensity = channel_state.current_intensity;
        
        // Calculer la nouvelle intensité selon l'algorithme
        let new_intensity = match self.config.algorithm {
            IntensityAlgorithm::PID => self.adjust_pid(image_mean, current_intensity),
            IntensityAlgorithm::BinarySearch => self.adjust_binary_search(image_mean, current_intensity),
            IntensityAlgorithm::Gradient => self.adjust_gradient(image_mean, current_intensity),
            IntensityAlgorithm::Histogram => self.adjust_histogram(image, current_intensity),
        };
        
        // Limiter l'intensité
        let new_intensity = new_intensity.max(self.config.min_intensity).min(self.config.max_intensity);
        
        // Appliquer la nouvelle intensité
        controller.set_intensity(&self.channel_id, new_intensity).await?;
        
        // Enregistrer l'ajustement dans l'historique
        self.history.push((new_intensity, image_mean));
        
        Ok(new_intensity)
    }
    
    /// Calcule la valeur moyenne de l'image
    fn calculate_image_mean(&self, image: &ArrayView3<u8>) -> f64 {
        if let Some((x, y, width, height)) = self.config.roi {
            // Extraire la région d'intérêt
            let roi = image.slice(s![y..y+height, x..x+width, ..]);
            
            // Calculer la moyenne
            let sum: u64 = roi.iter().map(|&x| x as u64).sum();
            let count = roi.len() as u64;
            
            if count > 0 {
                sum as f64 / count as f64
            } else {
                0.0
            }
        } else {
            // Utiliser toute l'image
            let sum: u64 = image.iter().map(|&x| x as u64).sum();
            let count = image.len() as u64;
            
            if count > 0 {
                sum as f64 / count as f64
            } else {
                0.0
            }
        }
    }
    
    /// Ajuste l'intensité avec l'algorithme PID
    fn adjust_pid(&mut self, image_mean: f64, current_intensity: f64) -> f64 {
        if let Some(pid) = &mut self.pid {
            // Calculer la sortie du PID
            let output = pid.compute(image_mean);
            
            // Appliquer la sortie à l'intensité actuelle
            current_intensity + output
        } else {
            // Fallback sur un ajustement simple
            if (image_mean - self.config.target_intensity).abs() <= self.config.tolerance {
                current_intensity
            } else if image_mean < self.config.target_intensity {
                current_intensity + self.config.adjustment_step
            } else {
                current_intensity - self.config.adjustment_step
            }
        }
    }
    
    /// Ajuste l'intensité avec l'algorithme de recherche binaire
    fn adjust_binary_search(&mut self, image_mean: f64, current_intensity: f64) -> f64 {
        // Si l'intensité est déjà dans la plage cible, ne rien faire
        if (image_mean - self.config.target_intensity).abs() <= self.config.tolerance {
            return current_intensity;
        }
        
        // Recherche binaire basée sur l'historique
        if self.history.len() >= 2 {
            // Trouver les deux points les plus proches de la cible
            let mut lower = (0.0, 0.0);
            let mut upper = (100.0, 255.0);
            
            for &(intensity, mean) in &self.history {
                if mean <= self.config.target_intensity && mean > lower.1 {
                    lower = (intensity, mean);
                } else if mean > self.config.target_intensity && mean < upper.1 {
                    upper = (intensity, mean);
                }
            }
            
            // Interpolation linéaire
            if lower.1 != upper.1 {
                let ratio = (self.config.target_intensity - lower.1) / (upper.1 - lower.1);
                lower.0 + ratio * (upper.0 - lower.0)
            } else {
                // Fallback sur un ajustement simple
                if image_mean < self.config.target_intensity {
                    current_intensity + self.config.adjustment_step
                } else {
                    current_intensity - self.config.adjustment_step
                }
            }
        } else {
            // Pas assez de données, utiliser un ajustement simple
            if image_mean < self.config.target_intensity {
                current_intensity + self.config.adjustment_step
            } else {
                current_intensity - self.config.adjustment_step
            }
        }
    }
    
    /// Ajuste l'intensité avec l'algorithme de gradient
    fn adjust_gradient(&mut self, image_mean: f64, current_intensity: f64) -> f64 {
        // Si l'intensité est déjà dans la plage cible, ne rien faire
        if (image_mean - self.config.target_intensity).abs() <= self.config.tolerance {
            return current_intensity;
        }
        
        // Calculer le gradient
        if self.history.len() >= 2 {
            let (prev_intensity, prev_mean) = self.history[self.history.len() - 1];
            
            // Calculer la dérivée
            let d_mean = image_mean - prev_mean;
            let d_intensity = current_intensity - prev_intensity;
            
            // Éviter la division par zéro
            if d_intensity.abs() > 1e-6 {
                let gradient = d_mean / d_intensity;
                
                // Calculer l'erreur
                let error = self.config.target_intensity - image_mean;
                
                // Calculer l'ajustement
                if gradient.abs() > 1e-6 {
                    let adjustment = error / gradient;
                    
                    // Limiter l'ajustement
                    let max_adjustment = self.config.adjustment_step * 2.0;
                    let adjustment = adjustment.max(-max_adjustment).min(max_adjustment);
                    
                    current_intensity + adjustment
                } else {
                    // Gradient trop faible, utiliser un ajustement simple
                    if image_mean < self.config.target_intensity {
                        current_intensity + self.config.adjustment_step
                    } else {
                        current_intensity - self.config.adjustment_step
                    }
                }
            } else {
                // Pas de changement d'intensité, utiliser un ajustement simple
                if image_mean < self.config.target_intensity {
                    current_intensity + self.config.adjustment_step
                } else {
                    current_intensity - self.config.adjustment_step
                }
            }
        } else {
            // Pas assez de données, utiliser un ajustement simple
            if image_mean < self.config.target_intensity {
                current_intensity + self.config.adjustment_step
            } else {
                current_intensity - self.config.adjustment_step
            }
        }
    }
    
    /// Ajuste l'intensité avec l'algorithme d'histogramme
    fn adjust_histogram(&mut self, image: &ArrayView3<u8>, current_intensity: f64) -> f64 {
        // Calculer l'histogramme
        let mut histogram = [0u32; 256];
        
        if let Some((x, y, width, height)) = self.config.roi {
            // Extraire la région d'intérêt
            let roi = image.slice(s![y..y+height, x..x+width, ..]);
            
            // Remplir l'histogramme
            for &pixel in roi.iter() {
                histogram[pixel as usize] += 1;
            }
        } else {
            // Utiliser toute l'image
            for &pixel in image.iter() {
                histogram[pixel as usize] += 1;
            }
        }
        
        // Calculer la médiane
        let total_pixels = histogram.iter().sum::<u32>();
        let mut cumulative = 0;
        let mut median = 0;
        
        for (value, &count) in histogram.iter().enumerate() {
            cumulative += count;
            if cumulative >= total_pixels / 2 {
                median = value;
                break;
            }
        }
        
        // Calculer l'ajustement
        let target_median = self.config.target_intensity;
        
        if (median as f64 - target_median).abs() <= self.config.tolerance {
            current_intensity
        } else {
            // Calculer le ratio d'ajustement
            let ratio = target_median / median as f64;
            
            // Appliquer le ratio à l'intensité actuelle
            let new_intensity = current_intensity * ratio;
            
            // Limiter l'ajustement
            let max_adjustment = self.config.adjustment_step * 2.0;
            let adjustment = (new_intensity - current_intensity).max(-max_adjustment).min(max_adjustment);
            
            current_intensity + adjustment
        }
    }
    
    /// Obtient l'historique des ajustements
    pub fn get_history(&self) -> &[(f64, f64)] {
        &self.history
    }
    
    /// Réinitialise l'historique
    pub fn reset_history(&mut self) {
        self.history.clear();
        if let Some(pid) = &mut self.pid {
            pid.reset();
        }
    }
}
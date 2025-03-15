use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use log::{debug, error, info, warn};
use tokio::time;
use chrono::{DateTime, Utc};
use ndarray::{Array2, Array3, ArrayView3, Axis};

use crate::{
    LightingController, LightingConfig, LightChannelConfig, LightChannelState,
    LightingError, SyncMode, LightingType, LightingDiagnostics, DiagnosticResult,
    DiagnosticStatus, ChannelDiagnostic
};

/// Configuration de surveillance
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Intervalle de surveillance (en secondes)
    pub interval_sec: u64,
    
    /// Seuil d'alerte pour la durée d'utilisation (en heures)
    pub usage_threshold_hours: f64,
    
    /// Seuil d'alerte pour l'intensité minimale (%)
    pub min_intensity_threshold: f64,
    
    /// Seuil d'alerte pour l'uniformité (%)
    pub uniformity_threshold: f64,
    
    /// Seuil d'alerte pour la variation d'intensité (%)
    pub intensity_variation_threshold: f64,
    
    /// Nombre de mesures à conserver dans l'historique
    pub history_size: usize,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            interval_sec: 3600,  // 1 heure
            usage_threshold_hours: 5000.0,  // 5000 heures
            min_intensity_threshold: 80.0,  // 80% de l'intensité initiale
            uniformity_threshold: 80.0,  // 80% d'uniformité
            intensity_variation_threshold: 5.0,  // 5% de variation
            history_size: 100,
        }
    }
}

/// Mesure de surveillance
#[derive(Debug, Clone)]
pub struct MonitoringMeasurement {
    /// Horodatage
    pub timestamp: DateTime<Utc>,
    
    /// Intensité moyenne (%)
    pub mean_intensity: f64,
    
    /// Uniformité (%)
    pub uniformity: f64,
    
    /// Température (°C)
    pub temperature: Option<f64>,
    
    /// Durée d'utilisation (heures)
    pub usage_hours: f64,
    
    /// Statut
    pub status: DiagnosticStatus,
}

/// Système de surveillance d'éclairage
pub struct LightingMonitor {
    /// Diagnostics d'éclairage
    diagnostics: LightingDiagnostics,
    
    /// Configuration de surveillance
    config: MonitoringConfig,
    
    /// Historique des mesures
    history: Vec<MonitoringMeasurement>,
    
    /// Tâche de surveillance en arrière-plan
    monitoring_task: Option<tokio::task::JoinHandle<()>>,
    
    /// Canal pour arrêter la surveillance
    stop_channel: (tokio::sync::mpsc::Sender<()>, tokio::sync::mpsc::Receiver<()>),
    
    /// Callbacks d'alerte
    alert_callbacks: Vec<Box<dyn Fn(&MonitoringMeasurement) + Send + Sync>>,
}

impl LightingMonitor {
    /// Crée un nouveau système de surveillance
    pub fn new(
        controller: Box<dyn LightingController>,
        config: MonitoringConfig
    ) -> Self {
        let diagnostics = LightingDiagnostics::new(
            controller,
            config.usage_threshold_hours,
            config.min_intensity_threshold
        );
        
        let stop_channel = tokio::sync::mpsc::channel(1);
        
        Self {
            diagnostics,
            config,
            history: Vec::new(),
            monitoring_task: None,
            stop_channel,
            alert_callbacks: Vec::new(),
        }
    }
    
    /// Démarre la surveillance
    pub fn start(&mut self) -> Result<(), LightingError> {
        if self.monitoring_task.is_some() {
            return Err(LightingError::ConfigError("Surveillance déjà démarrée".to_string()));
        }
        
        let config = self.config.clone();
        let diagnostics = Arc::new(Mutex::new(&self.diagnostics));
        let history = Arc::new(Mutex::new(&mut self.history));
        let alert_callbacks = Arc::new(Mutex::new(&self.alert_callbacks));
        let mut stop_receiver = self.stop_channel.1.clone();
        
        // Démarrer la tâche de surveillance en arrière-plan
        self.monitoring_task = Some(tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(config.interval_sec));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Exécuter le diagnostic
                        let diagnostic_result = {
                            let mut diagnostics = diagnostics.lock().unwrap();
                            match diagnostics.run_diagnostic().await {
                                Ok(result) => result,
                                Err(e) => {
                                    error!("Erreur lors du diagnostic: {}", e);
                                    continue;
                                }
                            }
                        };
                        
                        // Créer une mesure
                        let measurement = Self::create_measurement(&diagnostic_result, &config);
                        
                        // Ajouter la mesure à l'historique
                        {
                            let mut history = history.lock().unwrap();
                            history.push(measurement.clone());
                            
                            // Limiter la taille de l'historique
                            if history.len() > config.history_size {
                                history.remove(0);
                            }
                        }
                        
                        // Vérifier les alertes
                        if measurement.status != DiagnosticStatus::Ok {
                            let callbacks = alert_callbacks.lock().unwrap();
                            for callback in callbacks.iter() {
                                callback(&measurement);
                            }
                        }
                    }
                    _ = stop_receiver.recv() => {
                        info!("Arrêt de la surveillance");
                        break;
                    }
                }
            }
        }));
        
        Ok(())
    }
    
    /// Arrête la surveillance
    pub fn stop(&mut self) -> Result<(), LightingError> {
        if let Some(task) = self.monitoring_task.take() {
            // Envoyer un signal d'arrêt
            if let Err(e) = self.stop_channel.0.try_send(()) {
                error!("Erreur lors de l'envoi du signal d'arrêt: {}", e);
            }
            
            // Attendre la fin de la tâche
            tokio::spawn(async move {
                if let Err(e) = task.await {
                    error!("Erreur lors de l'arrêt de la tâche de surveillance: {}", e);
                }
            });
        }
        
        Ok(())
    }
    
    /// Crée une mesure à partir d'un résultat de diagnostic
    fn create_measurement(diagnostic_result: &DiagnosticResult, config: &MonitoringConfig) -> MonitoringMeasurement {
        // Calculer l'intensité moyenne
        let mut mean_intensity = 0.0;
        let mut total_usage = 0.0;
        let mut min_uniformity = 100.0;
        let mut channel_count = 0;
        
        for (_, channel) in &diagnostic_result.channel_results {
            mean_intensity += channel.max_intensity;
            total_usage += channel.usage_hours;
            min_uniformity = min_uniformity.min(channel.uniformity);
            channel_count += 1;
        }
        
        if channel_count > 0 {
            mean_intensity /= channel_count as f64;
            total_usage /= channel_count as f64;
        }
        
        // Créer la mesure
        MonitoringMeasurement {
            timestamp: diagnostic_result.timestamp,
            mean_intensity,
            uniformity: min_uniformity,
            temperature: None,  // Non disponible dans le diagnostic
            usage_hours: total_usage,
            status: diagnostic_result.status,
        }
    }
    
    /// Ajoute un callback d'alerte
    pub fn add_alert_callback<F>(&mut self, callback: F)
    where
        F: Fn(&MonitoringMeasurement) + Send + Sync + 'static
    {
        self.alert_callbacks.push(Box::new(callback));
    }
    
    /// Obtient l'historique des mesures
    pub fn get_history(&self) -> &[MonitoringMeasurement] {
        &self.history
    }
    
    /// Analyse les tendances
    pub fn analyze_trends(&self) -> HashMap<String, f64> {
        let mut trends = HashMap::new();
        
        if self.history.len() < 2 {
            return trends;
        }
        
        // Calculer la tendance d'intensité
        let intensity_values: Vec<f64> = self.history.iter()
            .map(|m| m.mean_intensity)
            .collect();
        
        let intensity_trend = Self::calculate_trend(&intensity_values);
        trends.insert("intensity".to_string(), intensity_trend);
        
        // Calculer la tendance d'uniformité
        let uniformity_values: Vec<f64> = self.history.iter()
            .map(|m| m.uniformity)
            .collect();
        
        let uniformity_trend = Self::calculate_trend(&uniformity_values);
        trends.insert("uniformity".to_string(), uniformity_trend);
        
        // Calculer la tendance de température
        if let Some(temp_values) = self.history.iter()
            .filter_map(|m| m.temperature)
            .collect::<Vec<f64>>()
            .into_iter()
            .next()
            .map(|_| self.history.iter().filter_map(|m| m.temperature).collect::<Vec<f64>>())
        {
            if !temp_values.is_empty() {
                let temp_trend = Self::calculate_trend(&temp_values);
                trends.insert("temperature".to_string(), temp_trend);
            }
        }
        
        trends
    }
    
    /// Calcule la tendance d'une série de valeurs
    fn calculate_trend(values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        
        // Régression linéaire simple
        let n = values.len() as f64;
        let x_mean = (n - 1.0) / 2.0;  // Moyenne des indices (0, 1, 2, ...)
        let y_mean = values.iter().sum::<f64>() / n;
        
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        
        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }
        
        if denominator.abs() < 1e-10 {
            0.0
        } else {
            numerator / denominator
        }
    }
    
    /// Prédit la durée de vie restante
    pub fn predict_remaining_life(&self) -> Option<f64> {
        if self.history.len() < 2 {
            return None;
        }
        
        // Calculer la tendance d'intensité
        let intensity_values: Vec<f64> = self.history.iter()
            .map(|m| m.mean_intensity)
            .collect();
        
        let intensity_trend = Self::calculate_trend(&intensity_values);
        
        // Si la tendance est positive ou nulle, la durée de vie est indéterminée
        if intensity_trend >= 0.0 {
            return None;
        }
        
        // Calculer la durée de vie restante
        let current_intensity = intensity_values.last().unwrap();
        let min_intensity = self.config.min_intensity_threshold;
        
        // Nombre de périodes avant d'atteindre l'intensité minimale
        let periods = (*current_intensity - min_intensity) / -intensity_trend;
        
        // Convertir en heures
        let hours = periods * self.config.interval_sec as f64 / 3600.0;
        
        Some(hours)
    }
    
    /// Détecte les anomalies
    pub fn detect_anomalies(&self) -> Vec<String> {
        let mut anomalies = Vec::new();
        
        if self.history.len() < 3 {
            return anomalies;
        }
        
        // Calculer les moyennes et écarts-types
        let intensity_values: Vec<f64> = self.history.iter()
            .map(|m| m.mean_intensity)
            .collect();
        
        let uniformity_values: Vec<f64> = self.history.iter()
            .map(|m| m.uniformity)
            .collect();
        
        let intensity_mean = intensity_values.iter().sum::<f64>() / intensity_values.len() as f64;
        let uniformity_mean = uniformity_values.iter().sum::<f64>() / uniformity_values.len() as f64;
        
        let intensity_std_dev = (intensity_values.iter()
            .map(|&x| (x - intensity_mean).powi(2))
            .sum::<f64>() / intensity_values.len() as f64)
            .sqrt();
            
        let uniformity_std_dev = (uniformity_values.iter()
            .map(|&x| (x - uniformity_mean).powi(2))
            .sum::<f64>() / uniformity_values.len() as f64)
            .sqrt();
        
        // Vérifier les anomalies d'intensité
        let last_intensity = intensity_values.last().unwrap();
        if (*last_intensity - intensity_mean).abs() > 3.0 * intensity_std_dev {
            anomalies.push(format!(
                "Anomalie d'intensité détectée: {:.1}% (moyenne: {:.1}%, écart-type: {:.1}%)",
                last_intensity, intensity_mean, intensity_std_dev
            ));
        }
        
        // Vérifier les anomalies d'uniformité
        let last_uniformity = uniformity_values.last().unwrap();
        if (*last_uniformity - uniformity_mean).abs() > 3.0 * uniformity_std_dev {
            anomalies.push(format!(
                "Anomalie d'uniformité détectée: {:.1}% (moyenne: {:.1}%, écart-type: {:.1}%)",
                last_uniformity, uniformity_mean, uniformity_std_dev
            ));
        }
        
        // Vérifier les variations brusques
        if self.history.len() >= 2 {
            let last = self.history.last().unwrap();
            let previous = &self.history[self.history.len() - 2];
            
            let intensity_change = (last.mean_intensity - previous.mean_intensity).abs();
            let uniformity_change = (last.uniformity - previous.uniformity).abs();
            
            if intensity_change > self.config.intensity_variation_threshold {
                anomalies.push(format!(
                    "Variation brusque d'intensité: {:.1}% (seuil: {:.1}%)",
                    intensity_change, self.config.intensity_variation_threshold
                ));
            }
            
            if uniformity_change > self.config.intensity_variation_threshold {
                anomalies.push(format!(
                    "Variation brusque d'uniformité: {:.1}% (seuil: {:.1}%)",
                    uniformity_change, self.config.intensity_variation_threshold
                ));
            }
        }
        
        anomalies
    }
}
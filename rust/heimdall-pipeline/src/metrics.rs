use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use log::{debug, error, info, warn};
use metrics::{Counter, Gauge, Histogram, Key, KeyName, Label, Recorder, Unit};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use crate::PipelineStats;

/// Fenêtre de temps pour les métriques glissantes (en secondes)
const METRICS_WINDOW_SECONDS: u64 = 60;

/// Intervalle de mise à jour des métriques (en millisecondes)
const METRICS_UPDATE_INTERVAL_MS: u64 = 100;

/// Métriques du pipeline d'acquisition
pub struct PipelineMetrics {
    /// Compteur d'images acquises
    frames_acquired: Counter,
    
    /// Compteur d'images traitées
    frames_processed: Counter,
    
    /// Compteur d'images perdues
    frames_dropped: Counter,
    
    /// Compteur de débordements de buffer
    buffer_overflows: Counter,
    
    /// Compteur de désynchronisations
    desync_events: Counter,
    
    /// Compteur de récupérations
    recovery_events: Counter,
    
    /// Jauge d'utilisation du buffer
    buffer_usage: Gauge,
    
    /// Histogramme de latence d'acquisition
    acquisition_latency: Histogram,
    
    /// Histogramme de latence de traitement
    processing_latency: Histogram,
    
    /// Jauge de taux d'acquisition
    acquisition_rate: Gauge,
    
    /// Jauge de taux de traitement
    processing_rate: Gauge,
    
    /// Historique des acquisitions pour le calcul du taux
    acquisition_history: Arc<Mutex<VecDeque<Instant>>>,
    
    /// Historique des traitements pour le calcul du taux
    processing_history: Arc<Mutex<VecDeque<Instant>>>,
    
    /// Horodatage de la dernière mise à jour des métriques
    last_update: Arc<Mutex<Instant>>,
    
    /// Handle Prometheus pour l'exposition des métriques
    prometheus_handle: Option<PrometheusHandle>,
}

impl PipelineMetrics {
    /// Crée une nouvelle instance de métriques
    pub fn new() -> Self {
        // Initialiser le recorder Prometheus
        let builder = PrometheusBuilder::new();
        let builder = builder
            .set_buckets_for_metric(
                Matcher::Full("heimdall_pipeline_acquisition_latency".to_string()),
                &[0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0],
            )
            .unwrap()
            .set_buckets_for_metric(
                Matcher::Full("heimdall_pipeline_processing_latency".to_string()),
                &[0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0],
            )
            .unwrap();
        
        let (recorder, prometheus_handle) = builder.build().unwrap();
        metrics::set_boxed_recorder(Box::new(recorder)).unwrap();
        
        Self {
            frames_acquired: metrics::counter!("heimdall_pipeline_frames_acquired"),
            frames_processed: metrics::counter!("heimdall_pipeline_frames_processed"),
            frames_dropped: metrics::counter!("heimdall_pipeline_frames_dropped"),
            buffer_overflows: metrics::counter!("heimdall_pipeline_buffer_overflows"),
            desync_events: metrics::counter!("heimdall_pipeline_desync_events"),
            recovery_events: metrics::counter!("heimdall_pipeline_recovery_events"),
            buffer_usage: metrics::gauge!("heimdall_pipeline_buffer_usage"),
            acquisition_latency: metrics::histogram!("heimdall_pipeline_acquisition_latency"),
            processing_latency: metrics::histogram!("heimdall_pipeline_processing_latency"),
            acquisition_rate: metrics::gauge!("heimdall_pipeline_acquisition_rate"),
            processing_rate: metrics::gauge!("heimdall_pipeline_processing_rate"),
            acquisition_history: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            processing_history: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            last_update: Arc::new(Mutex::new(Instant::now())),
            prometheus_handle: Some(prometheus_handle),
        }
    }
    
    /// Enregistre une acquisition d'image
    pub fn record_acquisition(&self, latency_ms: f64) {
        self.frames_acquired.increment(1);
        self.acquisition_latency.record(latency_ms);
        
        // Ajouter l'horodatage à l'historique
        let mut history = self.acquisition_history.lock().unwrap();
        history.push_back(Instant::now());
        
        // Supprimer les entrées trop anciennes
        let cutoff = Instant::now() - Duration::from_secs(METRICS_WINDOW_SECONDS);
        while let Some(timestamp) = history.front() {
            if *timestamp < cutoff {
                history.pop_front();
            } else {
                break;
            }
        }
        
        // Mettre à jour le taux d'acquisition
        let rate = history.len() as f64 / METRICS_WINDOW_SECONDS as f64;
        self.acquisition_rate.set(rate);
        
        // Mettre à jour les métriques si nécessaire
        self.update_metrics_if_needed();
    }
    
    /// Enregistre un traitement d'image
    pub fn record_processing(&self, latency_ms: f64) {
        self.frames_processed.increment(1);
        self.processing_latency.record(latency_ms);
        
        // Ajouter l'horodatage à l'historique
        let mut history = self.processing_history.lock().unwrap();
        history.push_back(Instant::now());
        
        // Supprimer les entrées trop anciennes
        let cutoff = Instant::now() - Duration::from_secs(METRICS_WINDOW_SECONDS);
        while let Some(timestamp) = history.front() {
            if *timestamp < cutoff {
                history.pop_front();
            } else {
                break;
            }
        }
        
        // Mettre à jour le taux de traitement
        let rate = history.len() as f64 / METRICS_WINDOW_SECONDS as f64;
        self.processing_rate.set(rate);
        
        // Mettre à jour les métriques si nécessaire
        self.update_metrics_if_needed();
    }
    
    /// Enregistre une image perdue
    pub fn record_dropped_frame(&self) {
        self.frames_dropped.increment(1);
        self.update_metrics_if_needed();
    }
    
    /// Enregistre un débordement de buffer
    pub fn record_buffer_overflow(&self) {
        self.buffer_overflows.increment(1);
        self.update_metrics_if_needed();
    }
    
    /// Enregistre une désynchronisation
    pub fn record_desync(&self) {
        self.desync_events.increment(1);
        self.update_metrics_if_needed();
    }
    
    /// Enregistre une récupération
    pub fn record_recovery(&self) {
        self.recovery_events.increment(1);
        self.update_metrics_if_needed();
    }
    
    /// Met à jour l'utilisation du buffer
    pub fn update_buffer_usage(&self, used: usize, capacity: usize) {
        let usage = if capacity > 0 {
            (used as f64 / capacity as f64) * 100.0
        } else {
            0.0
        };
        
        self.buffer_usage.set(usage);
        self.update_metrics_if_needed();
    }
    
    /// Met à jour les métriques si l'intervalle est écoulé
    fn update_metrics_if_needed(&self) {
        let mut last_update = self.last_update.lock().unwrap();
        let now = Instant::now();
        
        if now.duration_since(*last_update).as_millis() >= METRICS_UPDATE_INTERVAL_MS as u128 {
            *last_update = now;
            
            // Mettre à jour les taux
            let acquisition_rate = {
                let history = self.acquisition_history.lock().unwrap();
                history.len() as f64 / METRICS_WINDOW_SECONDS as f64
            };
            
            let processing_rate = {
                let history = self.processing_history.lock().unwrap();
                history.len() as f64 / METRICS_WINDOW_SECONDS as f64
            };
            
            self.acquisition_rate.set(acquisition_rate);
            self.processing_rate.set(processing_rate);
        }
    }
    
    /// Obtient les statistiques actuelles du pipeline
    pub fn get_stats(&self) -> PipelineStats {
        let acquisition_history = self.acquisition_history.lock().unwrap();
        let processing_history = self.processing_history.lock().unwrap();
        
        let acquisition_rate = acquisition_history.len() as f64 / METRICS_WINDOW_SECONDS as f64;
        let processing_rate = processing_history.len() as f64 / METRICS_WINDOW_SECONDS as f64;
        
        PipelineStats {
            total_frames_acquired: self.frames_acquired.get_counter() as u64,
            total_frames_processed: self.frames_processed.get_counter() as u64,
            total_frames_dropped: self.frames_dropped.get_counter() as u64,
            buffer_overflows: self.buffer_overflows.get_counter() as u64,
            desync_events: self.desync_events.get_counter() as u64,
            recovery_events: self.recovery_events.get_counter() as u64,
            avg_acquisition_rate: acquisition_rate,
            avg_processing_rate: processing_rate,
            avg_acquisition_latency: 0.0, // À calculer à partir de l'histogramme
            avg_processing_latency: 0.0,  // À calculer à partir de l'histogramme
            avg_buffer_usage: self.buffer_usage.get_gauge(),
            last_update: std::time::SystemTime::now(),
        }
    }
    
    /// Réinitialise toutes les métriques
    pub fn reset(&self) {
        // Réinitialiser les historiques
        {
            let mut acquisition_history = self.acquisition_history.lock().unwrap();
            acquisition_history.clear();
        }
        
        {
            let mut processing_history = self.processing_history.lock().unwrap();
            processing_history.clear();
        }
        
        // Réinitialiser les jauges
        self.buffer_usage.set(0.0);
        self.acquisition_rate.set(0.0);
        self.processing_rate.set(0.0);
        
        // Note: Les compteurs ne peuvent pas être réinitialisés dans metrics-rs
        // Nous pourrions recréer de nouveaux compteurs, mais cela compliquerait le code
    }
    
    /// Obtient le handle Prometheus pour l'exposition des métriques
    pub fn get_prometheus_handle(&self) -> Option<&PrometheusHandle> {
        self.prometheus_handle.as_ref()
    }
}

/// Extension pour les types de métriques
trait MetricExt {
    fn get_counter(&self) -> u64;
    fn get_gauge(&self) -> f64;
}

impl MetricExt for Counter {
    fn get_counter(&self) -> u64 {
        // Cette méthode n'existe pas dans l'API publique de metrics-rs
        // Dans une implémentation réelle, il faudrait soit étendre la bibliothèque,
        // soit utiliser une approche différente pour suivre les valeurs
        0
    }
}

impl MetricExt for Gauge {
    fn get_gauge(&self) -> f64 {
        // Cette méthode n'existe pas dans l'API publique de metrics-rs
        // Dans une implémentation réelle, il faudrait soit étendre la bibliothèque,
        // soit utiliser une approche différente pour suivre les valeurs
        0.0
    }
}

/// Tests unitaires
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_metrics_basic() {
        let metrics = PipelineMetrics::new();
        
        // Enregistrer quelques métriques
        metrics.record_acquisition(5.0);
        metrics.record_processing(10.0);
        metrics.record_dropped_frame();
        metrics.update_buffer_usage(5, 10);
        
        // Obtenir les statistiques
        let stats = metrics.get_stats();
        
        // Vérifier les valeurs
        assert_eq!(stats.total_frames_acquired, 1);
        assert_eq!(stats.total_frames_processed, 1);
        assert_eq!(stats.total_frames_dropped, 1);
    }
    
    #[test]
    fn test_metrics_concurrent() {
        let metrics = Arc::new(PipelineMetrics::new());
        
        // Créer des threads pour enregistrer des métriques
        let mut handles = vec![];
        let thread_count = 4;
        let operations_per_thread = 1000;
        
        for i in 0..thread_count {
            let metrics_clone = metrics.clone();
            let handle = thread::spawn(move || {
                for j in 0..operations_per_thread {
                    metrics_clone.record_acquisition((i * j % 20) as f64);
                    
                    if j % 2 == 0 {
                        metrics_clone.record_processing((i * j % 30) as f64);
                    }
                    
                    if j % 10 == 0 {
                        metrics_clone.record_dropped_frame();
                    }
                    
                    if j % 100 == 0 {
                        metrics_clone.update_buffer_usage(j % 20, 20);
                    }
                }
            });
            handles.push(handle);
        }
        
        // Attendre que tous les threads terminent
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Obtenir les statistiques finales
        let stats = metrics.get_stats();
        
        // Vérifier les totaux
        assert_eq!(stats.total_frames_acquired, thread_count as u64 * operations_per_thread as u64);
        assert_eq!(stats.total_frames_processed, thread_count as u64 * (operations_per_thread / 2) as u64);
        assert_eq!(stats.total_frames_dropped, thread_count as u64 * (operations_per_thread / 10) as u64);
    }
}
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use crate::{Measurement, MetricType, PerfError};

/// Fenêtre glissante pour les métriques
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidingWindow<T> {
    /// Valeurs dans la fenêtre
    values: VecDeque<T>,
    
    /// Taille maximale de la fenêtre
    max_size: usize,
}

impl<T> SlidingWindow<T> {
    /// Crée une nouvelle fenêtre glissante
    pub fn new(max_size: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
    
    /// Ajoute une valeur à la fenêtre
    pub fn push(&mut self, value: T) {
        if self.values.len() >= self.max_size {
            self.values.pop_front();
        }
        
        self.values.push_back(value);
    }
    
    /// Obtient les valeurs dans la fenêtre
    pub fn values(&self) -> &VecDeque<T> {
        &self.values
    }
    
    /// Obtient le nombre de valeurs dans la fenêtre
    pub fn len(&self) -> usize {
        self.values.len()
    }
    
    /// Vérifie si la fenêtre est vide
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    
    /// Vide la fenêtre
    pub fn clear(&mut self) {
        self.values.clear();
    }
}

impl<T: Clone> SlidingWindow<T> {
    /// Obtient la dernière valeur dans la fenêtre
    pub fn last(&self) -> Option<T> {
        self.values.back().cloned()
    }
    
    /// Obtient la première valeur dans la fenêtre
    pub fn first(&self) -> Option<T> {
        self.values.front().cloned()
    }
}

impl<T: Copy + std::ops::Add<Output = T> + std::ops::Div<Output = T> + From<u8>> SlidingWindow<T> {
    /// Calcule la moyenne des valeurs dans la fenêtre
    pub fn average(&self) -> Option<T> {
        if self.values.is_empty() {
            return None;
        }
        
        let mut sum = T::from(0);
        for value in &self.values {
            sum = sum + *value;
        }
        
        Some(sum / T::from(self.values.len() as u8))
    }
}

impl<T: Copy + PartialOrd> SlidingWindow<T> {
    /// Obtient la valeur minimale dans la fenêtre
    pub fn min(&self) -> Option<T> {
        if self.values.is_empty() {
            return None;
        }
        
        let mut min = self.values[0];
        for value in &self.values {
            if *value < min {
                min = *value;
            }
        }
        
        Some(min)
    }
    
    /// Obtient la valeur maximale dans la fenêtre
    pub fn max(&self) -> Option<T> {
        if self.values.is_empty() {
            return None;
        }
        
        let mut max = self.values[0];
        for value in &self.values {
            if *value > max {
                max = *value;
            }
        }
        
        Some(max)
    }
}

/// Compteur de métriques
#[derive(Debug, Clone)]
pub struct MetricCounter {
    /// Nom de la métrique
    name: String,
    
    /// Type de métrique
    metric_type: MetricType,
    
    /// Unité de mesure
    unit: String,
    
    /// Valeur actuelle
    value: f64,
    
    /// Fenêtre glissante des valeurs
    window: SlidingWindow<f64>,
    
    /// Heure de la dernière mise à jour
    last_update: Instant,
}

impl MetricCounter {
    /// Crée un nouveau compteur de métriques
    pub fn new(name: &str, metric_type: MetricType, unit: &str, window_size: usize) -> Self {
        Self {
            name: name.to_string(),
            metric_type,
            unit: unit.to_string(),
            value: 0.0,
            window: SlidingWindow::new(window_size),
            last_update: Instant::now(),
        }
    }
    
    /// Définit la valeur du compteur
    pub fn set(&mut self, value: f64) {
        self.value = value;
        self.window.push(value);
        self.last_update = Instant::now();
    }
    
    /// Incrémente le compteur
    pub fn increment(&mut self, value: f64) {
        self.value += value;
        self.window.push(self.value);
        self.last_update = Instant::now();
    }
    
    /// Réinitialise le compteur
    pub fn reset(&mut self) {
        self.value = 0.0;
        self.window.clear();
        self.last_update = Instant::now();
    }
    
    /// Obtient la valeur actuelle
    pub fn value(&self) -> f64 {
        self.value
    }
    
    /// Obtient la moyenne des valeurs
    pub fn average(&self) -> Option<f64> {
        self.window.average()
    }
    
    /// Obtient la valeur minimale
    pub fn min(&self) -> Option<f64> {
        self.window.min()
    }
    
    /// Obtient la valeur maximale
    pub fn max(&self) -> Option<f64> {
        self.window.max()
    }
    
    /// Obtient le temps écoulé depuis la dernière mise à jour
    pub fn time_since_last_update(&self) -> Duration {
        self.last_update.elapsed()
    }
    
    /// Crée une mesure à partir du compteur
    pub fn to_measurement(&self) -> Measurement {
        Measurement::new(
            self.metric_type,
            &self.name,
            self.value,
            &self.unit,
        )
    }
}

/// Chronomètre pour mesurer le temps d'exécution
#[derive(Debug)]
pub struct Timer {
    /// Nom du chronomètre
    name: String,
    
    /// Heure de début
    start: Instant,
    
    /// Fenêtre glissante des durées
    window: SlidingWindow<Duration>,
}

impl Timer {
    /// Crée un nouveau chronomètre
    pub fn new(name: &str, window_size: usize) -> Self {
        Self {
            name: name.to_string(),
            start: Instant::now(),
            window: SlidingWindow::new(window_size),
        }
    }
    
    /// Redémarre le chronomètre
    pub fn restart(&mut self) {
        self.start = Instant::now();
    }
    
    /// Arrête le chronomètre et enregistre la durée
    pub fn stop(&mut self) -> Duration {
        let duration = self.start.elapsed();
        self.window.push(duration);
        duration
    }
    
    /// Obtient la durée actuelle
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
    
    /// Obtient la durée moyenne
    pub fn average(&self) -> Option<Duration> {
        if self.window.is_empty() {
            return None;
        }
        
        let sum: u128 = self.window.values().iter().map(|d| d.as_nanos()).sum();
        let avg = sum / self.window.len() as u128;
        
        Some(Duration::from_nanos(avg as u64))
    }
    
    /// Obtient la durée minimale
    pub fn min(&self) -> Option<Duration> {
        self.window.min()
    }
    
    /// Obtient la durée maximale
    pub fn max(&self) -> Option<Duration> {
        self.window.max()
    }
    
    /// Crée une mesure à partir du chronomètre
    pub fn to_measurement(&self) -> Measurement {
        Measurement::new(
            MetricType::ExecutionTime,
            &self.name,
            self.elapsed().as_secs_f64() * 1000.0, // Convertir en millisecondes
            "ms",
        )
    }
}

/// Mesureur de débit
#[derive(Debug)]
pub struct ThroughputMeter {
    /// Nom du mesureur
    name: String,
    
    /// Compteur d'éléments
    count: u64,
    
    /// Heure de début
    start: Instant,
    
    /// Fenêtre glissante des débits
    window: SlidingWindow<f64>,
    
    /// Intervalle de mise à jour
    update_interval: Duration,
    
    /// Dernière mise à jour
    last_update: Instant,
}

impl ThroughputMeter {
    /// Crée un nouveau mesureur de débit
    pub fn new(name: &str, window_size: usize, update_interval: Duration) -> Self {
        Self {
            name: name.to_string(),
            count: 0,
            start: Instant::now(),
            window: SlidingWindow::new(window_size),
            update_interval,
            last_update: Instant::now(),
        }
    }
    
    /// Incrémente le compteur
    pub fn increment(&mut self, count: u64) {
        self.count += count;
        
        // Mettre à jour le débit si l'intervalle est écoulé
        if self.last_update.elapsed() >= self.update_interval {
            self.update();
        }
    }
    
    /// Met à jour le débit
    pub fn update(&mut self) {
        let elapsed = self.last_update.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let throughput = self.count as f64 / elapsed;
            self.window.push(throughput);
            
            // Réinitialiser le compteur et l'heure de début
            self.count = 0;
            self.last_update = Instant::now();
        }
    }
    
    /// Réinitialise le mesureur
    pub fn reset(&mut self) {
        self.count = 0;
        self.start = Instant::now();
        self.window.clear();
        self.last_update = Instant::now();
    }
    
    /// Obtient le débit actuel
    pub fn throughput(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.count as f64 / elapsed
        } else {
            0.0
        }
    }
    
    /// Obtient le débit moyen
    pub fn average(&self) -> Option<f64> {
        self.window.average()
    }
    
    /// Obtient le débit minimal
    pub fn min(&self) -> Option<f64> {
        self.window.min()
    }
    
    /// Obtient le débit maximal
    pub fn max(&self) -> Option<f64> {
        self.window.max()
    }
    
    /// Crée une mesure à partir du mesureur
    pub fn to_measurement(&self) -> Measurement {
        Measurement::new(
            MetricType::Throughput,
            &self.name,
            self.throughput(),
            "items/s",
        )
    }
}
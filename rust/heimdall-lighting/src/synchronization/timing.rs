use std::time::{Duration, Instant};
use log::{debug, error, info, warn};

/// Mesure de temps haute précision
pub struct HighPrecisionTimer {
    /// Horodatage de démarrage
    start_time: Instant,
    
    /// Durée cible
    target_duration: Duration,
    
    /// Nombre d'itérations pour la calibration
    calibration_iterations: usize,
    
    /// Délai de boucle calibré
    calibrated_loop_delay_ns: u64,
}

impl HighPrecisionTimer {
    /// Crée un nouveau timer haute précision
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            target_duration: Duration::from_micros(0),
            calibration_iterations: 1000,
            calibrated_loop_delay_ns: 0,
        }
    }
    
    /// Calibre le timer
    pub fn calibrate(&mut self) {
        info!("Calibration du timer haute précision...");
        
        // Mesurer le temps d'exécution d'une boucle vide
        let start = Instant::now();
        
        for _ in 0..self.calibration_iterations {
            // Boucle vide pour mesurer l'overhead
            std::hint::black_box(());
        }
        
        let elapsed = start.elapsed();
        self.calibrated_loop_delay_ns = elapsed.as_nanos() as u64 / self.calibration_iterations as u64;
        
        info!("Délai de boucle calibré: {} ns", self.calibrated_loop_delay_ns);
    }
    
    /// Démarre le timer
    pub fn start(&mut self, duration: Duration) {
        self.start_time = Instant::now();
        self.target_duration = duration;
    }
    
    /// Attend la fin du timer avec une précision élevée
    pub fn wait_precise(&self) {
        let target_time = self.start_time + self.target_duration;
        
        // Attente grossière avec sleep
        let now = Instant::now();
        if now < target_time {
            let sleep_duration = target_time - now;
            
            // Ne pas dormir pour les très courtes durées
            if sleep_duration > Duration::from_micros(50) {
                // Dormir un peu moins que nécessaire pour éviter de dépasser
                let safe_sleep = sleep_duration - Duration::from_micros(50);
                if safe_sleep > Duration::from_nanos(0) {
                    std::thread::sleep(safe_sleep);
                }
            }
        }
        
        // Attente fine avec busy-waiting
        while Instant::now() < target_time {
            // Busy-waiting
            std::hint::spin_loop();
        }
    }
    
    /// Attend un délai précis
    pub fn delay_precise(duration: Duration) {
        let start = Instant::now();
        let target = start + duration;
        
        // Attente grossière avec sleep
        let now = Instant::now();
        if now < target {
            let sleep_duration = target - now;
            
            // Ne pas dormir pour les très courtes durées
            if sleep_duration > Duration::from_micros(50) {
                // Dormir un peu moins que nécessaire pour éviter de dépasser
                let safe_sleep = sleep_duration - Duration::from_micros(50);
                if safe_sleep > Duration::from_nanos(0) {
                    std::thread::sleep(safe_sleep);
                }
            }
        }
        
        // Attente fine avec busy-waiting
        while Instant::now() < target {
            // Busy-waiting
            std::hint::spin_loop();
        }
    }
    
    /// Mesure le temps écoulé depuis le démarrage
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    /// Vérifie si le timer est terminé
    pub fn is_elapsed(&self) -> bool {
        self.elapsed() >= self.target_duration
    }
}

/// Mesure de jitter (variation de timing)
pub struct JitterMeasurement {
    /// Valeurs mesurées
    values: Vec<Duration>,
    
    /// Valeur minimale
    min: Duration,
    
    /// Valeur maximale
    max: Duration,
    
    /// Somme des valeurs
    sum: Duration,
}

impl JitterMeasurement {
    /// Crée une nouvelle mesure de jitter
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            min: Duration::from_secs(u64::MAX),
            max: Duration::from_secs(0),
            sum: Duration::from_secs(0),
        }
    }
    
    /// Ajoute une mesure
    pub fn add(&mut self, value: Duration) {
        self.values.push(value);
        
        if value < self.min {
            self.min = value;
        }
        
        if value > self.max {
            self.max = value;
        }
        
        self.sum += value;
    }
    
    /// Calcule la moyenne
    pub fn mean(&self) -> Duration {
        if self.values.is_empty() {
            Duration::from_secs(0)
        } else {
            let nanos = self.sum.as_nanos() / self.values.len() as u128;
            Duration::from_nanos(nanos as u64)
        }
    }
    
    /// Calcule le jitter (écart entre min et max)
    pub fn jitter(&self) -> Duration {
        if self.values.is_empty() {
            Duration::from_secs(0)
        } else {
            self.max - self.min
        }
    }
    
    /// Calcule l'écart-type
    pub fn std_dev(&self) -> Duration {
        if self.values.len() < 2 {
            return Duration::from_secs(0);
        }
        
        let mean = self.mean();
        let mean_nanos = mean.as_nanos() as f64;
        
        // Calculer la somme des carrés des écarts
        let variance = self.values.iter()
            .map(|v| {
                let diff = v.as_nanos() as f64 - mean_nanos;
                diff * diff
            })
            .sum::<f64>() / (self.values.len() - 1) as f64;
            
        let std_dev = variance.sqrt();
        Duration::from_nanos(std_dev as u64)
    }
    
    /// Réinitialise les mesures
    pub fn reset(&mut self) {
        self.values.clear();
        self.min = Duration::from_secs(u64::MAX);
        self.max = Duration::from_secs(0);
        self.sum = Duration::from_secs(0);
    }
    
    /// Obtient les statistiques
    pub fn get_stats(&self) -> JitterStats {
        JitterStats {
            count: self.values.len(),
            min: self.min,
            max: self.max,
            mean: self.mean(),
            jitter: self.jitter(),
            std_dev: self.std_dev(),
        }
    }
}

/// Statistiques de jitter
#[derive(Debug, Clone, Copy)]
pub struct JitterStats {
    /// Nombre de mesures
    pub count: usize,
    
    /// Valeur minimale
    pub min: Duration,
    
    /// Valeur maximale
    pub max: Duration,
    
    /// Valeur moyenne
    pub mean: Duration,
    
    /// Jitter (écart entre min et max)
    pub jitter: Duration,
    
    /// Écart-type
    pub std_dev: Duration,
}

/// Mesure de latence entre deux événements
pub struct LatencyMeasurement {
    /// Mesure de jitter
    jitter: JitterMeasurement,
    
    /// Horodatage du dernier événement de départ
    last_start: Option<Instant>,
}

impl LatencyMeasurement {
    /// Crée une nouvelle mesure de latence
    pub fn new() -> Self {
        Self {
            jitter: JitterMeasurement::new(),
            last_start: None,
        }
    }
    
    /// Enregistre un événement de départ
    pub fn start(&mut self) {
        self.last_start = Some(Instant::now());
    }
    
    /// Enregistre un événement d'arrivée et calcule la latence
    pub fn end(&mut self) -> Option<Duration> {
        if let Some(start) = self.last_start {
            let latency = start.elapsed();
            self.jitter.add(latency);
            self.last_start = None;
            Some(latency)
        } else {
            None
        }
    }
    
    /// Obtient les statistiques de latence
    pub fn get_stats(&self) -> JitterStats {
        self.jitter.get_stats()
    }
    
    /// Réinitialise les mesures
    pub fn reset(&mut self) {
        self.jitter.reset();
        self.last_start = None;
    }
}
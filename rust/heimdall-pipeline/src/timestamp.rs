use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicU64, Ordering};
use std::ops::{Add, Sub};
use std::fmt;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Horodatage précis pour les acquisitions d'images
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp {
    /// Secondes depuis l'époque UNIX
    seconds: u64,
    
    /// Nanosecondes dans la seconde courante
    nanoseconds: u32,
    
    /// Compteur monotone pour garantir l'ordre (en cas de correction d'horloge)
    monotonic_counter: u64,
}

/// Compteur monotone global pour garantir l'ordre des timestamps
static GLOBAL_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Timestamp {
    /// Crée un nouveau timestamp avec l'heure actuelle
    pub fn now() -> Self {
        // Obtenir l'heure système
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0));
        
        // Incrémenter le compteur monotone
        let counter = GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
        
        Self {
            seconds: duration.as_secs(),
            nanoseconds: duration.subsec_nanos(),
            monotonic_counter: counter,
        }
    }
    
    /// Crée un timestamp à partir de secondes et nanosecondes
    pub fn from_seconds_nanos(seconds: u64, nanoseconds: u32) -> Self {
        // Incrémenter le compteur monotone
        let counter = GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
        
        Self {
            seconds,
            nanoseconds,
            monotonic_counter: counter,
        }
    }
    
    /// Crée un timestamp à partir d'un SystemTime
    pub fn from_system_time(time: SystemTime) -> Self {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0));
        
        // Incrémenter le compteur monotone
        let counter = GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
        
        Self {
            seconds: duration.as_secs(),
            nanoseconds: duration.subsec_nanos(),
            monotonic_counter: counter,
        }
    }
    
    /// Convertit le timestamp en SystemTime
    pub fn to_system_time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::new(self.seconds, self.nanoseconds)
    }
    
    /// Convertit le timestamp en DateTime<Utc>
    pub fn to_datetime(&self) -> DateTime<Utc> {
        let system_time = self.to_system_time();
        DateTime::<Utc>::from(system_time)
    }
    
    /// Obtient les secondes depuis l'époque UNIX
    pub fn seconds(&self) -> u64 {
        self.seconds
    }
    
    /// Obtient les nanosecondes dans la seconde courante
    pub fn nanoseconds(&self) -> u32 {
        self.nanoseconds
    }
    
    /// Obtient le compteur monotone
    pub fn monotonic_counter(&self) -> u64 {
        self.monotonic_counter
    }
    
    /// Calcule la différence entre deux timestamps en nanosecondes
    pub fn diff_nanos(&self, other: &Self) -> i64 {
        let self_nanos = (self.seconds as i64) * 1_000_000_000 + (self.nanoseconds as i64);
        let other_nanos = (other.seconds as i64) * 1_000_000_000 + (other.nanoseconds as i64);
        self_nanos - other_nanos
    }
    
    /// Calcule la différence entre deux timestamps en microsecondes
    pub fn diff_micros(&self, other: &Self) -> i64 {
        self.diff_nanos(other) / 1_000
    }
    
    /// Calcule la différence entre deux timestamps en millisecondes
    pub fn diff_millis(&self, other: &Self) -> i64 {
        self.diff_nanos(other) / 1_000_000
    }
    
    /// Ajoute une durée au timestamp
    pub fn add_duration(&self, duration: Duration) -> Self {
        let mut seconds = self.seconds + duration.as_secs();
        let mut nanoseconds = self.nanoseconds + duration.subsec_nanos();
        
        if nanoseconds >= 1_000_000_000 {
            seconds += 1;
            nanoseconds -= 1_000_000_000;
        }
        
        // Incrémenter le compteur monotone
        let counter = GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
        
        Self {
            seconds,
            nanoseconds,
            monotonic_counter: counter,
        }
    }
    
    /// Soustrait une durée du timestamp
    pub fn sub_duration(&self, duration: Duration) -> Self {
        let total_nanos_self = self.seconds as u128 * 1_000_000_000 + self.nanoseconds as u128;
        let total_nanos_duration = duration.as_secs() as u128 * 1_000_000_000 + duration.subsec_nanos() as u128;
        
        let total_nanos_result = if total_nanos_self >= total_nanos_duration {
            total_nanos_self - total_nanos_duration
        } else {
            0
        };
        
        let seconds = (total_nanos_result / 1_000_000_000) as u64;
        let nanoseconds = (total_nanos_result % 1_000_000_000) as u32;
        
        // Incrémenter le compteur monotone
        let counter = GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
        
        Self {
            seconds,
            nanoseconds,
            monotonic_counter: counter,
        }
    }
    
    /// Convertit le timestamp en durée depuis l'époque UNIX
    pub fn to_duration(&self) -> Duration {
        Duration::new(self.seconds, self.nanoseconds)
    }
    
    /// Vérifie si le timestamp est dans le futur
    pub fn is_future(&self) -> bool {
        let now = Self::now();
        self > &now
    }
    
    /// Vérifie si le timestamp est dans le passé
    pub fn is_past(&self) -> bool {
        let now = Self::now();
        self < &now
    }
    
    /// Calcule l'âge du timestamp (durée écoulée depuis ce timestamp)
    pub fn age(&self) -> Duration {
        let now = Self::now();
        if now > *self {
            Duration::new(
                now.seconds - self.seconds,
                if now.nanoseconds >= self.nanoseconds {
                    now.nanoseconds - self.nanoseconds
                } else {
                    1_000_000_000 + now.nanoseconds - self.nanoseconds
                }
            )
        } else {
            Duration::new(0, 0)
        }
    }
    
    /// Formate le timestamp en chaîne ISO 8601
    pub fn to_iso8601(&self) -> String {
        self.to_datetime().to_rfc3339()
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timestamp({}.{:09}, #{})", self.seconds, self.nanoseconds, self.monotonic_counter)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:09}", self.seconds, self.nanoseconds)
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;
    
    fn add(self, rhs: Duration) -> Self::Output {
        self.add_duration(rhs)
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Self;
    
    fn sub(self, rhs: Duration) -> Self::Output {
        self.sub_duration(rhs)
    }
}

impl Sub<Timestamp> for Timestamp {
    type Output = Duration;
    
    fn sub(self, rhs: Timestamp) -> Self::Output {
        let diff_nanos = self.diff_nanos(&rhs);
        if diff_nanos >= 0 {
            Duration::new(
                (diff_nanos / 1_000_000_000) as u64,
                (diff_nanos % 1_000_000_000) as u32
            )
        } else {
            let abs_diff = -diff_nanos;
            Duration::new(
                (abs_diff / 1_000_000_000) as u64,
                (abs_diff % 1_000_000_000) as u32
            )
        }
    }
}

/// Tests unitaires
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_timestamp_creation() {
        let ts = Timestamp::now();
        assert!(ts.seconds > 0);
        
        let ts2 = Timestamp::from_seconds_nanos(100, 200);
        assert_eq!(ts2.seconds, 100);
        assert_eq!(ts2.nanoseconds, 200);
        
        let system_time = SystemTime::now();
        let ts3 = Timestamp::from_system_time(system_time);
        let back_to_system = ts3.to_system_time();
        
        // La conversion peut introduire une petite différence due à l'arrondi
        let diff = if back_to_system > system_time {
            back_to_system.duration_since(system_time).unwrap()
        } else {
            system_time.duration_since(back_to_system).unwrap()
        };
        
        assert!(diff < Duration::from_micros(1));
    }
    
    #[test]
    fn test_timestamp_comparison() {
        let ts1 = Timestamp::from_seconds_nanos(100, 200);
        let ts2 = Timestamp::from_seconds_nanos(100, 300);
        let ts3 = Timestamp::from_seconds_nanos(101, 100);
        
        assert!(ts1 < ts2);
        assert!(ts2 < ts3);
        assert!(ts1 < ts3);
        
        assert_eq!(ts1.diff_nanos(&ts2), -100);
        assert_eq!(ts3.diff_nanos(&ts1), 999_999_900);
    }
    
    #[test]
    fn test_timestamp_arithmetic() {
        let ts = Timestamp::from_seconds_nanos(100, 500_000_000);
        
        let ts_plus = ts.add_duration(Duration::from_millis(600));
        assert_eq!(ts_plus.seconds, 101);
        assert_eq!(ts_plus.nanoseconds, 100_000_000);
        
        let ts_minus = ts.sub_duration(Duration::from_millis(600));
        assert_eq!(ts_minus.seconds, 99);
        assert_eq!(ts_minus.nanoseconds, 900_000_000);
        
        let duration = ts_plus - ts_minus;
        assert_eq!(duration.as_millis(), 1200);
    }
    
    #[test]
    fn test_timestamp_monotonic() {
        let ts1 = Timestamp::now();
        let ts2 = Timestamp::now();
        
        // Même si les timestamps sont très proches, le compteur monotone garantit l'ordre
        assert!(ts1.monotonic_counter < ts2.monotonic_counter);
        
        // Test avec plusieurs threads
        let mut handles = vec![];
        let timestamps_per_thread = 1000;
        let thread_count = 4;
        
        for _ in 0..thread_count {
            let handle = thread::spawn(move || {
                let mut timestamps = Vec::with_capacity(timestamps_per_thread);
                for _ in 0..timestamps_per_thread {
                    timestamps.push(Timestamp::now());
                }
                timestamps
            });
            handles.push(handle);
        }
        
        let mut all_timestamps = vec![];
        for handle in handles {
            let thread_timestamps = handle.join().unwrap();
            all_timestamps.extend(thread_timestamps);
        }
        
        // Trier les timestamps
        all_timestamps.sort();
        
        // Vérifier que tous les compteurs monotones sont uniques et croissants
        for i in 1..all_timestamps.len() {
            assert!(all_timestamps[i-1].monotonic_counter < all_timestamps[i].monotonic_counter);
        }
    }
}
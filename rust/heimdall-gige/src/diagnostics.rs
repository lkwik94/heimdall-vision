//! Diagnostics et surveillance des caméras GigE Vision
//!
//! Ce module fournit des outils pour diagnostiquer et surveiller
//! l'état des caméras GigE Vision et du système d'acquisition.

use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, SystemTime};

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::sync::SyncStatus;

/// Résultat d'un test de diagnostic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Nom du test
    pub name: String,
    
    /// Succès ou échec
    pub success: bool,
    
    /// Message de résultat
    pub message: String,
    
    /// Horodatage du test
    pub timestamp: SystemTime,
    
    /// Durée du test
    pub duration: Duration,
    
    /// Données supplémentaires
    pub data: HashMap<String, String>,
}

impl TestResult {
    /// Crée un nouveau résultat de test réussi
    pub fn success(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            success: true,
            message: message.to_string(),
            timestamp: SystemTime::now(),
            duration: Duration::from_millis(0),
            data: HashMap::new(),
        }
    }
    
    /// Crée un nouveau résultat de test échoué
    pub fn failure(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            success: false,
            message: message.to_string(),
            timestamp: SystemTime::now(),
            duration: Duration::from_millis(0),
            data: HashMap::new(),
        }
    }
    
    /// Définit la durée du test
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    
    /// Ajoute une donnée supplémentaire
    pub fn with_data(mut self, key: &str, value: &str) -> Self {
        self.data.insert(key.to_string(), value.to_string());
        self
    }
}

/// État d'une caméra
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraStatus {
    /// Identifiant de la caméra
    pub id: String,
    
    /// État de connexion
    pub connected: bool,
    
    /// État d'acquisition
    pub acquiring: bool,
    
    /// Température du capteur (en degrés Celsius)
    pub sensor_temperature: Option<f32>,
    
    /// Température du boîtier (en degrés Celsius)
    pub housing_temperature: Option<f32>,
    
    /// Utilisation de la bande passante (en Mo/s)
    pub bandwidth_usage: Option<f64>,
    
    /// Taux de perte de paquets (en pourcentage)
    pub packet_loss_rate: Option<f64>,
    
    /// Nombre d'images acquises
    pub frame_count: u64,
    
    /// Nombre d'erreurs d'acquisition
    pub error_count: u64,
    
    /// Horodatage de la dernière image
    pub last_frame_time: Option<SystemTime>,
    
    /// Temps d'exposition actuel (en microsecondes)
    pub exposure_time_us: u64,
    
    /// Gain actuel (en dB)
    pub gain_db: f64,
    
    /// Statistiques d'image
    pub image_stats: Option<ImageStats>,
    
    /// Erreurs récentes
    pub recent_errors: Vec<String>,
}

/// Statistiques d'image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageStats {
    /// Valeur minimale
    pub min: u8,
    
    /// Valeur maximale
    pub max: u8,
    
    /// Valeur moyenne
    pub mean: f64,
    
    /// Écart-type
    pub std_dev: f64,
    
    /// Histogramme simplifié (10 bins)
    pub histogram: [u32; 10],
}

/// Rapport de diagnostic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    /// Horodatage du rapport
    pub timestamp: SystemTime,
    
    /// Résultats des tests
    pub test_results: HashMap<String, TestResult>,
    
    /// États des caméras
    pub camera_statuses: HashMap<String, CameraStatus>,
    
    /// État de synchronisation
    pub sync_status: Option<SyncStatus>,
    
    /// Métriques de performance
    pub performance_metrics: HashMap<String, f64>,
}

impl DiagnosticReport {
    /// Crée un nouveau rapport de diagnostic
    pub fn new() -> Self {
        Self {
            timestamp: SystemTime::now(),
            test_results: HashMap::new(),
            camera_statuses: HashMap::new(),
            sync_status: None,
            performance_metrics: HashMap::new(),
        }
    }
    
    /// Ajoute un résultat de test
    pub fn add_test(&mut self, name: &str, result: TestResult) {
        self.test_results.insert(name.to_string(), result);
    }
    
    /// Ajoute un état de caméra
    pub fn add_camera_status(&mut self, camera_id: &str, status: CameraStatus) {
        self.camera_statuses.insert(camera_id.to_string(), status);
    }
    
    /// Ajoute un état de synchronisation
    pub fn add_sync_status(&mut self, status: SyncStatus) {
        self.sync_status = Some(status);
    }
    
    /// Ajoute une métrique de performance
    pub fn add_performance_metric(&mut self, name: &str, value: f64) {
        self.performance_metrics.insert(name.to_string(), value);
    }
    
    /// Vérifie si tous les tests ont réussi
    pub fn all_tests_passed(&self) -> bool {
        self.test_results.values().all(|r| r.success)
    }
    
    /// Vérifie si toutes les caméras sont connectées
    pub fn all_cameras_connected(&self) -> bool {
        self.camera_statuses.values().all(|s| s.connected)
    }
    
    /// Obtient un résumé du rapport
    pub fn summary(&self) -> String {
        let test_count = self.test_results.len();
        let passed_tests = self.test_results.values().filter(|r| r.success).count();
        
        let camera_count = self.camera_statuses.len();
        let connected_cameras = self.camera_statuses.values().filter(|s| s.connected).count();
        
        format!(
            "Tests: {}/{} réussis, Caméras: {}/{} connectées",
            passed_tests, test_count, connected_cameras, camera_count
        )
    }
}

impl Default for DiagnosticReport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DiagnosticReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Rapport de diagnostic - {}", self.summary())?;
        
        if !self.test_results.is_empty() {
            writeln!(f, "\nRésultats des tests:")?;
            for (name, result) in &self.test_results {
                let status = if result.success { "✓" } else { "✗" };
                writeln!(f, "  {} {} - {}", status, name, result.message)?;
            }
        }
        
        if !self.camera_statuses.is_empty() {
            writeln!(f, "\nÉtats des caméras:")?;
            for (id, status) in &self.camera_statuses {
                let conn_status = if status.connected { "connectée" } else { "déconnectée" };
                let acq_status = if status.acquiring { "en acquisition" } else { "inactive" };
                writeln!(f, "  Caméra {} - {} - {}", id, conn_status, acq_status)?;
                
                if let Some(temp) = status.sensor_temperature {
                    writeln!(f, "    Température du capteur: {:.1} °C", temp)?;
                }
                
                if status.error_count > 0 {
                    writeln!(f, "    Erreurs: {}", status.error_count)?;
                }
            }
        }
        
        if let Some(sync) = &self.sync_status {
            writeln!(f, "\nÉtat de synchronisation:")?;
            writeln!(f, "  Mode: {:?}", sync.mode)?;
            writeln!(f, "  Déclenchements: {}", sync.trigger_count)?;
            
            if let Some(interval) = sync.average_interval_us {
                writeln!(f, "  Intervalle moyen: {} µs", interval)?;
            }
            
            if let Some(jitter) = sync.sync_jitter_us {
                writeln!(f, "  Jitter: {} µs", jitter)?;
            }
        }
        
        if !self.performance_metrics.is_empty() {
            writeln!(f, "\nMétriques de performance:")?;
            for (name, value) in &self.performance_metrics {
                writeln!(f, "  {}: {:.2}", name, value)?;
            }
        }
        
        Ok(())
    }
}

/// Teste la connectivité réseau
pub async fn test_network_connectivity() -> TestResult {
    debug!("Test de connectivité réseau");
    
    let start_time = std::time::Instant::now();
    let mut success = true;
    let mut message = "Connectivité réseau OK".to_string();
    let mut data = HashMap::new();
    
    // Tester la connectivité vers des adresses IP typiques de caméras GigE
    let test_ips = [
        IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),   // Adresse typique GigE Vision
        IpAddr::V4(Ipv4Addr::new(169, 254, 2, 1)),   // Autre adresse typique
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), // Adresse réseau local typique
    ];
    
    let mut failed_ips = Vec::new();
    
    for &ip in &test_ips {
        let addr = format!("{}:3956", ip); // Port GigE Vision standard
        
        match timeout(Duration::from_secs(1), TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => {
                data.insert(ip.to_string(), "connecté".to_string());
            },
            _ => {
                data.insert(ip.to_string(), "non connecté".to_string());
                failed_ips.push(ip);
            }
        }
    }
    
    if !failed_ips.is_empty() {
        success = false;
        message = format!(
            "Impossible de se connecter à {} adresses IP", 
            failed_ips.len()
        );
    }
    
    // Vérifier la MTU
    let mtu = get_network_mtu().await;
    data.insert("mtu".to_string(), mtu.to_string());
    
    if mtu < 8000 {
        warn!("MTU réseau sous-optimal pour GigE Vision: {}", mtu);
        data.insert("mtu_warning".to_string(), "MTU sous-optimal pour GigE Vision".to_string());
    }
    
    TestResult {
        name: "network_connectivity".to_string(),
        success,
        message,
        timestamp: SystemTime::now(),
        duration: start_time.elapsed(),
        data,
    }
}

/// Obtient la MTU du réseau
async fn get_network_mtu() -> u32 {
    // Cette fonction simule la détection de la MTU
    // En production, elle utiliserait des commandes système ou des API réseau
    
    // Simuler une MTU typique pour Jumbo Frames
    9000
}

/// Teste les performances d'acquisition
pub async fn test_acquisition_performance(
    frame_rate: f64,
    frame_count: u64,
    latency_ms: f64,
) -> TestResult {
    let start_time = std::time::Instant::now();
    
    let mut data = HashMap::new();
    data.insert("frame_rate".to_string(), frame_rate.to_string());
    data.insert("frame_count".to_string(), frame_count.to_string());
    data.insert("latency_ms".to_string(), latency_ms.to_string());
    
    let success = frame_rate >= 25.0 && latency_ms <= 5.0;
    
    let message = if success {
        format!(
            "Performances d'acquisition satisfaisantes: {:.1} FPS, {:.2} ms de latence",
            frame_rate, latency_ms
        )
    } else {
        format!(
            "Performances d'acquisition insuffisantes: {:.1} FPS, {:.2} ms de latence",
            frame_rate, latency_ms
        )
    };
    
    TestResult {
        name: "acquisition_performance".to_string(),
        success,
        message,
        timestamp: SystemTime::now(),
        duration: start_time.elapsed(),
        data,
    }
}

/// Teste la qualité des images
pub fn test_image_quality(stats: &ImageStats) -> TestResult {
    let start_time = std::time::Instant::now();
    
    let mut data = HashMap::new();
    data.insert("min".to_string(), stats.min.to_string());
    data.insert("max".to_string(), stats.max.to_string());
    data.insert("mean".to_string(), stats.mean.to_string());
    data.insert("std_dev".to_string(), stats.std_dev.to_string());
    
    // Vérifier si l'image a un bon contraste et n'est pas saturée
    let dynamic_range = stats.max as f64 - stats.min as f64;
    let is_saturated = stats.min == 0 && stats.histogram[0] > 0 || 
                       stats.max == 255 && stats.histogram[9] > 0;
    
    let success = dynamic_range >= 50.0 && !is_saturated;
    
    let message = if success {
        format!(
            "Qualité d'image satisfaisante: plage dynamique de {:.0}, moyenne de {:.1}",
            dynamic_range, stats.mean
        )
    } else if is_saturated {
        "Image saturée: ajuster l'exposition ou le gain".to_string()
    } else {
        format!(
            "Contraste insuffisant: plage dynamique de {:.0}",
            dynamic_range
        )
    };
    
    TestResult {
        name: "image_quality".to_string(),
        success,
        message,
        timestamp: SystemTime::now(),
        duration: start_time.elapsed(),
        data,
    }
}

/// Teste la synchronisation des caméras
pub fn test_camera_synchronization(sync_status: &SyncStatus) -> TestResult {
    let start_time = std::time::Instant::now();
    
    let mut data = HashMap::new();
    data.insert("mode".to_string(), format!("{:?}", sync_status.mode));
    data.insert("trigger_count".to_string(), sync_status.trigger_count.to_string());
    
    if let Some(jitter) = sync_status.sync_jitter_us {
        data.insert("jitter_us".to_string(), jitter.to_string());
    }
    
    // Vérifier si le jitter est acceptable (< 100 µs)
    let success = sync_status.sync_jitter_us.map_or(true, |j| j < 100);
    
    let message = if success {
        if let Some(jitter) = sync_status.sync_jitter_us {
            format!("Synchronisation satisfaisante: jitter de {} µs", jitter)
        } else {
            "Synchronisation satisfaisante".to_string()
        }
    } else {
        format!(
            "Jitter de synchronisation trop élevé: {} µs",
            sync_status.sync_jitter_us.unwrap_or(0)
        )
    };
    
    TestResult {
        name: "camera_synchronization".to_string(),
        success,
        message,
        timestamp: SystemTime::now(),
        duration: start_time.elapsed(),
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_diagnostic_report() {
        let mut report = DiagnosticReport::new();
        
        // Ajouter des résultats de test
        report.add_test(
            "test1",
            TestResult::success("test1", "Test réussi")
                .with_duration(Duration::from_millis(10)),
        );
        
        report.add_test(
            "test2",
            TestResult::failure("test2", "Test échoué")
                .with_duration(Duration::from_millis(20)),
        );
        
        // Vérifier les résultats
        assert_eq!(report.test_results.len(), 2);
        assert!(report.test_results.get("test1").unwrap().success);
        assert!(!report.test_results.get("test2").unwrap().success);
        assert!(!report.all_tests_passed());
    }
    
    #[test]
    fn test_camera_status() {
        let mut report = DiagnosticReport::new();
        
        // Ajouter des états de caméra
        let camera1 = CameraStatus {
            id: "camera1".to_string(),
            connected: true,
            acquiring: true,
            sensor_temperature: Some(35.5),
            housing_temperature: Some(30.2),
            bandwidth_usage: Some(120.5),
            packet_loss_rate: Some(0.0),
            frame_count: 1000,
            error_count: 0,
            last_frame_time: Some(SystemTime::now()),
            exposure_time_us: 10000,
            gain_db: 2.0,
            image_stats: None,
            recent_errors: Vec::new(),
        };
        
        let camera2 = CameraStatus {
            id: "camera2".to_string(),
            connected: false,
            acquiring: false,
            sensor_temperature: None,
            housing_temperature: None,
            bandwidth_usage: None,
            packet_loss_rate: None,
            frame_count: 0,
            error_count: 1,
            last_frame_time: None,
            exposure_time_us: 0,
            gain_db: 0.0,
            image_stats: None,
            recent_errors: vec!["Connexion perdue".to_string()],
        };
        
        report.add_camera_status("camera1", camera1);
        report.add_camera_status("camera2", camera2);
        
        // Vérifier les états
        assert_eq!(report.camera_statuses.len(), 2);
        assert!(report.camera_statuses.get("camera1").unwrap().connected);
        assert!(!report.camera_statuses.get("camera2").unwrap().connected);
        assert!(!report.all_cameras_connected());
    }
    
    #[test]
    fn test_image_quality_test() {
        let stats = ImageStats {
            min: 20,
            max: 220,
            mean: 120.0,
            std_dev: 40.0,
            histogram: [0, 10, 20, 30, 40, 30, 20, 10, 5, 0],
        };
        
        let result = test_image_quality(&stats);
        assert!(result.success);
        
        let stats_poor = ImageStats {
            min: 100,
            max: 120,
            mean: 110.0,
            std_dev: 5.0,
            histogram: [0, 0, 0, 0, 0, 100, 100, 0, 0, 0],
        };
        
        let result = test_image_quality(&stats_poor);
        assert!(!result.success);
    }
}
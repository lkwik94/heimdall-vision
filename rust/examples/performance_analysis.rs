use heimdall_camera::{Camera, CameraConfig, CameraFactory, PixelFormat, TriggerMode};
use heimdall_perf::{init, MetricType, Measurement, ProfilingSession};
use heimdall_perf::metrics::{MetricCounter, Timer, ThroughputMeter};
use heimdall_perf::reports::ReportFormat;
use heimdall_rt::{RtConfig, RtPriority};
use heimdall_rt::scheduler::{RtScheduler, RtTask, TaskType};
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialiser le logger
    env_logger::init();
    
    println!("Heimdall Vision - Analyse de performance");
    println!("========================================");
    
    // Initialiser le profilage
    let profiling_manager = init("./performance_reports")?;
    
    // Créer une session de profilage
    let session = {
        let mut manager = profiling_manager.lock().unwrap();
        manager.start_session("bottle_inspection")?
    };
    
    // Démarrer la collecte de métriques système
    {
        let mut session_guard = session.lock().unwrap();
        session_guard.start_system_metrics()?;
    }
    
    // Créer une caméra simulée
    println!("\nCréation d'une caméra simulée...");
    let mut camera = CameraFactory::create("simulator", "simulated_camera")?;
    
    // Configurer la caméra
    let config = CameraConfig {
        id: "simulated_camera".to_string(),
        pixel_format: PixelFormat::RGB8,
        width: 1280,
        height: 720,
        frame_rate: 30.0,
        exposure_time_us: 10000,
        gain_db: 0.0,
        trigger_mode: TriggerMode::Continuous,
        vendor_params: std::collections::HashMap::new(),
    };
    
    println!("Configuration de la caméra...");
    camera.initialize(config).await?;
    
    // Créer des compteurs de métriques
    let mut fps_counter = MetricCounter::new("fps", MetricType::Throughput, "fps", 100);
    let mut processing_timer = Timer::new("processing_time", 100);
    let mut memory_counter = MetricCounter::new("memory_usage", MetricType::MemoryUsage, "MB", 100);
    let mut throughput_meter = ThroughputMeter::new("image_throughput", 100, Duration::from_secs(1));
    
    // Démarrer l'acquisition
    println!("Démarrage de l'acquisition...");
    camera.start_acquisition().await?;
    
    // Simuler le traitement d'images
    println!("\nTraitement d'images pendant 5 secondes...");
    let start = Instant::now();
    let mut frame_count = 0;
    
    while start.elapsed() < Duration::from_secs(5) {
        // Mesurer le temps d'acquisition
        {
            let mut session_guard = session.lock().unwrap();
            session_guard.start_timing("acquisition");
        }
        
        // Acquérir une image
        let frame = camera.acquire_frame().await?;
        
        {
            let mut session_guard = session.lock().unwrap();
            session_guard.stop_timing("acquisition")?;
        }
        
        // Mesurer le temps de traitement
        processing_timer.restart();
        
        // Simuler le traitement
        let image_size = frame.data.len();
        println!("Traitement d'une image de {} octets", image_size);
        
        // Simuler un traitement intensif
        let mut sum = 0;
        for byte in &frame.data {
            sum += *byte as u64;
        }
        let mean = sum as f64 / image_size as f64;
        
        // Simuler la détection de défauts
        let defect_count = (frame_count % 5) as u64; // Simuler un défaut toutes les 5 images
        
        // Arrêter le chronomètre
        let processing_time = processing_timer.stop();
        println!("Traitement terminé en {:?}", processing_time);
        
        // Mettre à jour les métriques
        frame_count += 1;
        fps_counter.set(frame_count as f64 / start.elapsed().as_secs_f64());
        throughput_meter.increment(1);
        
        // Collecter les métriques système
        {
            let mut session_guard = session.lock().unwrap();
            if let Err(e) = session_guard.collect_system_metrics() {
                println!("Erreur lors de la collecte des métriques système: {:?}", e);
            }
        }
        
        // Simuler l'utilisation mémoire
        let memory_usage = 100.0 + (frame_count as f64 * 0.5); // Simuler une fuite mémoire
        memory_counter.set(memory_usage);
        
        // Ajouter les mesures à la session
        {
            let mut session_guard = session.lock().unwrap();
            
            // Ajouter les mesures
            session_guard.add_measurement(fps_counter.to_measurement());
            session_guard.add_measurement(processing_timer.to_measurement());
            session_guard.add_measurement(memory_counter.to_measurement());
            session_guard.add_measurement(throughput_meter.to_measurement());
            
            // Ajouter une mesure pour les défauts détectés
            session_guard.add_measurement(Measurement::new(
                MetricType::Custom,
                "defects_detected",
                defect_count as f64,
                "count",
            ));
            
            // Incrémenter le compteur de défauts
            session_guard.increment_counter("total_defects", defect_count);
        }
        
        // Attendre un peu
        sleep(Duration::from_millis(33)).await; // ~30 FPS
    }
    
    // Arrêter l'acquisition
    println!("\nArrêt de l'acquisition...");
    camera.stop_acquisition().await?;
    
    // Générer des rapports
    println!("\nGénération des rapports de performance...");
    
    {
        let session_guard = session.lock().unwrap();
        
        // Générer un rapport JSON
        session_guard.save_report(Path::new("./performance_reports/report.json"), ReportFormat::Json)?;
        
        // Générer un rapport HTML
        session_guard.save_report(Path::new("./performance_reports/report.html"), ReportFormat::Html)?;
        
        // Générer un rapport Markdown
        session_guard.save_report(Path::new("./performance_reports/report.md"), ReportFormat::Markdown)?;
        
        // Générer un flamegraph
        session_guard.generate_flamegraph(Path::new("./performance_reports/flamegraph.svg"))?;
    }
    
    // Arrêter la session
    {
        let mut manager = profiling_manager.lock().unwrap();
        manager.stop_session()?;
    }
    
    // Afficher les statistiques
    println!("\nStatistiques de performance:");
    println!("  Images traitées: {}", frame_count);
    println!("  FPS moyen: {:.2}", fps_counter.average().unwrap_or(0.0));
    println!("  Temps de traitement moyen: {:?}", processing_timer.average().unwrap_or(Duration::from_secs(0)));
    println!("  Débit moyen: {:.2} images/s", throughput_meter.average().unwrap_or(0.0));
    
    println!("\nRapports générés dans le répertoire './performance_reports'");
    println!("\nAnalyse de performance terminée avec succès!");
    
    Ok(())
}
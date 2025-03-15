use heimdall_pipeline::{
    PipelineConfig, OverflowStrategy, PipelineState, PipelineError,
    pipeline::{AcquisitionPipeline, PipelineImage},
};
use heimdall_rt::RtPriority;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::time::{Duration, Instant};
use std::thread;

fn main() -> Result<(), PipelineError> {
    // Initialiser le logger
    env_logger::init();
    
    println!("Démarrage du système d'inspection de bouteilles");
    println!("Capacité: 100 000 bouteilles/heure");
    
    // Créer la configuration du pipeline
    let config = PipelineConfig {
        buffer_capacity: 32,
        max_image_size: 1920 * 1080 * 3, // Full HD RGB
        acquisition_threads: 1,
        processing_threads: 4,
        acquisition_priority: RtPriority::Critical,
        processing_priority: RtPriority::High,
        acquisition_cpu_affinity: vec![0],
        processing_cpu_affinity: vec![1, 2, 3, 4],
        metrics_interval_ms: 1000,
        enable_auto_recovery: true,
        max_wait_time_ms: 100,
        overflow_strategy: OverflowStrategy::DropOldest,
    };
    
    // Créer le pipeline
    let pipeline = AcquisitionPipeline::new(config)?;
    
    // Compteurs pour les statistiques
    let total_processed = Arc::new(AtomicUsize::new(0));
    let defects_detected = Arc::new(AtomicUsize::new(0));
    
    // Ajouter un callback de traitement
    let total_processed_clone = total_processed.clone();
    let defects_detected_clone = defects_detected.clone();
    
    pipeline.add_processor_callback(move |image| {
        // Incrémenter le compteur d'images traitées
        total_processed_clone.fetch_add(1, Ordering::SeqCst);
        
        // Simuler la détection de défauts
        let defect_detected = detect_bottle_defect(image);
        
        if defect_detected {
            defects_detected_clone.fetch_add(1, Ordering::SeqCst);
        }
        
        Ok(())
    })?;
    
    // Initialiser le pipeline
    pipeline.initialize()?;
    
    // Démarrer le pipeline
    pipeline.start()?;
    
    println!("Pipeline démarré, appuyez sur Ctrl+C pour arrêter");
    
    // Afficher les statistiques périodiquement
    let start_time = Instant::now();
    let mut last_stats_time = Instant::now();
    
    loop {
        // Attendre un peu
        thread::sleep(Duration::from_millis(1000));
        
        // Obtenir les statistiques
        let stats = pipeline.get_stats();
        let elapsed = last_stats_time.elapsed();
        last_stats_time = Instant::now();
        
        // Calculer les taux
        let processed = total_processed.load(Ordering::SeqCst);
        let defects = defects_detected.load(Ordering::SeqCst);
        
        let processing_rate = if elapsed.as_secs() > 0 {
            processed as f64 / elapsed.as_secs() as f64
        } else {
            0.0
        };
        
        let defect_rate = if processed > 0 {
            (defects as f64 / processed as f64) * 100.0
        } else {
            0.0
        };
        
        // Afficher les statistiques
        println!("=== Statistiques d'inspection ===");
        println!("Temps d'exécution: {:.1} s", start_time.elapsed().as_secs_f64());
        println!("Images acquises: {}", stats.total_frames_acquired);
        println!("Images traitées: {}", stats.total_frames_processed);
        println!("Images perdues: {}", stats.total_frames_dropped);
        println!("Taux d'acquisition: {:.1} images/s", stats.avg_acquisition_rate);
        println!("Taux de traitement: {:.1} images/s", stats.avg_processing_rate);
        println!("Utilisation du buffer: {:.1}%", stats.avg_buffer_usage);
        println!("Défauts détectés: {} ({:.2}%)", defects, defect_rate);
        println!("");
        
        // Vérifier si l'utilisateur a appuyé sur Ctrl+C
        if std::io::stdin().read_line(&mut String::new()).is_ok() {
            break;
        }
    }
    
    // Arrêter le pipeline
    pipeline.stop()?;
    
    println!("Pipeline arrêté");
    
    Ok(())
}

/// Simule la détection de défauts dans une bouteille
fn detect_bottle_defect(image: &PipelineImage) -> bool {
    // Simuler un traitement d'image
    thread::sleep(Duration::from_millis(5));
    
    // Simuler une détection de défaut (5% de chance)
    rand::random::<u8>() < 13 // ~5% de chance
}
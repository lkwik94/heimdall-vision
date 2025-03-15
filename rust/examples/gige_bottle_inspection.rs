//! Exemple d'inspection de bouteilles avec caméras GigE Vision
//!
//! Cet exemple montre comment utiliser le module heimdall-gige pour
//! configurer et utiliser des caméras GigE Vision dans un contexte
//! d'inspection de bouteilles à haute cadence.

use std::error::Error;
use std::time::{Duration, Instant};

use heimdall_gige::{GigESystem, SyncMode};
use log::{debug, error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialiser le logger
    env_logger::init();
    
    println!("Heimdall Vision - Inspection de bouteilles avec caméras GigE");
    println!("=========================================================");
    
    // Initialiser le système GigE
    let mut gige = GigESystem::new()?;
    
    // Découvrir les caméras disponibles
    println!("\nDécouverte des caméras GigE Vision...");
    let cameras = gige.discover_cameras().await?;
    
    println!("\nCaméras découvertes:");
    for (i, camera) in cameras.iter().enumerate() {
        println!("  {}. {} ({} - {})", i + 1, camera.id, camera.model, camera.vendor);
        println!("     Adresse IP: {}", camera.ip_address);
        println!("     Résolution max: {}x{}", camera.capabilities.max_width, camera.capabilities.max_height);
        println!("     Formats de pixel: {:?}", camera.capabilities.pixel_formats);
    }
    
    // Configurer les caméras en mode synchronisé
    println!("\nConfiguration des caméras en mode synchronisé...");
    gige.configure_cameras(SyncMode::Hardware).await?;
    
    // Optimiser les paramètres pour l'inspection de bouteilles
    println!("\nOptimisation des paramètres pour l'inspection de bouteilles...");
    gige.optimize_camera_parameters().await?;
    
    // Démarrer l'acquisition
    println!("\nDémarrage de l'acquisition...");
    gige.start_acquisition().await?;
    
    // Acquérir des images pendant 10 secondes
    println!("\nAcquisition d'images pendant 10 secondes...");
    
    let start_time = Instant::now();
    let mut frame_count = 0;
    
    while start_time.elapsed() < Duration::from_secs(10) {
        // Acquérir un ensemble d'images
        let frame_set = gige.acquire_frames().await?;
        
        // Afficher des informations sur les images
        println!("\nEnsemble d'images #{}", frame_set.frame_id);
        println!("  Nombre d'images: {}", frame_set.frames.len());
        
        for (camera_id, frame) in &frame_set.frames {
            println!("  Caméra {}: {}x{} {:?}", camera_id, frame.width, frame.height, frame.pixel_format);
            
            // Calculer l'intensité moyenne
            if frame.pixel_format == heimdall_camera::PixelFormat::Mono8 {
                let mean = frame.mean()?;
                println!("    Intensité moyenne: {:.2}", mean);
            }
        }
        
        frame_count += 1;
        
        // Petite pause pour éviter de surcharger la console
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    let elapsed = start_time.elapsed();
    let fps = frame_count as f64 / elapsed.as_secs_f64();
    
    println!("\nAcquisition terminée: {} images en {:?} ({:.2} FPS)", frame_count, elapsed, fps);
    
    // Exécuter un diagnostic
    println!("\nExécution d'un diagnostic...");
    let report = gige.run_diagnostics().await?;
    
    println!("\nRapport de diagnostic:");
    println!("{}", report);
    
    // Arrêter l'acquisition
    println!("\nArrêt de l'acquisition...");
    gige.stop_acquisition().await?;
    
    println!("\nExemple terminé avec succès!");
    
    Ok(())
}
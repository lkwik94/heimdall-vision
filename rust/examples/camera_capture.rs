use heimdall_camera::{Camera, CameraConfig, CameraFactory, PixelFormat, TriggerMode};
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialiser le logger
    env_logger::init();
    
    println!("Heimdall Vision - Exemple de capture de caméra");
    println!("==============================================");
    
    // Énumérer les caméras disponibles
    println!("\nCaméras disponibles:");
    let cameras = CameraFactory::enumerate();
    for (i, (camera_type, camera_id)) in cameras.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, camera_id, camera_type);
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
    
    // Démarrer l'acquisition
    println!("Démarrage de l'acquisition...");
    camera.start_acquisition().await?;
    
    // Capturer quelques images
    println!("\nCapture de 5 images...");
    for i in 1..=5 {
        println!("\nCapture de l'image {}...", i);
        
        // Acquérir une image
        let frame = camera.acquire_frame().await?;
        
        // Afficher les informations de l'image
        println!("  Dimensions: {}x{}", frame.width, frame.height);
        println!("  Format de pixel: {:?}", frame.pixel_format);
        println!("  Taille des données: {} octets", frame.data.len());
        println!("  ID de trame: {}", frame.frame_id);
        
        // Calculer l'intensité moyenne
        let mut sum = 0;
        let mut count = 0;
        
        match frame.pixel_format {
            PixelFormat::Mono8 => {
                for pixel in &frame.data {
                    sum += *pixel as u64;
                    count += 1;
                }
            },
            PixelFormat::RGB8 | PixelFormat::BGR8 => {
                for i in (0..frame.data.len()).step_by(3) {
                    let r = frame.data[i] as u64;
                    let g = frame.data[i + 1] as u64;
                    let b = frame.data[i + 2] as u64;
                    sum += (r + g + b) / 3;
                    count += 1;
                }
            },
            _ => {
                println!("  Calcul d'intensité non supporté pour ce format de pixel");
            },
        }
        
        if count > 0 {
            let mean = sum as f64 / count as f64;
            println!("  Intensité moyenne: {:.2}", mean);
        }
        
        // Attendre un peu
        sleep(Duration::from_millis(100)).await;
    }
    
    // Arrêter l'acquisition
    println!("\nArrêt de l'acquisition...");
    camera.stop_acquisition().await?;
    
    println!("\nExemple terminé avec succès!");
    
    Ok(())
}
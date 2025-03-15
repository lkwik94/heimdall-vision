use heimdall_camera::{Camera, CameraConfig, CameraFactory, PixelFormat, TriggerMode};
use heimdall_rt::{RtConfig, RtPriority};
use heimdall_rt::scheduler::{RtScheduler, RtTask, TaskType};
use heimdall_rt::sync::{RtQueue, RtChannel, RtRwLock};
use opencv::{core, imgproc, highgui, prelude::*};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Structure représentant un défaut détecté sur une bouteille
#[derive(Debug, Clone)]
struct BottleDefect {
    /// Position du défaut (x, y)
    position: (i32, i32),
    
    /// Taille du défaut en pixels
    size: f64,
    
    /// Score de confiance (0.0 - 1.0)
    confidence: f64,
    
    /// Type de défaut
    defect_type: DefectType,
}

/// Types de défauts possibles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefectType {
    /// Contamination (corps étranger)
    Contamination,
    
    /// Fissure
    Crack,
    
    /// Déformation
    Deformation,
    
    /// Défaut de couleur
    ColorDefect,
}

/// Détecteur de défauts sur les bouteilles
struct BottleDefectDetector {
    /// Seuil de détection
    threshold: f64,
    
    /// Taille minimale du défaut
    min_size: f64,
    
    /// Taille maximale du défaut
    max_size: f64,
    
    /// Sensibilité de détection
    sensitivity: f64,
}

impl BottleDefectDetector {
    /// Crée un nouveau détecteur avec les paramètres par défaut
    fn new() -> Self {
        Self {
            threshold: 30.0,
            min_size: 10.0,
            max_size: 1000.0,
            sensitivity: 0.8,
        }
    }
    
    /// Détecte les défauts dans une image
    fn detect_defects(&self, image: &Mat) -> Result<Vec<BottleDefect>, Box<dyn Error>> {
        // Convertir en niveaux de gris si nécessaire
        let mut gray = Mat::default();
        if image.channels()? > 1 {
            imgproc::cvt_color(image, &mut gray, imgproc::COLOR_BGR2GRAY, 0)?;
        } else {
            gray = image.clone();
        }
        
        // Appliquer un flou gaussien pour réduire le bruit
        let mut blurred = Mat::default();
        imgproc::gaussian_blur(&gray, &mut blurred, core::Size::new(5, 5), 0.0, 0.0, core::BORDER_DEFAULT)?;
        
        // Appliquer un seuillage adaptatif
        let mut binary = Mat::default();
        imgproc::adaptive_threshold(
            &blurred,
            &mut binary,
            255.0,
            imgproc::ADAPTIVE_THRESH_GAUSSIAN_C,
            imgproc::THRESH_BINARY_INV,
            11,
            self.threshold,
        )?;
        
        // Trouver les contours
        let mut contours = core::Vector::<core::Vector<core::Point>>::new();
        let mut hierarchy = core::Vector::<core::Vec4i>::new();
        imgproc::find_contours(
            &binary,
            &mut contours,
            &mut hierarchy,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_SIMPLE,
            core::Point::new(0, 0),
        )?;
        
        // Analyser les contours pour détecter les défauts
        let mut defects = Vec::new();
        
        for contour in contours {
            // Calculer l'aire du contour
            let area = imgproc::contour_area(&contour, false)?;
            
            // Filtrer par taille
            if area >= self.min_size && area <= self.max_size {
                // Calculer le centre du contour
                let moments = imgproc::moments(&contour, false)?;
                if moments.m00 > 0.0 {
                    let cx = (moments.m10 / moments.m00) as i32;
                    let cy = (moments.m01 / moments.m00) as i32;
                    
                    // Calculer la circularité
                    let perimeter = imgproc::arc_length(&contour, true)?;
                    let circularity = if perimeter > 0.0 {
                        4.0 * std::f64::consts::PI * area / (perimeter * perimeter)
                    } else {
                        0.0
                    };
                    
                    // Déterminer le type de défaut
                    let defect_type = if circularity > 0.7 {
                        DefectType::Contamination
                    } else if circularity < 0.3 {
                        DefectType::Crack
                    } else if area > 500.0 {
                        DefectType::Deformation
                    } else {
                        DefectType::ColorDefect
                    };
                    
                    // Calculer la confiance
                    let confidence = (area / self.max_size).min(1.0) * self.sensitivity;
                    
                    // Ajouter le défaut
                    defects.push(BottleDefect {
                        position: (cx, cy),
                        size: area,
                        confidence,
                        defect_type,
                    });
                }
            }
        }
        
        Ok(defects)
    }
    
    /// Dessine les défauts sur une image
    fn draw_defects(&self, image: &mut Mat, defects: &[BottleDefect]) -> Result<(), Box<dyn Error>> {
        for defect in defects {
            // Choisir la couleur en fonction du type de défaut
            let color = match defect.defect_type {
                DefectType::Contamination => core::Scalar::new(0.0, 0.0, 255.0, 0.0), // Rouge
                DefectType::Crack => core::Scalar::new(0.0, 255.0, 255.0, 0.0),       // Jaune
                DefectType::Deformation => core::Scalar::new(255.0, 0.0, 0.0, 0.0),   // Bleu
                DefectType::ColorDefect => core::Scalar::new(255.0, 0.0, 255.0, 0.0), // Magenta
            };
            
            // Dessiner un cercle à la position du défaut
            let radius = (defect.size.sqrt() / 2.0) as i32;
            imgproc::circle(
                image,
                core::Point::new(defect.position.0, defect.position.1),
                radius.max(5).min(50),
                color,
                2,
                imgproc::LINE_8,
                0,
            )?;
            
            // Afficher le type et la confiance
            let text = format!(
                "{:?} ({:.0}%)",
                defect.defect_type,
                defect.confidence * 100.0
            );
            imgproc::put_text(
                image,
                &text,
                core::Point::new(defect.position.0, defect.position.1 - radius - 5),
                imgproc::FONT_HERSHEY_SIMPLEX,
                0.5,
                color,
                1,
                imgproc::LINE_8,
                false,
            )?;
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialiser le logger
    env_logger::init();
    
    println!("Heimdall Vision - Détecteur de défauts sur bouteilles");
    println!("====================================================");
    
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
    
    // Créer le détecteur de défauts
    let detector = BottleDefectDetector::new();
    
    // Créer une file d'attente partagée pour les images
    let image_queue = Arc::new(RtQueue::<Mat>::new(5));
    let image_queue_clone = image_queue.clone();
    
    // Créer un canal pour les résultats
    let result_channel = Arc::new(RtChannel::<Vec<BottleDefect>>::new(5));
    let result_channel_clone = result_channel.clone();
    
    // Créer un ordonnanceur
    let mut scheduler = RtScheduler::new();
    
    // Configurer la tâche d'acquisition
    let acquisition_config = RtConfig {
        priority: RtPriority::High,
        period_ms: 100,  // 10 Hz
        deadline_ms: 50,
        cpu_affinity: vec![0],
        lock_memory: true,
        use_rt_scheduler: true,
    };
    
    // Créer la tâche d'acquisition
    let acquisition_task = RtTask::new(
        "acquisition",
        TaskType::Periodic,
        acquisition_config,
    );
    
    // Configurer la tâche de détection
    let detection_config = RtConfig {
        priority: RtPriority::Normal,
        period_ms: 0,  // Apériodique
        deadline_ms: 80,
        cpu_affinity: vec![1],
        lock_memory: true,
        use_rt_scheduler: true,
    };
    
    // Créer la tâche de détection
    let detection_task = RtTask::new(
        "detection",
        TaskType::Aperiodic,
        detection_config,
    );
    
    // Ajouter les tâches à l'ordonnanceur
    scheduler.add_task(acquisition_task);
    scheduler.add_task(detection_task);
    
    // Démarrer l'acquisition
    println!("Démarrage de l'acquisition...");
    camera.start_acquisition().await?;
    
    // Créer une fenêtre pour afficher les résultats
    highgui::named_window("Détection de défauts", highgui::WINDOW_NORMAL)?;
    
    // Démarrer les tâches
    println!("\nDémarrage des tâches temps réel...");
    
    let camera_clone = Arc::new(tokio::sync::Mutex::new(camera));
    
    scheduler.start_all(|task_id| {
        match task_id {
            "acquisition" => {
                let queue = image_queue.clone();
                let camera = camera_clone.clone();
                
                Box::new(move || {
                    // Acquérir une image de manière asynchrone
                    let rt = tokio::runtime::Handle::current();
                    
                    rt.block_on(async {
                        let mut camera_guard = camera.lock().await;
                        match camera_guard.acquire_frame().await {
                            Ok(frame) => {
                                // Convertir en Mat OpenCV
                                if let Ok(mat) = heimdall_camera::to_opencv_mat(&frame) {
                                    // Ajouter l'image à la file d'attente
                                    match queue.push(mat) {
                                        Ok(_) => println!("Image acquise et ajoutée à la file d'attente"),
                                        Err(_) => println!("File d'attente pleine, image ignorée"),
                                    }
                                }
                            },
                            Err(e) => println!("Erreur lors de l'acquisition: {:?}", e),
                        }
                    });
                })
            },
            "detection" => {
                let queue = image_queue_clone.clone();
                let channel = result_channel_clone.clone();
                let detector = detector.clone();
                
                Box::new(move || {
                    // Vérifier s'il y a une image à traiter
                    if let Some(image) = queue.pop() {
                        println!("Traitement d'une image pour la détection de défauts");
                        
                        // Détecter les défauts
                        match detector.detect_defects(&image) {
                            Ok(defects) => {
                                println!("Détection terminée: {} défauts trouvés", defects.len());
                                
                                // Dessiner les défauts sur l'image
                                let mut result_image = image.clone();
                                if let Err(e) = detector.draw_defects(&mut result_image, &defects) {
                                    println!("Erreur lors du dessin des défauts: {:?}", e);
                                }
                                
                                // Afficher l'image
                                if let Err(e) = highgui::imshow("Détection de défauts", &result_image) {
                                    println!("Erreur lors de l'affichage: {:?}", e);
                                }
                                highgui::wait_key(1).unwrap_or(0);
                                
                                // Envoyer les résultats
                                if let Err(e) = channel.try_send(defects) {
                                    println!("Erreur lors de l'envoi des résultats: {:?}", e);
                                }
                            },
                            Err(e) => println!("Erreur lors de la détection: {:?}", e),
                        }
                    }
                })
            },
            _ => Box::new(|| {}),
        }
    }).await?;
    
    // Exécuter pendant quelques secondes
    println!("\nExécution des tâches pendant 10 secondes...");
    sleep(Duration::from_secs(10)).await;
    
    // Arrêter les tâches
    println!("\nArrêt des tâches...");
    scheduler.stop_all().await?;
    
    // Arrêter l'acquisition
    let mut camera_guard = camera_clone.lock().await;
    camera_guard.stop_acquisition().await?;
    
    // Afficher les statistiques
    println!("\nStatistiques de détection:");
    let mut total_defects = 0;
    let mut defect_types = std::collections::HashMap::new();
    
    while let Ok(defects) = result_channel.try_recv() {
        total_defects += defects.len();
        
        for defect in defects {
            *defect_types.entry(defect.defect_type).or_insert(0) += 1;
        }
    }
    
    println!("  Nombre total de défauts détectés: {}", total_defects);
    println!("  Répartition par type:");
    for (defect_type, count) in defect_types {
        println!("    {:?}: {}", defect_type, count);
    }
    
    println!("\nExemple terminé avec succès!");
    
    Ok(())
}

// Implémentation de Clone pour BottleDefectDetector
impl Clone for BottleDefectDetector {
    fn clone(&self) -> Self {
        Self {
            threshold: self.threshold,
            min_size: self.min_size,
            max_size: self.max_size,
            sensitivity: self.sensitivity,
        }
    }
}
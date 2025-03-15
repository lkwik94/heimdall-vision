use heimdall_rt::{RtConfig, RtPriority};
use heimdall_rt::scheduler::{RtScheduler, RtTask, TaskType};
use heimdall_rt::sync::{RtQueue, RtChannel, RtRwLock};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialiser le logger
    env_logger::init();
    
    println!("Heimdall Vision - Exemple de traitement temps réel");
    println!("=================================================");
    
    // Créer une file d'attente partagée pour les images
    let image_queue = Arc::new(RtQueue::<Vec<u8>>::new(10));
    let image_queue_clone = image_queue.clone();
    
    // Créer un canal pour les résultats
    let result_channel = Arc::new(RtChannel::<String>::new(10));
    let result_channel_clone = result_channel.clone();
    
    // Créer un verrou pour les statistiques partagées
    let stats = Arc::new(RtRwLock::new(Vec::<Duration>::new()));
    let stats_clone = stats.clone();
    
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
    
    // Configurer la tâche de traitement
    let processing_config = RtConfig {
        priority: RtPriority::Normal,
        period_ms: 0,  // Apériodique
        deadline_ms: 80,
        cpu_affinity: vec![1],
        lock_memory: true,
        use_rt_scheduler: true,
    };
    
    // Créer la tâche de traitement
    let processing_task = RtTask::new(
        "processing",
        TaskType::Aperiodic,
        processing_config,
    );
    
    // Ajouter les tâches à l'ordonnanceur
    scheduler.add_task(acquisition_task);
    scheduler.add_task(processing_task);
    
    // Démarrer les tâches
    println!("\nDémarrage des tâches temps réel...");
    
    scheduler.start_all(|task_id| {
        match task_id {
            "acquisition" => {
                let queue = image_queue.clone();
                let stats = stats_clone.clone();
                
                Box::new(move || {
                    // Simuler l'acquisition d'une image
                    let start = Instant::now();
                    
                    // Créer une image simulée
                    let image_size = 1280 * 720 * 3;
                    let image_data = vec![0u8; image_size];
                    
                    // Ajouter l'image à la file d'attente
                    match queue.push(image_data) {
                        Ok(_) => {
                            println!("Image acquise et ajoutée à la file d'attente");
                        },
                        Err(_) => {
                            println!("File d'attente pleine, image ignorée");
                        },
                    }
                    
                    // Enregistrer le temps d'exécution
                    let elapsed = start.elapsed();
                    let mut stats_data = stats.write();
                    stats_data.push(elapsed);
                    
                    println!("Acquisition terminée en {:?}", elapsed);
                })
            },
            "processing" => {
                let queue = image_queue_clone.clone();
                let channel = result_channel_clone.clone();
                let stats = stats_clone.clone();
                
                Box::new(move || {
                    // Vérifier s'il y a une image à traiter
                    if let Some(image_data) = queue.pop() {
                        let start = Instant::now();
                        
                        // Simuler le traitement de l'image
                        let image_size = image_data.len();
                        println!("Traitement d'une image de {} octets", image_size);
                        
                        // Simuler un traitement intensif
                        let mut sum = 0;
                        for byte in &image_data {
                            sum += *byte as u64;
                        }
                        let mean = sum as f64 / image_size as f64;
                        
                        // Envoyer le résultat
                        let result = format!("Intensité moyenne: {:.2}", mean);
                        if let Err(e) = channel.try_send(result) {
                            println!("Erreur lors de l'envoi du résultat: {:?}", e);
                        }
                        
                        // Enregistrer le temps d'exécution
                        let elapsed = start.elapsed();
                        let mut stats_data = stats.write();
                        stats_data.push(elapsed);
                        
                        println!("Traitement terminé en {:?}", elapsed);
                    }
                })
            },
            _ => Box::new(|| {}),
        }
    }).await?;
    
    // Exécuter pendant quelques secondes
    println!("\nExécution des tâches pendant 5 secondes...");
    sleep(Duration::from_secs(5)).await;
    
    // Arrêter les tâches
    println!("\nArrêt des tâches...");
    scheduler.stop_all().await?;
    
    // Afficher les statistiques
    println!("\nStatistiques d'exécution:");
    let stats_data = stats.read();
    
    if !stats_data.is_empty() {
        let mut min = stats_data[0];
        let mut max = stats_data[0];
        let mut sum = Duration::from_secs(0);
        
        for &duration in &*stats_data {
            if duration < min {
                min = duration;
            }
            if duration > max {
                max = duration;
            }
            sum += duration;
        }
        
        let avg = sum / stats_data.len() as u32;
        
        println!("  Nombre d'exécutions: {}", stats_data.len());
        println!("  Temps minimum: {:?}", min);
        println!("  Temps maximum: {:?}", max);
        println!("  Temps moyen: {:?}", avg);
    } else {
        println!("  Aucune donnée statistique disponible");
    }
    
    // Afficher les résultats
    println!("\nRésultats de traitement:");
    while let Ok(result) = result_channel.try_recv() {
        println!("  {}", result);
    }
    
    println!("\nExemple terminé avec succès!");
    
    Ok(())
}
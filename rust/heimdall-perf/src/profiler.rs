use std::fs::File;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use log::{debug, error, info, warn};
use pprof::protos::Message;
use crate::PerfError;

/// État global du profiler
static PROFILER_GUARD: Mutex<Option<pprof::ProfilerGuard<'static>>> = Mutex::new(None);

/// Initialise le profiler
pub fn init() -> Result<(), PerfError> {
    info!("Initialisation du profiler");
    
    // Créer un nouveau profiler
    let guard = pprof::ProfilerGuard::new(100)
        .map_err(|e| PerfError::InitError(format!("Erreur d'initialisation du profiler: {}", e)))?;
    
    // Stocker le profiler
    let mut profiler_guard = PROFILER_GUARD.lock().unwrap();
    *profiler_guard = Some(guard);
    
    Ok(())
}

/// Génère un rapport de profilage
pub fn generate_report() -> Result<pprof::Report, PerfError> {
    debug!("Génération d'un rapport de profilage");
    
    let profiler_guard = PROFILER_GUARD.lock().unwrap();
    
    if let Some(guard) = &*profiler_guard {
        let report = guard.report()
            .build()
            .map_err(|e| PerfError::MeasurementError(format!("Erreur de génération du rapport: {}", e)))?;
        
        Ok(report)
    } else {
        Err(PerfError::MeasurementError("Profiler non initialisé".to_string()))
    }
}

/// Génère un flamegraph
pub fn generate_flamegraph(path: &Path) -> Result<(), PerfError> {
    info!("Génération d'un flamegraph dans: {:?}", path);
    
    let report = generate_report()?;
    
    // Créer le fichier de sortie
    let file = File::create(path)
        .map_err(|e| PerfError::IoError(e))?;
    
    // Générer le flamegraph
    report.flamegraph(file)
        .map_err(|e| PerfError::MeasurementError(format!("Erreur de génération du flamegraph: {}", e)))?;
    
    Ok(())
}

/// Génère un profil protobuf
pub fn generate_proto(path: &Path) -> Result<(), PerfError> {
    info!("Génération d'un profil protobuf dans: {:?}", path);
    
    let report = generate_report()?;
    
    // Créer le fichier de sortie
    let mut file = File::create(path)
        .map_err(|e| PerfError::IoError(e))?;
    
    // Générer le profil protobuf
    let profile = report.pprof()
        .map_err(|e| PerfError::MeasurementError(format!("Erreur de génération du profil: {}", e)))?;
    
    profile.write_to_vec()
        .and_then(|bytes| file.write_all(&bytes))
        .map_err(|e| PerfError::IoError(e))?;
    
    Ok(())
}

/// Mesure le temps d'exécution d'une fonction
pub fn measure_time<F, T>(name: &str, f: F) -> (T, Duration)
where
    F: FnOnce() -> T,
{
    debug!("Mesure du temps d'exécution de: {}", name);
    
    let start = std::time::Instant::now();
    let result = f();
    let duration = start.elapsed();
    
    debug!("Temps d'exécution de {}: {:?}", name, duration);
    
    (result, duration)
}

/// Mesure le temps d'exécution d'une fonction asynchrone
pub async fn measure_time_async<F, T>(name: &str, f: F) -> (T, Duration)
where
    F: std::future::Future<Output = T>,
{
    debug!("Mesure du temps d'exécution asynchrone de: {}", name);
    
    let start = std::time::Instant::now();
    let result = f.await;
    let duration = start.elapsed();
    
    debug!("Temps d'exécution asynchrone de {}: {:?}", name, duration);
    
    (result, duration)
}

/// Mesure le débit de traitement
pub fn measure_throughput<F, T>(name: &str, count: usize, f: F) -> (T, f64)
where
    F: FnOnce() -> T,
{
    debug!("Mesure du débit de traitement de: {}", name);
    
    let start = std::time::Instant::now();
    let result = f();
    let duration = start.elapsed();
    
    let throughput = count as f64 / duration.as_secs_f64();
    
    debug!("Débit de traitement de {}: {:.2} items/s", name, throughput);
    
    (result, throughput)
}

/// Mesure le débit de traitement asynchrone
pub async fn measure_throughput_async<F, T>(name: &str, count: usize, f: F) -> (T, f64)
where
    F: std::future::Future<Output = T>,
{
    debug!("Mesure du débit de traitement asynchrone de: {}", name);
    
    let start = std::time::Instant::now();
    let result = f.await;
    let duration = start.elapsed();
    
    let throughput = count as f64 / duration.as_secs_f64();
    
    debug!("Débit de traitement asynchrone de {}: {:.2} items/s", name, throughput);
    
    (result, throughput)
}

use std::io::Write;
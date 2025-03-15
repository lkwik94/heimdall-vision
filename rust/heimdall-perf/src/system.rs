use std::time::Instant;
use log::{debug, error, info, warn};
use crate::PerfError;

/// Métriques système
pub struct SystemMetrics {
    /// Processus
    #[cfg(target_os = "linux")]
    process: procfs::process::Process,
    
    /// Statistiques CPU précédentes
    #[cfg(target_os = "linux")]
    prev_cpu_stat: Option<procfs::CpuStat>,
    
    /// Statistiques de processus précédentes
    #[cfg(target_os = "linux")]
    prev_proc_stat: Option<procfs::process::Stat>,
    
    /// Heure de la dernière collecte
    last_collect: Instant,
}

impl SystemMetrics {
    /// Crée une nouvelle instance de métriques système
    pub fn new() -> Result<Self, PerfError> {
        #[cfg(target_os = "linux")]
        {
            // Obtenir le processus actuel
            let pid = std::process::id();
            let process = procfs::process::Process::new(pid as i32)
                .map_err(|e| PerfError::SystemError(format!("Erreur d'accès au processus: {}", e)))?;
            
            Ok(Self {
                process,
                prev_cpu_stat: None,
                prev_proc_stat: None,
                last_collect: Instant::now(),
            })
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self {
                last_collect: Instant::now(),
            })
        }
    }
    
    /// Collecte les métriques système
    pub fn collect(&mut self) -> Result<(), PerfError> {
        #[cfg(target_os = "linux")]
        {
            // Obtenir les statistiques CPU
            let cpu_stat = procfs::CpuStat::new()
                .map_err(|e| PerfError::SystemError(format!("Erreur d'accès aux statistiques CPU: {}", e)))?;
            
            // Obtenir les statistiques de processus
            let proc_stat = self.process.stat()
                .map_err(|e| PerfError::SystemError(format!("Erreur d'accès aux statistiques de processus: {}", e)))?;
            
            // Mettre à jour les statistiques précédentes
            self.prev_cpu_stat = Some(cpu_stat);
            self.prev_proc_stat = Some(proc_stat);
        }
        
        // Mettre à jour l'heure de la dernière collecte
        self.last_collect = Instant::now();
        
        Ok(())
    }
    
    /// Obtient l'utilisation CPU
    pub fn cpu_usage(&self) -> Result<f64, PerfError> {
        #[cfg(target_os = "linux")]
        {
            if let (Some(prev_cpu), Some(prev_proc)) = (&self.prev_cpu_stat, &self.prev_proc_stat) {
                // Obtenir les statistiques actuelles
                let cpu_stat = procfs::CpuStat::new()
                    .map_err(|e| PerfError::SystemError(format!("Erreur d'accès aux statistiques CPU: {}", e)))?;
                
                let proc_stat = self.process.stat()
                    .map_err(|e| PerfError::SystemError(format!("Erreur d'accès aux statistiques de processus: {}", e)))?;
                
                // Calculer l'utilisation CPU
                let cpu_time_delta = (cpu_stat.user - prev_cpu.user) + (cpu_stat.nice - prev_cpu.nice) +
                                    (cpu_stat.system - prev_cpu.system) + (cpu_stat.idle - prev_cpu.idle) +
                                    (cpu_stat.iowait.unwrap_or(0) - prev_cpu.iowait.unwrap_or(0)) +
                                    (cpu_stat.irq.unwrap_or(0) - prev_cpu.irq.unwrap_or(0)) +
                                    (cpu_stat.softirq.unwrap_or(0) - prev_cpu.softirq.unwrap_or(0)) +
                                    (cpu_stat.steal.unwrap_or(0) - prev_cpu.steal.unwrap_or(0)) +
                                    (cpu_stat.guest.unwrap_or(0) - prev_cpu.guest.unwrap_or(0)) +
                                    (cpu_stat.guest_nice.unwrap_or(0) - prev_cpu.guest_nice.unwrap_or(0));
                
                let proc_time_delta = (proc_stat.utime - prev_proc.utime) + (proc_stat.stime - prev_proc.stime);
                
                if cpu_time_delta > 0 {
                    let usage = (proc_time_delta as f64 / cpu_time_delta as f64) * 100.0 * num_cpus::get() as f64;
                    return Ok(usage);
                }
            }
            
            Err(PerfError::MeasurementError("Données insuffisantes pour calculer l'utilisation CPU".to_string()))
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Err(PerfError::SystemError("Mesure de l'utilisation CPU non supportée sur cette plateforme".to_string()))
        }
    }
    
    /// Obtient l'utilisation mémoire
    pub fn memory_usage(&self) -> Result<u64, PerfError> {
        #[cfg(target_os = "linux")]
        {
            let statm = self.process.statm()
                .map_err(|e| PerfError::SystemError(format!("Erreur d'accès aux statistiques mémoire: {}", e)))?;
            
            // Convertir en octets (statm.resident est en pages)
            let page_size = procfs::page_size()
                .map_err(|e| PerfError::SystemError(format!("Erreur d'obtention de la taille de page: {}", e)))?;
            
            Ok(statm.resident as u64 * page_size as u64)
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Err(PerfError::SystemError("Mesure de l'utilisation mémoire non supportée sur cette plateforme".to_string()))
        }
    }
    
    /// Obtient le nombre de threads
    pub fn thread_count(&self) -> Result<u64, PerfError> {
        #[cfg(target_os = "linux")]
        {
            let stat = self.process.stat()
                .map_err(|e| PerfError::SystemError(format!("Erreur d'accès aux statistiques de processus: {}", e)))?;
            
            Ok(stat.num_threads as u64)
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Err(PerfError::SystemError("Mesure du nombre de threads non supportée sur cette plateforme".to_string()))
        }
    }
    
    /// Obtient le temps écoulé depuis la dernière collecte
    pub fn time_since_last_collect(&self) -> std::time::Duration {
        self.last_collect.elapsed()
    }
}
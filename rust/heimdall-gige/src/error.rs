//! Gestion des erreurs pour le module GigE Vision
//!
//! Ce module définit les types d'erreurs spécifiques au module GigE Vision
//! et les stratégies de reprise associées.

use std::fmt;
use std::io;
use std::time::Duration;
use thiserror::Error;

/// Erreur du module GigE Vision
#[derive(Error, Debug)]
pub enum GigEError {
    /// Erreur d'initialisation
    #[error("Erreur d'initialisation: {0}")]
    InitError(String),
    
    /// Erreur de configuration
    #[error("Erreur de configuration: {0}")]
    ConfigError(String),
    
    /// Erreur d'acquisition
    #[error("Erreur d'acquisition: {0}")]
    AcquisitionError(String),
    
    /// Erreur de synchronisation
    #[error("Erreur de synchronisation: {0}")]
    SyncError(String),
    
    /// Erreur réseau
    #[error("Erreur réseau: {0}")]
    NetworkError(String),
    
    /// Erreur de périphérique
    #[error("Erreur de périphérique: {0}")]
    DeviceError(String),
    
    /// Erreur de timeout
    #[error("Timeout: {0}")]
    TimeoutError(String),
    
    /// Erreur de buffer
    #[error("Erreur de buffer: {0}")]
    BufferError(String),
    
    /// Erreur de conversion
    #[error("Erreur de conversion: {0}")]
    ConversionError(String),
    
    /// Erreur d'Aravis
    #[error("Erreur d'Aravis: {0}")]
    AravisError(String),
    
    /// Erreur d'entrée/sortie
    #[error("Erreur d'E/S: {0}")]
    IoError(#[from] io::Error),
    
    /// Erreur de sérialisation/désérialisation
    #[error("Erreur de sérialisation: {0}")]
    SerdeError(#[from] serde_json::Error),
    
    /// Erreur générique
    #[error("Erreur: {0}")]
    Other(String),
}

impl From<heimdall_camera::CameraError> for GigEError {
    fn from(err: heimdall_camera::CameraError) -> Self {
        match err {
            heimdall_camera::CameraError::InitError(msg) => GigEError::InitError(msg),
            heimdall_camera::CameraError::ConfigError(msg) => GigEError::ConfigError(msg),
            heimdall_camera::CameraError::AcquisitionError(msg) => GigEError::AcquisitionError(msg),
            heimdall_camera::CameraError::NotFound(msg) => GigEError::DeviceError(msg),
            heimdall_camera::CameraError::ConversionError(msg) => GigEError::ConversionError(msg),
            heimdall_camera::CameraError::AravisError(msg) => GigEError::AravisError(msg),
        }
    }
}

impl From<anyhow::Error> for GigEError {
    fn from(err: anyhow::Error) -> Self {
        GigEError::Other(err.to_string())
    }
}

/// Catégorie d'erreur
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Erreur temporaire qui peut être résolue par une nouvelle tentative
    Transient,
    
    /// Erreur permanente qui nécessite une intervention
    Permanent,
    
    /// Erreur fatale qui nécessite un redémarrage du système
    Fatal,
}

/// Stratégie de reprise
#[derive(Debug, Clone)]
pub struct RecoveryStrategy {
    /// Catégorie d'erreur
    pub category: ErrorCategory,
    
    /// Nombre de tentatives
    pub retry_count: u32,
    
    /// Délai entre les tentatives
    pub retry_delay: Duration,
    
    /// Action de reprise
    pub action: RecoveryAction,
}

/// Action de reprise
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    /// Réessayer l'opération
    Retry,
    
    /// Réinitialiser le périphérique
    ResetDevice,
    
    /// Réinitialiser la connexion
    ResetConnection,
    
    /// Redémarrer le système
    RestartSystem,
    
    /// Action personnalisée
    Custom(String),
}

impl fmt::Display for RecoveryAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoveryAction::Retry => write!(f, "Réessayer l'opération"),
            RecoveryAction::ResetDevice => write!(f, "Réinitialiser le périphérique"),
            RecoveryAction::ResetConnection => write!(f, "Réinitialiser la connexion"),
            RecoveryAction::RestartSystem => write!(f, "Redémarrer le système"),
            RecoveryAction::Custom(action) => write!(f, "Action personnalisée: {}", action),
        }
    }
}

/// Détermine la stratégie de reprise pour une erreur donnée
pub fn determine_recovery_strategy(error: &GigEError) -> RecoveryStrategy {
    match error {
        GigEError::NetworkError(_) => RecoveryStrategy {
            category: ErrorCategory::Transient,
            retry_count: 5,
            retry_delay: Duration::from_millis(100),
            action: RecoveryAction::Retry,
        },
        
        GigEError::TimeoutError(_) => RecoveryStrategy {
            category: ErrorCategory::Transient,
            retry_count: 3,
            retry_delay: Duration::from_millis(200),
            action: RecoveryAction::Retry,
        },
        
        GigEError::AcquisitionError(_) => RecoveryStrategy {
            category: ErrorCategory::Transient,
            retry_count: 3,
            retry_delay: Duration::from_millis(50),
            action: RecoveryAction::Retry,
        },
        
        GigEError::DeviceError(_) => RecoveryStrategy {
            category: ErrorCategory::Permanent,
            retry_count: 1,
            retry_delay: Duration::from_secs(1),
            action: RecoveryAction::ResetDevice,
        },
        
        GigEError::BufferError(_) => RecoveryStrategy {
            category: ErrorCategory::Transient,
            retry_count: 2,
            retry_delay: Duration::from_millis(50),
            action: RecoveryAction::Retry,
        },
        
        GigEError::AravisError(_) => RecoveryStrategy {
            category: ErrorCategory::Permanent,
            retry_count: 1,
            retry_delay: Duration::from_secs(1),
            action: RecoveryAction::ResetConnection,
        },
        
        GigEError::InitError(_) | GigEError::ConfigError(_) => RecoveryStrategy {
            category: ErrorCategory::Permanent,
            retry_count: 0,
            retry_delay: Duration::from_secs(0),
            action: RecoveryAction::RestartSystem,
        },
        
        _ => RecoveryStrategy {
            category: ErrorCategory::Permanent,
            retry_count: 1,
            retry_delay: Duration::from_secs(1),
            action: RecoveryAction::Custom("Vérifier la configuration et les connexions".to_string()),
        },
    }
}

/// Exécute une opération avec une stratégie de reprise
pub async fn with_recovery<F, T, E>(
    operation: F,
    error_mapper: impl Fn(E) -> GigEError,
) -> Result<T, GigEError>
where
    F: Fn() -> impl std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut last_error = None;
    
    for attempt in 1..=3 {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                let gige_error = error_mapper(err);
                let strategy = determine_recovery_strategy(&gige_error);
                
                if attempt <= strategy.retry_count as usize {
                    log::warn!(
                        "Erreur lors de l'opération (tentative {}/{}): {}. Stratégie: {}. Nouvelle tentative dans {:?}.",
                        attempt,
                        strategy.retry_count + 1,
                        gige_error,
                        strategy.action,
                        strategy.retry_delay
                    );
                    
                    tokio::time::sleep(strategy.retry_delay).await;
                    last_error = Some(gige_error);
                } else {
                    return Err(gige_error);
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| GigEError::Other("Erreur inconnue".to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_categories() {
        let network_error = GigEError::NetworkError("Connexion perdue".to_string());
        let strategy = determine_recovery_strategy(&network_error);
        assert_eq!(strategy.category, ErrorCategory::Transient);
        
        let init_error = GigEError::InitError("Échec d'initialisation".to_string());
        let strategy = determine_recovery_strategy(&init_error);
        assert_eq!(strategy.category, ErrorCategory::Permanent);
    }
    
    #[tokio::test]
    async fn test_with_recovery_success() {
        let result = with_recovery(
            || async { Ok::<_, &str>(42) },
            |e| GigEError::Other(e.to_string()),
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
    
    #[tokio::test]
    async fn test_with_recovery_failure() {
        let mut attempts = 0;
        
        let result = with_recovery(
            || async {
                attempts += 1;
                Err::<i32, _>("Erreur de test")
            },
            |e| GigEError::NetworkError(e.to_string()),
        ).await;
        
        assert!(result.is_err());
        assert!(attempts > 1); // Devrait avoir fait plusieurs tentatives
    }
}
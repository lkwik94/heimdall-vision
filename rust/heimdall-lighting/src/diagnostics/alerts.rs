use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use log::{debug, error, info, warn};
use tokio::time;
use chrono::{DateTime, Utc};

use crate::diagnostics::monitoring::MonitoringMeasurement;
use crate::{DiagnosticStatus, LightingError};

/// Niveau d'alerte
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertLevel {
    /// Information
    Info,
    
    /// Avertissement
    Warning,
    
    /// Erreur
    Error,
    
    /// Critique
    Critical,
}

/// Alerte
#[derive(Debug, Clone)]
pub struct Alert {
    /// Identifiant
    pub id: String,
    
    /// Horodatage
    pub timestamp: DateTime<Utc>,
    
    /// Niveau
    pub level: AlertLevel,
    
    /// Message
    pub message: String,
    
    /// Source
    pub source: String,
    
    /// Données supplémentaires
    pub data: HashMap<String, String>,
    
    /// Acquittée
    pub acknowledged: bool,
}

/// Gestionnaire d'alertes
pub struct AlertManager {
    /// Alertes actives
    alerts: Vec<Alert>,
    
    /// Historique des alertes
    history: Vec<Alert>,
    
    /// Callbacks de notification
    notification_callbacks: Vec<Box<dyn Fn(&Alert) + Send + Sync>>,
    
    /// Taille maximale de l'historique
    max_history_size: usize,
}

impl AlertManager {
    /// Crée un nouveau gestionnaire d'alertes
    pub fn new(max_history_size: usize) -> Self {
        Self {
            alerts: Vec::new(),
            history: Vec::new(),
            notification_callbacks: Vec::new(),
            max_history_size,
        }
    }
    
    /// Ajoute une alerte
    pub fn add_alert(&mut self, alert: Alert) {
        // Vérifier si l'alerte existe déjà
        if let Some(existing) = self.alerts.iter_mut().find(|a| a.id == alert.id) {
            // Mettre à jour l'alerte existante
            existing.timestamp = alert.timestamp;
            existing.level = alert.level;
            existing.message = alert.message;
            existing.data = alert.data;
            existing.acknowledged = false;
        } else {
            // Ajouter une nouvelle alerte
            self.alerts.push(alert.clone());
        }
        
        // Notifier les callbacks
        for callback in &self.notification_callbacks {
            callback(&alert);
        }
    }
    
    /// Acquitte une alerte
    pub fn acknowledge_alert(&mut self, id: &str) -> Result<(), LightingError> {
        if let Some(alert) = self.alerts.iter_mut().find(|a| a.id == id) {
            alert.acknowledged = true;
            
            // Déplacer l'alerte vers l'historique
            let alert = alert.clone();
            self.alerts.retain(|a| a.id != id);
            self.history.push(alert);
            
            // Limiter la taille de l'historique
            if self.history.len() > self.max_history_size {
                self.history.remove(0);
            }
            
            Ok(())
        } else {
            Err(LightingError::ConfigError(format!("Alerte non trouvée: {}", id)))
        }
    }
    
    /// Supprime une alerte
    pub fn remove_alert(&mut self, id: &str) -> Result<(), LightingError> {
        if self.alerts.iter().any(|a| a.id == id) {
            self.alerts.retain(|a| a.id != id);
            Ok(())
        } else {
            Err(LightingError::ConfigError(format!("Alerte non trouvée: {}", id)))
        }
    }
    
    /// Obtient les alertes actives
    pub fn get_active_alerts(&self) -> &[Alert] {
        &self.alerts
    }
    
    /// Obtient l'historique des alertes
    pub fn get_alert_history(&self) -> &[Alert] {
        &self.history
    }
    
    /// Ajoute un callback de notification
    pub fn add_notification_callback<F>(&mut self, callback: F)
    where
        F: Fn(&Alert) + Send + Sync + 'static
    {
        self.notification_callbacks.push(Box::new(callback));
    }
    
    /// Crée une alerte à partir d'une mesure de surveillance
    pub fn create_alert_from_measurement(
        &mut self,
        measurement: &MonitoringMeasurement,
        source: &str
    ) {
        let level = match measurement.status {
            DiagnosticStatus::Ok => return,  // Pas d'alerte
            DiagnosticStatus::Warning => AlertLevel::Warning,
            DiagnosticStatus::Error => AlertLevel::Error,
        };
        
        // Créer un message d'alerte
        let mut message = String::new();
        let mut data = HashMap::new();
        
        // Vérifier l'intensité
        if measurement.mean_intensity < 80.0 {
            message.push_str(&format!(
                "Intensité faible: {:.1}%. ",
                measurement.mean_intensity
            ));
            data.insert("intensity".to_string(), measurement.mean_intensity.to_string());
        }
        
        // Vérifier l'uniformité
        if measurement.uniformity < 80.0 {
            message.push_str(&format!(
                "Uniformité faible: {:.1}%. ",
                measurement.uniformity
            ));
            data.insert("uniformity".to_string(), measurement.uniformity.to_string());
        }
        
        // Vérifier la durée d'utilisation
        if measurement.usage_hours > 5000.0 {
            message.push_str(&format!(
                "Durée d'utilisation élevée: {:.1} heures. ",
                measurement.usage_hours
            ));
            data.insert("usage_hours".to_string(), measurement.usage_hours.to_string());
        }
        
        // Vérifier la température
        if let Some(temp) = measurement.temperature {
            if temp > 60.0 {
                message.push_str(&format!(
                    "Température élevée: {:.1}°C. ",
                    temp
                ));
                data.insert("temperature".to_string(), temp.to_string());
            }
        }
        
        // Si aucun message n'a été généré, ne pas créer d'alerte
        if message.is_empty() {
            return;
        }
        
        // Créer l'alerte
        let alert = Alert {
            id: format!("{}_{}", source, measurement.timestamp.timestamp()),
            timestamp: measurement.timestamp,
            level,
            message,
            source: source.to_string(),
            data,
            acknowledged: false,
        };
        
        // Ajouter l'alerte
        self.add_alert(alert);
    }
}

/// Notificateur d'alertes par e-mail
pub struct EmailNotifier {
    /// Adresse e-mail de destination
    recipient: String,
    
    /// Serveur SMTP
    smtp_server: String,
    
    /// Port SMTP
    smtp_port: u16,
    
    /// Nom d'utilisateur SMTP
    smtp_username: String,
    
    /// Mot de passe SMTP
    smtp_password: String,
    
    /// Adresse e-mail d'expédition
    from_address: String,
}

impl EmailNotifier {
    /// Crée un nouveau notificateur par e-mail
    pub fn new(
        recipient: &str,
        smtp_server: &str,
        smtp_port: u16,
        smtp_username: &str,
        smtp_password: &str,
        from_address: &str
    ) -> Self {
        Self {
            recipient: recipient.to_string(),
            smtp_server: smtp_server.to_string(),
            smtp_port,
            smtp_username: smtp_username.to_string(),
            smtp_password: smtp_password.to_string(),
            from_address: from_address.to_string(),
        }
    }
    
    /// Envoie une notification par e-mail
    pub fn send_notification(&self, alert: &Alert) -> Result<(), LightingError> {
        // Dans un système réel, cette fonction enverrait un e-mail
        // Pour cette démonstration, nous simulons l'envoi d'un e-mail
        
        info!("Envoi d'une notification par e-mail:");
        info!("  À: {}", self.recipient);
        info!("  De: {}", self.from_address);
        info!("  Sujet: Alerte d'éclairage: {}", alert.level);
        info!("  Message: {}", alert.message);
        
        Ok(())
    }
}

/// Notificateur d'alertes par webhook
pub struct WebhookNotifier {
    /// URL du webhook
    url: String,
    
    /// En-têtes HTTP
    headers: HashMap<String, String>,
}

impl WebhookNotifier {
    /// Crée un nouveau notificateur par webhook
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            headers: HashMap::new(),
        }
    }
    
    /// Ajoute un en-tête HTTP
    pub fn add_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }
    
    /// Envoie une notification par webhook
    pub async fn send_notification(&self, alert: &Alert) -> Result<(), LightingError> {
        // Dans un système réel, cette fonction enverrait une requête HTTP
        // Pour cette démonstration, nous simulons l'envoi d'une requête
        
        info!("Envoi d'une notification par webhook:");
        info!("  URL: {}", self.url);
        info!("  En-têtes: {:?}", self.headers);
        info!("  Payload: {{ \"level\": \"{:?}\", \"message\": \"{}\" }}", alert.level, alert.message);
        
        Ok(())
    }
}
use std::collections::HashMap;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};
use crate::{Measurement, MetricType, PerfError};

/// Format de rapport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// Format JSON
    Json,
    
    /// Format texte
    Text,
    
    /// Format Markdown
    Markdown,
    
    /// Format HTML
    Html,
    
    /// Format CSV
    Csv,
}

/// Statistiques de mesure
#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
pub struct MetricStats {
    /// Nom de la métrique
    #[tabled(rename = "Métrique")]
    pub name: String,
    
    /// Type de métrique
    #[tabled(rename = "Type")]
    pub metric_type: String,
    
    /// Nombre de mesures
    #[tabled(rename = "Nombre")]
    pub count: usize,
    
    /// Valeur minimale
    #[tabled(rename = "Minimum")]
    pub min: f64,
    
    /// Valeur maximale
    #[tabled(rename = "Maximum")]
    pub max: f64,
    
    /// Valeur moyenne
    #[tabled(rename = "Moyenne")]
    pub avg: f64,
    
    /// Écart-type
    #[tabled(rename = "Écart-type")]
    pub std_dev: f64,
    
    /// Unité de mesure
    #[tabled(rename = "Unité")]
    pub unit: String,
}

/// Rapport de performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Nom du rapport
    pub name: String,
    
    /// Horodatage
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Mesures
    pub measurements: Vec<Measurement>,
    
    /// Statistiques par métrique
    pub stats: Vec<MetricStats>,
}

impl Report {
    /// Crée un nouveau rapport
    pub fn new(name: &str, measurements: &[Measurement]) -> Self {
        let stats = Self::compute_stats(measurements);
        
        Self {
            name: name.to_string(),
            timestamp: chrono::Utc::now(),
            measurements: measurements.to_vec(),
            stats,
        }
    }
    
    /// Calcule les statistiques des mesures
    fn compute_stats(measurements: &[Measurement]) -> Vec<MetricStats> {
        // Regrouper les mesures par nom et type
        let mut groups: HashMap<(String, MetricType), Vec<&Measurement>> = HashMap::new();
        
        for measurement in measurements {
            let key = (measurement.name.clone(), measurement.metric_type);
            groups.entry(key).or_default().push(measurement);
        }
        
        // Calculer les statistiques pour chaque groupe
        let mut stats = Vec::new();
        
        for ((name, metric_type), group) in groups {
            if group.is_empty() {
                continue;
            }
            
            // Calculer les statistiques
            let count = group.len();
            let values: Vec<f64> = group.iter().map(|m| m.value).collect();
            let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            let sum: f64 = values.iter().sum();
            let avg = sum / count as f64;
            
            // Calculer l'écart-type
            let variance = values.iter()
                .map(|v| (*v - avg).powi(2))
                .sum::<f64>() / count as f64;
            let std_dev = variance.sqrt();
            
            // Obtenir l'unité
            let unit = group[0].unit.clone();
            
            stats.push(MetricStats {
                name,
                metric_type: format!("{:?}", metric_type),
                count,
                min,
                max,
                avg,
                std_dev,
                unit,
            });
        }
        
        stats
    }
    
    /// Génère un rapport au format spécifié
    pub fn generate(&self, format: ReportFormat) -> Result<String, PerfError> {
        match format {
            ReportFormat::Json => self.to_json(),
            ReportFormat::Text => self.to_text(),
            ReportFormat::Markdown => self.to_markdown(),
            ReportFormat::Html => self.to_html(),
            ReportFormat::Csv => self.to_csv(),
        }
    }
    
    /// Convertit le rapport en JSON
    fn to_json(&self) -> Result<String, PerfError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| PerfError::SerializationError(e))
    }
    
    /// Convertit le rapport en texte
    fn to_text(&self) -> Result<String, PerfError> {
        let mut output = String::new();
        
        // En-tête
        output.push_str(&format!("Rapport de performance: {}\n", self.name));
        output.push_str(&format!("Horodatage: {}\n\n", self.timestamp));
        
        // Statistiques
        output.push_str("Statistiques:\n");
        let table = Table::new(&self.stats).to_string();
        output.push_str(&table);
        output.push('\n');
        
        Ok(output)
    }
    
    /// Convertit le rapport en Markdown
    fn to_markdown(&self) -> Result<String, PerfError> {
        let mut output = String::new();
        
        // En-tête
        output.push_str(&format!("# Rapport de performance: {}\n\n", self.name));
        output.push_str(&format!("**Horodatage:** {}\n\n", self.timestamp));
        
        // Statistiques
        output.push_str("## Statistiques\n\n");
        
        // Créer un tableau Markdown
        output.push_str("| Métrique | Type | Nombre | Minimum | Maximum | Moyenne | Écart-type | Unité |\n");
        output.push_str("|----------|------|--------|---------|---------|---------|------------|-------|\n");
        
        for stat in &self.stats {
            output.push_str(&format!(
                "| {} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {} |\n",
                stat.name, stat.metric_type, stat.count, stat.min, stat.max, stat.avg, stat.std_dev, stat.unit
            ));
        }
        
        output.push('\n');
        
        Ok(output)
    }
    
    /// Convertit le rapport en HTML
    fn to_html(&self) -> Result<String, PerfError> {
        let mut output = String::new();
        
        // En-tête HTML
        output.push_str("<!DOCTYPE html>\n");
        output.push_str("<html>\n");
        output.push_str("<head>\n");
        output.push_str("  <title>Rapport de performance</title>\n");
        output.push_str("  <style>\n");
        output.push_str("    body { font-family: Arial, sans-serif; margin: 20px; }\n");
        output.push_str("    h1 { color: #333; }\n");
        output.push_str("    table { border-collapse: collapse; width: 100%; }\n");
        output.push_str("    th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n");
        output.push_str("    th { background-color: #f2f2f2; }\n");
        output.push_str("    tr:nth-child(even) { background-color: #f9f9f9; }\n");
        output.push_str("  </style>\n");
        output.push_str("</head>\n");
        output.push_str("<body>\n");
        
        // En-tête du rapport
        output.push_str(&format!("  <h1>Rapport de performance: {}</h1>\n", self.name));
        output.push_str(&format!("  <p><strong>Horodatage:</strong> {}</p>\n", self.timestamp));
        
        // Statistiques
        output.push_str("  <h2>Statistiques</h2>\n");
        output.push_str("  <table>\n");
        output.push_str("    <tr>\n");
        output.push_str("      <th>Métrique</th>\n");
        output.push_str("      <th>Type</th>\n");
        output.push_str("      <th>Nombre</th>\n");
        output.push_str("      <th>Minimum</th>\n");
        output.push_str("      <th>Maximum</th>\n");
        output.push_str("      <th>Moyenne</th>\n");
        output.push_str("      <th>Écart-type</th>\n");
        output.push_str("      <th>Unité</th>\n");
        output.push_str("    </tr>\n");
        
        for stat in &self.stats {
            output.push_str("    <tr>\n");
            output.push_str(&format!("      <td>{}</td>\n", stat.name));
            output.push_str(&format!("      <td>{}</td>\n", stat.metric_type));
            output.push_str(&format!("      <td>{}</td>\n", stat.count));
            output.push_str(&format!("      <td>{:.2}</td>\n", stat.min));
            output.push_str(&format!("      <td>{:.2}</td>\n", stat.max));
            output.push_str(&format!("      <td>{:.2}</td>\n", stat.avg));
            output.push_str(&format!("      <td>{:.2}</td>\n", stat.std_dev));
            output.push_str(&format!("      <td>{}</td>\n", stat.unit));
            output.push_str("    </tr>\n");
        }
        
        output.push_str("  </table>\n");
        
        // Pied de page HTML
        output.push_str("</body>\n");
        output.push_str("</html>\n");
        
        Ok(output)
    }
    
    /// Convertit le rapport en CSV
    fn to_csv(&self) -> Result<String, PerfError> {
        let mut output = String::new();
        
        // En-tête CSV
        output.push_str("Métrique,Type,Nombre,Minimum,Maximum,Moyenne,Écart-type,Unité\n");
        
        // Données
        for stat in &self.stats {
            output.push_str(&format!(
                "{},{},{},{:.2},{:.2},{:.2},{:.2},{}\n",
                stat.name, stat.metric_type, stat.count, stat.min, stat.max, stat.avg, stat.std_dev, stat.unit
            ));
        }
        
        Ok(output)
    }
}
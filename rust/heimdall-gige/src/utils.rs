//! Utilitaires pour le module GigE Vision
//!
//! Ce module fournit des fonctions utilitaires pour le module GigE Vision.

use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};

use anyhow::Result;
use log::{debug, info, warn};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Vérifie la connectivité réseau vers une adresse IP
pub async fn check_connectivity(ip: IpAddr, port: u16, timeout_ms: u64) -> bool {
    let addr = format!("{}:{}", ip, port);
    
    match timeout(Duration::from_millis(timeout_ms), TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

/// Vérifie la connectivité réseau vers plusieurs adresses IP
pub async fn check_multiple_connectivity(ips: &[(IpAddr, u16)], timeout_ms: u64) -> Vec<(IpAddr, bool)> {
    let mut results = Vec::new();
    
    for &(ip, port) in ips {
        let connected = check_connectivity(ip, port, timeout_ms).await;
        results.push((ip, connected));
    }
    
    results
}

/// Détecte la MTU du réseau
pub async fn detect_network_mtu() -> Result<u32> {
    // Cette fonction simule la détection de la MTU
    // En production, elle utiliserait des commandes système ou des API réseau
    
    // Simuler une MTU typique pour Jumbo Frames
    Ok(9000)
}

/// Mesure la bande passante disponible
pub async fn measure_bandwidth(ip: IpAddr, port: u16, duration_ms: u64) -> Result<f64> {
    // Cette fonction simule la mesure de bande passante
    // En production, elle enverrait des données et mesurerait le débit
    
    let start = Instant::now();
    let addr = format!("{}:{}", ip, port);
    
    // Tenter de se connecter
    let stream = match timeout(Duration::from_millis(1000), TcpStream::connect(&addr)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            warn!("Erreur de connexion à {}: {}", addr, e);
            return Ok(0.0);
        },
        Err(_) => {
            warn!("Timeout lors de la connexion à {}", addr);
            return Ok(0.0);
        },
    };
    
    // Simuler un test de bande passante
    tokio::time::sleep(Duration::from_millis(duration_ms)).await;
    
    // Simuler une bande passante de 1 Gbps avec une variation aléatoire
    let bandwidth = 1000.0 + (rand::random::<f64>() * 200.0 - 100.0);
    
    debug!("Bande passante mesurée vers {}: {:.1} Mbps", addr, bandwidth);
    
    drop(stream);
    
    Ok(bandwidth)
}

/// Convertit une adresse MAC en chaîne de caractères
pub fn mac_to_string(mac: &[u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// Convertit une chaîne de caractères en adresse MAC
pub fn string_to_mac(s: &str) -> Result<[u8; 6], &'static str> {
    let parts: Vec<&str> = s.split(':').collect();
    
    if parts.len() != 6 {
        return Err("Format d'adresse MAC invalide");
    }
    
    let mut mac = [0u8; 6];
    
    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16).map_err(|_| "Caractère hexadécimal invalide")?;
    }
    
    Ok(mac)
}

/// Convertit une adresse IP en entier 32 bits
pub fn ip_to_u32(ip: Ipv4Addr) -> u32 {
    u32::from(ip)
}

/// Convertit un entier 32 bits en adresse IP
pub fn u32_to_ip(ip: u32) -> Ipv4Addr {
    Ipv4Addr::from(ip)
}

/// Calcule le masque de sous-réseau à partir du préfixe CIDR
pub fn cidr_to_mask(cidr: u8) -> Ipv4Addr {
    if cidr > 32 {
        return Ipv4Addr::new(0, 0, 0, 0);
    }
    
    let mask = if cidr == 0 {
        0
    } else {
        !0 << (32 - cidr)
    };
    
    Ipv4Addr::from(mask)
}

/// Calcule l'adresse de diffusion à partir de l'adresse IP et du masque
pub fn broadcast_address(ip: Ipv4Addr, mask: Ipv4Addr) -> Ipv4Addr {
    let ip_bits = u32::from(ip);
    let mask_bits = u32::from(mask);
    let broadcast = ip_bits | !mask_bits;
    
    Ipv4Addr::from(broadcast)
}

/// Calcule l'adresse réseau à partir de l'adresse IP et du masque
pub fn network_address(ip: Ipv4Addr, mask: Ipv4Addr) -> Ipv4Addr {
    let ip_bits = u32::from(ip);
    let mask_bits = u32::from(mask);
    let network = ip_bits & mask_bits;
    
    Ipv4Addr::from(network)
}

/// Vérifie si une adresse IP est dans un sous-réseau
pub fn is_in_subnet(ip: Ipv4Addr, network: Ipv4Addr, mask: Ipv4Addr) -> bool {
    let ip_bits = u32::from(ip);
    let network_bits = u32::from(network);
    let mask_bits = u32::from(mask);
    
    (ip_bits & mask_bits) == network_bits
}

/// Formate une taille en octets en chaîne lisible
pub fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}

/// Formate une durée en chaîne lisible
pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    let millis = duration.subsec_millis();
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else if seconds > 0 {
        format!("{}s {}ms", seconds, millis)
    } else {
        format!("{}ms", millis)
    }
}

/// Calcule le débit en Mo/s à partir d'une taille et d'une durée
pub fn calculate_throughput(size: u64, duration: Duration) -> f64 {
    let bytes_per_second = size as f64 / duration.as_secs_f64();
    bytes_per_second / 1_000_000.0 // Convertir en Mo/s
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mac_conversion() {
        let mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let mac_str = mac_to_string(&mac);
        
        assert_eq!(mac_str, "00:11:22:33:44:55");
        
        let mac2 = string_to_mac(&mac_str).unwrap();
        assert_eq!(mac, mac2);
    }
    
    #[test]
    fn test_ip_conversion() {
        let ip = Ipv4Addr::new(192, 168, 1, 100);
        let ip_int = ip_to_u32(ip);
        
        let ip2 = u32_to_ip(ip_int);
        assert_eq!(ip, ip2);
    }
    
    #[test]
    fn test_cidr_to_mask() {
        assert_eq!(cidr_to_mask(24), Ipv4Addr::new(255, 255, 255, 0));
        assert_eq!(cidr_to_mask(16), Ipv4Addr::new(255, 255, 0, 0));
        assert_eq!(cidr_to_mask(8), Ipv4Addr::new(255, 0, 0, 0));
        assert_eq!(cidr_to_mask(0), Ipv4Addr::new(0, 0, 0, 0));
    }
    
    #[test]
    fn test_network_and_broadcast() {
        let ip = Ipv4Addr::new(192, 168, 1, 100);
        let mask = Ipv4Addr::new(255, 255, 255, 0);
        
        let network = network_address(ip, mask);
        assert_eq!(network, Ipv4Addr::new(192, 168, 1, 0));
        
        let broadcast = broadcast_address(ip, mask);
        assert_eq!(broadcast, Ipv4Addr::new(192, 168, 1, 255));
    }
    
    #[test]
    fn test_is_in_subnet() {
        let ip1 = Ipv4Addr::new(192, 168, 1, 100);
        let ip2 = Ipv4Addr::new(192, 168, 2, 100);
        let network = Ipv4Addr::new(192, 168, 1, 0);
        let mask = Ipv4Addr::new(255, 255, 255, 0);
        
        assert!(is_in_subnet(ip1, network, mask));
        assert!(!is_in_subnet(ip2, network, mask));
    }
    
    #[test]
    fn test_format_size() {
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_secs(5)), "5s 0ms");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m 5s");
    }
    
    #[test]
    fn test_calculate_throughput() {
        let size = 10 * 1024 * 1024; // 10 MiB
        let duration = Duration::from_secs(2);
        
        let throughput = calculate_throughput(size, duration);
        assert!((throughput - 5.0).abs() < 0.01);
    }
}
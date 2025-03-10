# heimdall/core/config.py
import os
import json
import logging
from typing import Dict, List, Any, Optional, Union
import yaml

logger = logging.getLogger("heimdall.config")

class Config:
    """Configuration management system for Heimdall"""
    
    def __init__(self, config_path: Optional[str] = None):
        """Initialize configuration
        
        Args:
            config_path: Path to configuration file (YAML or JSON)
        """
        self.config_path = config_path
        self.config: Dict[str, Any] = {
            # Default configuration values
            "system": {
                "name": "Heimdall Systems",
                "version": "0.1.0",
                "log_level": "INFO",
                "machine_type": "SBO",
                "production_speed": 90000  # bottles per hour
            },
            "cameras": {
                # Default camera configuration, will be extended with actual cameras
                "default": {
                    "type": "simulation",
                    "width": 1280,
                    "height": 720,
                    "fps": 30
                }
            },
            "inspection": {
                # Default inspection configuration
                "default": {
                    "pipeline": "basic",
                    "save_images": False,
                    "rejection_threshold": 0.7,
                    "min_defect_size": 5
                }
            },
            "stations": {
                # Will be populated with actual inspection stations
            },
            "communication": {
                "plc": {
                    "enabled": False,
                    "protocol": "modbus",
                    "ip": "192.168.1.100",
                    "port": 502
                },
                "rejection": {
                    "delay_ms": 100,
                    "pulse_duration_ms": 50
                }
            },
            "ui": {
                "theme": "dark",
                "window_width": 1280,
                "window_height": 800,
                "update_interval_ms": 100
            },
            "paths": {
                "logs": "./logs",
                "images": "./images",
                "results": "./results",
                "models": "./models"
            },
            "performance": {
                "cpu_monitoring": True,
                "memory_monitoring": True,
                "processing_time_monitoring": True
            },
            "debug": {
                "enabled": True,
                "save_debug_images": True,
                "verbose_logging": True
            }
        }
        
        # Load configuration from file if provided
        if config_path is not None:
            self.load_config(config_path)
            
    def load_config(self, config_path: str) -> bool:
        """Load configuration from file
        
        Args:
            config_path: Path to configuration file (YAML or JSON)
            
        Returns:
            True if successful, False otherwise
        """
        if not os.path.exists(config_path):
            logger.warning(f"Configuration file not found: {config_path}")
            return False
            
        try:
            _, ext = os.path.splitext(config_path)
            
            if ext.lower() in ['.yml', '.yaml']:
                # Load YAML configuration
                with open(config_path, 'r') as f:
                    file_config = yaml.safe_load(f)
            elif ext.lower() == '.json':
                # Load JSON configuration
                with open(config_path, 'r') as f:
                    file_config = json.load(f)
            else:
                logger.error(f"Unsupported configuration file format: {ext}")
                return False
                
            # Update configuration with values from file
            self._update_dict(self.config, file_config)
            logger.info(f"Loaded configuration from: {config_path}")
            return True
            
        except Exception as e:
            logger.error(f"Failed to load configuration: {str(e)}")
            return False
            
    def save_config(self, config_path: Optional[str] = None) -> bool:
        """Save configuration to file
        
        Args:
            config_path: Path to save configuration file (YAML or JSON)
            
        Returns:
            True if successful, False otherwise
        """
        save_path = config_path or self.config_path
        
        if save_path is None:
            logger.error("No configuration path specified")
            return False
            
        try:
            os.makedirs(os.path.dirname(save_path), exist_ok=True)
            
            _, ext = os.path.splitext(save_path)
            
            if ext.lower() in ['.yml', '.yaml']:
                # Save as YAML
                with open(save_path, 'w') as f:
                    yaml.dump(self.config, f, default_flow_style=False)
            elif ext.lower() == '.json':
                # Save as JSON
                with open(save_path, 'w') as f:
                    json.dump(self.config, f, indent=2)
            else:
                logger.error(f"Unsupported configuration file format: {ext}")
                return False
                
            logger.info(f"Saved configuration to: {save_path}")
            return True
            
        except Exception as e:
            logger.error(f"Failed to save configuration: {str(e)}")
            return False
    
    def add_camera(self, camera_id: str, config: Dict[str, Any]) -> None:
        """Add a camera configuration
        
        Args:
            camera_id: Unique identifier for the camera
            config: Camera configuration
        """
        if "cameras" not in self.config:
            self.config["cameras"] = {}
            
        self.config["cameras"][camera_id] = config
        
    def add_station(self, station_id: str, config: Dict[str, Any]) -> None:
        """Add an inspection station configuration
        
        Args:
            station_id: Unique identifier for the station
            config: Station configuration
        """
        if "stations" not in self.config:
            self.config["stations"] = {}
            
        self.config["stations"][station_id] = config
        
    def get(self, key: str, default: Any = None) -> Any:
        """Get a configuration value
        
        Args:
            key: Configuration key (dot notation supported, e.g., "ui.theme")
            default: Default value if key is not found
            
        Returns:
            Configuration value or default
        """
        parts = key.split('.')
        current = self.config
        
        for part in parts:
            if part not in current:
                return default
            current = current[part]
            
        return current
        
    def set(self, key: str, value: Any) -> None:
        """Set a configuration value
        
        Args:
            key: Configuration key (dot notation supported)
            value: Configuration value to set
        """
        parts = key.split('.')
        current = self.config
        
        # Navigate to the correct level
        for i, part in enumerate(parts[:-1]):
            if part not in current:
                current[part] = {}
            current = current[part]
            
        # Set the value
        current[parts[-1]] = value
        
    def _update_dict(self, target: Dict[str, Any], source: Dict[str, Any]) -> None:
        """Recursively update a dictionary
        
        Args:
            target: Target dictionary to update
            source: Source dictionary with updates
        """
        for key, value in source.items():
            if isinstance(value, dict) and key in target and isinstance(target[key], dict):
                # Recursively update dictionaries
                self._update_dict(target[key], value)
            else:
                # Directly update values
                target[key] = value
                
    def get_camera_config(self, camera_id: str) -> Dict[str, Any]:
        """Get configuration for a specific camera
        
        Args:
            camera_id: Camera identifier
            
        Returns:
            Camera configuration
        """
        cameras = self.config.get("cameras", {})
        default = cameras.get("default", {})
        camera_config = cameras.get(camera_id, {})
        
        # Merge with default values
        result = default.copy()
        result.update(camera_config)
        
        return result
        
    def get_station_config(self, station_id: str) -> Dict[str, Any]:
        """Get configuration for a specific inspection station
        
        Args:
            station_id: Station identifier
            
        Returns:
            Station configuration
        """
        stations = self.config.get("stations", {})
        inspections = self.config.get("inspection", {})
        default = inspections.get("default", {})
        station_config = stations.get(station_id, {})
        
        # Merge with default values
        result = default.copy()
        result.update(station_config)
        
        return result

    def get_all_cameras(self) -> Dict[str, Dict[str, Any]]:
        """Get all camera configurations
        
        Returns:
            Dictionary of camera configurations
        """
        cameras = self.config.get("cameras", {}).copy()
        if "default" in cameras:
            cameras.pop("default")
        return cameras
        
    def get_all_stations(self) -> Dict[str, Dict[str, Any]]:
        """Get all inspection station configurations
        
        Returns:
            Dictionary of station configurations
        """
        return self.config.get("stations", {}).copy()

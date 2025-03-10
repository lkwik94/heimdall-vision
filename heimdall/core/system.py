# heimdall/core/system.py
import os
import time
import logging
import threading
from typing import Dict, List, Any, Optional, Union, Callable

from heimdall.core.config import Config
from heimdall.core.acquisition import ImageSourceFactory, ImageSource
from heimdall.core.pipeline import Pipeline, PipelineFactory

logger = logging.getLogger("heimdall.system")

class InspectionStation:
    """Represents a single inspection station with camera and pipeline"""
    
    def __init__(self, station_id: str, config: Dict[str, Any], system_config: Config):
        """Initialize an inspection station
        
        Args:
            station_id: Unique identifier for this station
            config: Configuration for this station
            system_config: System-wide configuration
        """
        self.station_id = station_id
        self.config = config
        self.system_config = system_config
        self.logger = logging.getLogger(f"heimdall.station.{station_id}")
        
        # Extract configuration values
        camera_id = config.get("camera_id")
        pipeline_type = config.get("pipeline_type", "basic")
        
        # Initialize components
        self.camera_config = system_config.get_camera_config(camera_id)
        self.image_source = ImageSourceFactory.create_source(camera_id, self.camera_config)
        self.pipeline = PipelineFactory.create_pipeline(
            f"{station_id}_pipeline", 
            pipeline_type, 
            config
        )
        
        # Runtime state
        self.running = False
        self.last_result = None
        self.processing_thread = None
        self.stop_event = threading.Event()
        
        # Stats
        self.frames_processed = 0
        self.defects_detected = 0
        self.avg_processing_time = 0
        
        self.logger.info(f"Initialized inspection station {station_id}")
        
    def start(self) -> bool:
        """Start the inspection station
        
        Returns:
            True if started successfully, False otherwise
        """
        if self.running:
            self.logger.warning(f"Station {self.station_id} already running")
            return False
            
        # Open the image source
        if not self.image_source.open():
            self.logger.error(f"Failed to open image source for station {self.station_id}")
            return False
            
        # Reset state
        self.frames_processed = 0
        self.defects_detected = 0
        self.stop_event.clear()
        self.running = True
        
        # Start processing thread
        self.processing_thread = threading.Thread(
            target=self._processing_loop,
            name=f"station_{self.station_id}"
        )
        self.processing_thread.daemon = True
        self.processing_thread.start()
        
        self.logger.info(f"Started inspection station {self.station_id}")
        return True
        
    def stop(self) -> None:
        """Stop the inspection station"""
        if not self.running:
            return
            
        self.logger.info(f"Stopping inspection station {self.station_id}")
        self.stop_event.set()
        
        # Wait for thread to finish
        if self.processing_thread and self.processing_thread.is_alive():
            self.processing_thread.join(timeout=2.0)
            
        # Close the image source
        self.image_source.close()
        
        self.running = False
        self.logger.info(f"Stopped inspection station {self.station_id}")
        
    def _processing_loop(self) -> None:
        """Main processing loop for the inspection station"""
        self.logger.info(f"Processing loop started for station {self.station_id}")
        
        while not self.stop_event.is_set():
            # Read an image
            success, image = self.image_source.read()
            
            if not success or image is None:
                self.logger.warning(f"Failed to read image from source for station {self.station_id}")
                # Brief pause to avoid busy loop
                time.sleep(0.1)
                continue
                
            # Process the image
            start_time = time.time()
            result = self.pipeline.process(image)
            processing_time = time.time() - start_time
            
            # Update stats
            self.frames_processed += 1
            if result.get("defects_detected", False):
                self.defects_detected += 1
                
            # Update average processing time with exponential moving average
            if self.avg_processing_time == 0:
                self.avg_processing_time = processing_time
            else:
                self.avg_processing_time = 0.9 * self.avg_processing_time + 0.1 * processing_time
                
            # Store result
            self.last_result = result
            
            # Handle rejection if needed
            if result.get("defects_detected", False) and "reject" in self.config:
                self._handle_rejection(result)
                
            # Respect rate limit if specified
            rate_limit = self.config.get("rate_limit_ms", 0)
            if rate_limit > 0:
                elapsed = (time.time() - start_time) * 1000
                if elapsed < rate_limit:
                    time.sleep((rate_limit - elapsed) / 1000)
                    
        self.logger.info(f"Processing loop ended for station {self.station_id}")
        
    def _handle_rejection(self, result: Dict[str, Any]) -> None:
        """Handle bottle rejection
        
        Args:
            result: Processing result
        """
        # This would typically interface with some hardware
        # For now, just log the rejection
        self.logger.info(f"Bottle with defects rejected at station {self.station_id}")
        
    def get_status(self) -> Dict[str, Any]:
        """Get the current status of this station
        
        Returns:
            Status dictionary
        """
        return {
            "station_id": self.station_id,
            "running": self.running,
            "frames_processed": self.frames_processed,
            "defects_detected": self.defects_detected,
            "avg_processing_time": self.avg_processing_time,
            "last_result_time": self.last_result.get("timestamp") if self.last_result else None
        }


class System:
    """Main system coordinator for Heimdall"""
    
    def __init__(self, config_path: Optional[str] = None):
        """Initialize the system
        
        Args:
            config_path: Path to configuration file
        """
        # Initialize logging
        self._setup_logging()
        
        # Load configuration
        self.config = Config(config_path)
        
        # Initialize components
        self.stations: Dict[str, InspectionStation] = {}
        
        logger.info("Heimdall System initialized")
        
    def _setup_logging(self) -> None:
        """Set up logging configuration"""
        log_format = "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
        logging.basicConfig(
            level=logging.INFO,
            format=log_format
        )
        
    def configure_from_file(self, config_path: str) -> bool:
        """Configure the system from a file
        
        Args:
            config_path: Path to configuration file
            
        Returns:
            True if successful, False otherwise
        """
        success = self.config.load_config(config_path)
        if success:
            self._configure_from_loaded_config()
        return success
        
    def _configure_from_loaded_config(self) -> None:
        """Configure the system using the loaded configuration"""
        # Configure stations
        stations_config = self.config.get_all_stations()
        
        for station_id, station_config in stations_config.items():
            self.add_station(station_id, station_config)
            
        logger.info(f"Configured {len(self.stations)} inspection stations")
        
    def add_station(self, station_id: str, config: Dict[str, Any]) -> None:
        """Add an inspection station
        
        Args:
            station_id: Unique identifier for the station
            config: Station configuration
        """
        if station_id in self.stations:
            logger.warning(f"Inspection station {station_id} already exists, reconfiguring")
            self.stations[station_id].stop()
            
        self.stations[station_id] = InspectionStation(station_id, config, self.config)
        logger.info(f"Added inspection station {station_id}")
        
    def remove_station(self, station_id: str) -> bool:
        """Remove an inspection station
        
        Args:
            station_id: Unique identifier for the station
            
        Returns:
            True if removed, False if not found
        """
        if station_id not in self.stations:
            logger.warning(f"Inspection station {station_id} not found")
            return False
            
        self.stations[station_id].stop()
        del self.stations[station_id]
        logger.info(f"Removed inspection station {station_id}")
        return True
        
    def start(self) -> bool:
        """Start the system
        
        Returns:
            True if started successfully
        """
        logger.info("Starting Heimdall System")
        
        # Start all inspection stations
        for station_id, station in self.stations.items():
            success = station.start()
            if not success:
                logger.error(f"Failed to start station {station_id}")
                
        return True
        
    def stop(self) -> None:
        """Stop the system"""
        logger.info("Stopping Heimdall System")
        
        # Stop all inspection stations
        for station in self.stations.values():
            station.stop()
            
    def get_status(self) -> Dict[str, Any]:
        """Get the current system status
        
        Returns:
            System status dictionary
        """
        station_statuses = {
            station_id: station.get_status()
            for station_id, station in self.stations.items()
        }
        
        return {
            "stations": station_statuses,
            "station_count": len(self.stations),
            "running_stations": sum(1 for station in self.stations.values() if station.running),
            "system_time": time.time()
        }

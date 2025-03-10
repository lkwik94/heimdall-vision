#!/usr/bin/env python3
# heimdall/main.py
import os
import sys
import time
import logging
import argparse
from typing import Dict, Any

from heimdall.core.system import System

print("Starting Heimdall Systems main.py")

def parse_args():
    """Parse command line arguments
    
    Returns:
        Parsed arguments
    """
    parser = argparse.ArgumentParser(description="Heimdall Systems - Industrial Vision System")
    parser.add_argument("-c", "--config", type=str, help="Path to configuration file")
    parser.add_argument("-d", "--debug", action="store_true", help="Enable debug logging")
    parser.add_argument("-s", "--simulation", action="store_true", help="Run in simulation mode")
    
    return parser.parse_args()

def main():
    """Main entry point"""
    # Parse arguments
    args = parse_args()
    
    # Set up logging
    log_level = logging.DEBUG if args.debug else logging.INFO
    logging.basicConfig(
        level=log_level,
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    )
    
    logger = logging.getLogger("heimdall.main")
    logger.info("Starting Heimdall Systems")
    
    # Determine configuration path
    config_path = args.config
    if not config_path and args.simulation:
        # Use a default simulation configuration
        config_path = os.path.join(os.path.dirname(__file__), "config", "simulation.yaml")
        if not os.path.exists(config_path):
            # Create a basic simulation configuration
            os.makedirs(os.path.dirname(config_path), exist_ok=True)
            create_simulation_config(config_path)
            
    # Initialize the system
    system = System(config_path)
    
    if args.simulation and not config_path:
        # Configure a basic simulation setup
        logger.info("Configuring simulation mode")
        configure_simulation_system(system)
        
    try:
        # Start the system
        system.start()
        
        # Keep running until interrupted
        logger.info("System running. Press Ctrl+C to stop.")
        while True:
            time.sleep(1)
            
    except KeyboardInterrupt:
        logger.info("Interrupted by user")
    finally:
        # Stop the system
        system.stop()
        logger.info("Heimdall Systems stopped")

def create_simulation_config(config_path: str) -> None:
    """Create a basic simulation configuration
    
    Args:
        config_path: Path to save the configuration
    """
    import yaml
    
    config = {
        "system": {
            "name": "Heimdall Systems Simulation",
            "version": "0.1.0",
            "log_level": "INFO"
        },
        "cameras": {
            "cam_1": {
                "type": "simulation",
                "width": 640,
                "height": 480,
                "pattern": "bottle",
                "inject_defects": True,
                "defect_probability": 0.3
            },
            "cam_2": {
                "type": "simulation",
                "width": 640,
                "height": 480,
                "pattern": "bottle",
                "inject_defects": True,
                "defect_probability": 0.2
            }
        },
        "stations": {
            "base_inspection": {
                "camera_id": "cam_1",
                "pipeline_type": "bottle_base",
                "rate_limit_ms": 100
            },
            "sidewall_inspection": {
                "camera_id": "cam_2",
                "pipeline_type": "basic",
                "rate_limit_ms": 100
            }
        }
    }
    
    with open(config_path, "w") as f:
        yaml.dump(config, f, default_flow_style=False)

def configure_simulation_system(system: System) -> None:
    """Configure the system for simulation mode
    
    Args:
        system: The system to configure
    """
    # Add simulation cameras
    system.config.add_camera("cam_1", {
        "type": "simulation",
        "width": 640,
        "height": 480,
        "pattern": "bottle",
        "inject_defects": True,
        "defect_probability": 0.3
    })
    
    system.config.add_camera("cam_2", {
        "type": "simulation",
        "width": 640,
        "height": 480,
        "pattern": "bottle",
        "inject_defects": True,
        "defect_probability": 0.2
    })
    
    # Add inspection stations
    system.add_station("base_inspection", {
        "camera_id": "cam_1",
        "pipeline_type": "bottle_base",
        "rate_limit_ms": 100
    })
    
    system.add_station("sidewall_inspection", {
        "camera_id": "cam_2",
        "pipeline_type": "basic",
        "rate_limit_ms": 100
    })

if __name__ == "__main__":
    main()



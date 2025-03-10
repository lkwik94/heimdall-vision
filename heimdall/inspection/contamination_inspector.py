# heimdall/inspection/contamination_inspector.py
from typing import Dict, List, Any, Optional, Tuple
import numpy as np
import cv2

from heimdall.inspection.base_inspector import Inspector
from heimdall.core.pipeline import Pipeline, PipelineFactory
from heimdall.detectors.base import DefectDetector, Defect
from heimdall.detectors.contamination_detector import ContaminationDetector

class ContaminationInspector(Inspector):
    """Inspector for bottle contamination"""
    
    def __init__(self, inspector_id: str, config: Dict[str, Any] = None):
        """Initialize a contamination inspector
        
        Args:
            inspector_id: Unique identifier for this inspector
            config: Configuration parameters
        """
        super().__init__(inspector_id, config)
        
    def _setup_pipeline(self):
        """Set up the processing pipeline"""
        self.logger.info("Setting up contamination inspection pipeline")
        
        # Utiliser un pipeline optimisé pour la détection de contamination
        pipeline_type = self.config.get("pipeline_type", "contamination")  # Changé ici!
        self.pipeline = PipelineFactory.create_pipeline(
            f"{self.inspector_id}_pipeline",
            pipeline_type,
            self.config.get("pipeline_config", {})
        )
        
    def _setup_detectors(self):
        """Set up defect detectors"""
        self.logger.info("Setting up contamination defect detectors")
        
        # Create contamination detector
        contamination_detector = ContaminationDetector(
            "contamination_detector",
            self.config.get("contamination_detector_config", {
                "min_contaminant_size": 3,
                "max_contaminant_size": 3000,  # Augmenté pour les grands carrés noirs
                "contrast_threshold": 25,
                "min_confidence": 0.3,
                "use_color": True
            })
        )
        self.detectors.append(contamination_detector)
        
        # Additional detectors could be added here
        
        self.logger.info(f"Configured {len(self.detectors)} detectors")

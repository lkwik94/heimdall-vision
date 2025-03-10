# heimdall/detectors/base.py
import logging
from typing import Dict, List, Any, Optional, Tuple
import numpy as np
import cv2

class Defect:
    """Represents a defect detected in an image"""
    
    def __init__(self, defect_type: str, position: Tuple[int, int], 
                 size: float, confidence: float, metadata: Dict[str, Any] = None):
        """Initialize a defect
        
        Args:
            defect_type: Type of defect (e.g., "contamination", "crack")
            position: (x, y) coordinates of the defect center
            size: Size/area of the defect
            confidence: Confidence score (0-1) of the detection
            metadata: Additional information about the defect
        """
        self.defect_type = defect_type
        self.position = position
        self.size = size
        self.confidence = confidence
        self.metadata = metadata or {}
        
    def __str__(self) -> str:
        return f"Defect({self.defect_type}, pos={self.position}, size={self.size:.1f}, conf={self.confidence:.2f})"
        
    def to_dict(self) -> Dict[str, Any]:
        """Convert defect to dictionary"""
        return {
            "type": self.defect_type,
            "position": self.position,
            "size": self.size,
            "confidence": self.confidence,
            **self.metadata
        }


class DefectDetector:
    """Base class for all defect detectors"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        """Initialize the detector
        
        Args:
            name: Detector name
            config: Configuration parameters
        """
        self.name = name
        self.config = config or {}
        self.logger = logging.getLogger(f"heimdall.detector.{name}")
        
    def detect(self, image: np.ndarray, context: Dict[str, Any] = None) -> List[Defect]:
        """Detect defects in an image
        
        Args:
            image: Input image
            context: Additional context information
            
        Returns:
            List of detected defects
        """
        raise NotImplementedError("Subclasses must implement this method")
        
    def __call__(self, image: np.ndarray, context: Dict[str, Any] = None) -> List[Defect]:
        """Make the detector callable
        
        Args:
            image: Input image
            context: Additional context information
            
        Returns:
            List of detected defects
        """
        if context is None:
            context = {}
            
        start_time = context.get("start_time", 0)
        result = self.detect(image, context)
        self.logger.debug(f"Detector {self.name} found {len(result)} defects")
        
        return result

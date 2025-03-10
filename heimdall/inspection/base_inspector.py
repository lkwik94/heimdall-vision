# heimdall/inspection/base_inspector.py
import time
import logging
from typing import Dict, List, Any, Optional, Tuple
import numpy as np
import cv2

from heimdall.core.pipeline import Pipeline
from heimdall.detectors.base import Defect

class InspectionResult:
    """Result of an inspection"""
    
    def __init__(self, 
                inspection_id: str,
                timestamp: float,
                success: bool,
                defects: List[Defect] = None,
                images: Dict[str, np.ndarray] = None,
                metadata: Dict[str, Any] = None):
        """Initialize an inspection result
        
        Args:
            inspection_id: Unique identifier for this inspection
            timestamp: Unix timestamp of the inspection
            success: Whether the inspection completed successfully
            defects: List of detected defects
            images: Dictionary of images (original, processed, visualization)
            metadata: Additional inspection metadata
        """
        self.inspection_id = inspection_id
        self.timestamp = timestamp
        self.success = success
        self.defects = defects or []
        self.images = images or {}
        self.metadata = metadata or {}
        self.processing_time = metadata.get("processing_time", 0)
        
    @property
    def has_defects(self) -> bool:
        """Check if any defects were detected"""
        return len(self.defects) > 0
        
    @property
    def defect_count(self) -> int:
        """Get the number of defects detected"""
        return len(self.defects)
        
    def to_dict(self) -> Dict[str, Any]:
        """Convert result to dictionary (without images)"""
        return {
            "inspection_id": self.inspection_id,
            "timestamp": self.timestamp,
            "success": self.success,
            "has_defects": self.has_defects,
            "defect_count": self.defect_count,
            "defects": [defect.to_dict() for defect in self.defects],
            "processing_time": self.processing_time,
            "metadata": self.metadata
        }
        
    def __str__(self) -> str:
        return (f"InspectionResult(id={self.inspection_id}, "
               f"success={self.success}, defects={self.defect_count})")


class Inspector:
    """Base class for all inspectors"""
    
    def __init__(self, inspector_id: str, config: Dict[str, Any] = None):
        """Initialize an inspector
        
        Args:
            inspector_id: Unique identifier for this inspector
            config: Configuration parameters
        """
        self.inspector_id = inspector_id
        self.config = config or {}
        self.logger = logging.getLogger(f"heimdall.inspector.{inspector_id}")
        self.pipeline = None
        self.detectors = []
        self._setup_pipeline()
        self._setup_detectors()
        
    def _setup_pipeline(self):
        """Set up the processing pipeline"""
        raise NotImplementedError("Subclasses must implement this method")
        
    def _setup_detectors(self):
        """Set up defect detectors"""
        raise NotImplementedError("Subclasses must implement this method")
        
    def inspect(self, image: np.ndarray, context: Dict[str, Any] = None) -> InspectionResult:
        """Inspect an image
        
        Args:
            image: Input image
            context: Additional context information
            
        Returns:
            Inspection result
        """
        # Initialize context
        if context is None:
            context = {}
            
        # Initialize result
        start_time = time.time()
        inspection_id = context.get("inspection_id", f"{self.inspector_id}_{int(start_time * 1000)}")
        
        # Create result structure
        result = InspectionResult(
            inspection_id=inspection_id,
            timestamp=start_time,
            success=False,
            images={"original": image.copy()},
            metadata={"inspector_id": self.inspector_id}
        )
        
        try:
            # Process image through pipeline
            pipeline_result = self.pipeline.process(image, context)
            processed_image = pipeline_result["result_image"]
            
            # Store processed image
            result.images["processed"] = processed_image
            
            # Apply all detectors
            all_defects = []
            for detector in self.detectors:
                defects = detector(processed_image, context)
                all_defects.extend(defects)
                
                # Create visualization for this detector if available
                if hasattr(detector, "visualize") and callable(detector.visualize):
                    viz_key = f"visualization_{detector.name}"
                    result.images[viz_key] = detector.visualize(image.copy(), defects)
            
            # Update result
            result.defects = all_defects
            result.success = True
            
            # Create summary visualization
            result.images["visualization"] = self._create_visualization(
                image, processed_image, all_defects)
            
        except Exception as e:
            self.logger.error(f"Inspection failed: {str(e)}")
            result.success = False
            result.metadata["error"] = str(e)
            
        finally:
            # Calculate processing time
            processing_time = time.time() - start_time
            result.processing_time = processing_time
            result.metadata["processing_time"] = processing_time
            
            self.logger.info(f"Inspection {inspection_id} completed in {processing_time:.3f}s, "
                           f"found {len(result.defects)} defects")
            
        return result
    
    def _create_visualization(self, 
                            original: np.ndarray, 
                            processed: np.ndarray, 
                            defects: List[Defect]) -> np.ndarray:
        """Create a visualization of the inspection results
        
        Args:
            original: Original image
            processed: Processed image
            defects: List of detected defects
            
        Returns:
            Visualization image
        """
        # Create a copy of the original image
        if len(original.shape) == 2:
            viz = cv2.cvtColor(original, cv2.COLOR_GRAY2BGR)
        else:
            viz = original.copy()
            
        # Draw defects
        for defect in defects:
            # Draw circle at defect position
            cv2.circle(viz, defect.position, 10, (0, 0, 255), 2)
            
            # Draw defect ID and confidence
            cv2.putText(
                viz, 
                f"{defect.defect_type}: {defect.confidence:.2f}",
                (defect.position[0] + 15, defect.position[1]),
                cv2.FONT_HERSHEY_SIMPLEX,
                0.5,
                (0, 0, 255),
                1
            )
            
        # Draw summary text
        cv2.putText(
            viz,
            f"Defects: {len(defects)}",
            (10, 30),
            cv2.FONT_HERSHEY_SIMPLEX,
            1,
            (0, 0, 255) if defects else (0, 255, 0),
            2
        )
        
        return viz

#!/usr/bin/env python3
# heimdall/rust_bridge.py
"""
Bridge module for integrating Rust components with Python.
This module provides a fallback pure Python implementation when
the Rust components are not available.
"""

import os
import sys
import logging
import time
import numpy as np
import cv2
from typing import Dict, List, Any, Optional, Tuple, Union

logger = logging.getLogger("heimdall.rust_bridge")

# Try to import the Rust module
try:
    import heimdall_core
    RUST_AVAILABLE = True
    logger.info("Rust components loaded successfully")
except ImportError:
    RUST_AVAILABLE = False
    logger.warning("Rust components not available, using Python fallback")

class RustBridge:
    """Bridge to Rust high-performance components"""
    
    @staticmethod
    def is_available() -> bool:
        """Check if Rust components are available
        
        Returns:
            True if Rust components are available
        """
        return RUST_AVAILABLE
    
    @staticmethod
    def process_image(image: np.ndarray, pipeline_type: str, params: Dict[str, Any] = None) -> Dict[str, Any]:
        """Process an image using the high-performance pipeline
        
        Args:
            image: Input image
            pipeline_type: Type of pipeline to use
            params: Additional parameters
            
        Returns:
            Dictionary with results
        """
        if RUST_AVAILABLE:
            # Use Rust implementation
            try:
                return heimdall_core.process_image(image, pipeline_type, params or {})
            except Exception as e:
                logger.error(f"Error in Rust process_image: {str(e)}")
                # Fall back to Python implementation
        
        # Python fallback implementation
        start_time = time.time()
        result = {}
        
        # Convert to grayscale
        if len(image.shape) == 3:
            gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        else:
            gray = image
            
        # Apply Gaussian blur
        blurred = cv2.GaussianBlur(gray, (5, 5), 0)
        
        if pipeline_type == "basic":
            # Basic pipeline
            edges = cv2.Canny(blurred, 50, 150)
            
            # Convert back to BGR for visualization
            processed = cv2.cvtColor(edges, cv2.COLOR_GRAY2BGR)
            result["processed_image"] = processed
            
        elif pipeline_type == "contamination":
            # Contamination pipeline
            # Apply adaptive threshold
            binary = cv2.adaptiveThreshold(
                blurred,
                255,
                cv2.ADAPTIVE_THRESH_GAUSSIAN_C,
                cv2.THRESH_BINARY_INV,
                11,
                2
            )
            
            # Find contours
            contours, _ = cv2.findContours(binary, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
            
            # Convert to list of tuples
            contours_list = []
            for contour in contours:
                M = cv2.moments(contour)
                if M["m00"] > 0:
                    cx = int(M["m10"] / M["m00"])
                    cy = int(M["m01"] / M["m00"])
                    area = cv2.contourArea(contour)
                    contours_list.append((cy, cx, 0.75))  # y, x, confidence
            
            # Create visualization
            processed = cv2.cvtColor(binary, cv2.COLOR_GRAY2BGR)
            result["processed_image"] = processed
            result["contours"] = contours_list
            
        else:
            raise ValueError(f"Unsupported pipeline type: {pipeline_type}")
            
        result["processing_time"] = time.time() - start_time
        return result
    
    @staticmethod
    def detect_contamination(image: np.ndarray, min_size: float = 10.0, 
                           max_size: float = 3000.0, threshold: float = 25.0) -> Dict[str, Any]:
        """Detect contamination in an image
        
        Args:
            image: Input image
            min_size: Minimum contaminant size
            max_size: Maximum contaminant size
            threshold: Contrast threshold
            
        Returns:
            Dictionary with defects and timing information
        """
        if RUST_AVAILABLE:
            # Use Rust implementation
            try:
                return heimdall_core.detect_contamination(image, min_size, max_size, threshold)
            except Exception as e:
                logger.error(f"Error in Rust detect_contamination: {str(e)}")
                # Fall back to Python implementation
        
        # Python fallback implementation
        from heimdall.detectors.contamination_detector import ContaminationDetector
        
        start_time = time.time()
        
        # Create detector with specified parameters
        detector = ContaminationDetector(config={
            "min_contaminant_size": min_size,
            "max_contaminant_size": max_size,
            "contrast_threshold": threshold,
            "min_confidence": 0.3
        })
        
        # Detect defects
        defects = detector.detect(image)
        
        # Convert to dictionary
        result = {
            "defects": [defect.to_dict() for defect in defects],
            "processing_time": time.time() - start_time
        }
        
        return result
    
    @staticmethod
    def benchmark_processing(image: np.ndarray, iterations: int = 100) -> Dict[str, float]:
        """Benchmark image processing performance
        
        Args:
            image: Input image
            iterations: Number of iterations
            
        Returns:
            Dictionary with benchmark results
        """
        if RUST_AVAILABLE:
            # Use Rust implementation
            try:
                return heimdall_core.benchmark_processing(image, iterations)
            except Exception as e:
                logger.error(f"Error in Rust benchmark_processing: {str(e)}")
                # Fall back to Python implementation
        
        # Python fallback implementation
        result = {}
        
        # Benchmark basic pipeline
        start_time = time.time()
        for _ in range(iterations):
            RustBridge.process_image(image, "basic")
        basic_time = (time.time() - start_time) / iterations
        
        # Benchmark contamination pipeline
        start_time = time.time()
        for _ in range(iterations):
            RustBridge.process_image(image, "contamination")
        contamination_time = (time.time() - start_time) / iterations
        
        result["basic_pipeline_time"] = basic_time
        result["contamination_pipeline_time"] = contamination_time
        result["iterations"] = iterations
        
        return result
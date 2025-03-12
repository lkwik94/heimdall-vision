#!/usr/bin/env python3
# benchmark.py
"""
Benchmark script to compare the performance of Python and Rust implementations.
"""

import os
import sys
import time
import logging
import argparse
import numpy as np
import cv2
from typing import Dict, List, Any

from heimdall.rust_bridge import RustBridge
from heimdall.core.acquisition import SimulationImageSource
from heimdall.detectors.contamination_detector import ContaminationDetector

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)

logger = logging.getLogger("benchmark")

def benchmark_contamination_detection(iterations: int = 10, image_path: str = None):
    """Benchmark contamination detection
    
    Args:
        iterations: Number of iterations
        image_path: Path to test image (optional)
    """
    logger.info("Benchmarking contamination detection")
    
    # Load or generate test image
    if image_path and os.path.exists(image_path):
        image = cv2.imread(image_path)
        logger.info(f"Loaded test image from {image_path}")
    else:
        # Generate a test image
        source = SimulationImageSource("benchmark_source", {
            "width": 640,
            "height": 480,
            "pattern": "bottle",
            "inject_defects": True,
            "defect_probability": 1.0
        })
        source.open()
        _, image = source.read()
        source.close()
        logger.info("Generated test image")
    
    # Save the test image for reference
    cv2.imwrite("benchmark_image.jpg", image)
    
    # Create Python detector
    detector = ContaminationDetector(config={
        "min_contaminant_size": 10,
        "max_contaminant_size": 3000,
        "contrast_threshold": 25,
        "min_confidence": 0.3
    })
    
    # Benchmark Python implementation
    logger.info(f"Running Python implementation ({iterations} iterations)...")
    py_start = time.time()
    for i in range(iterations):
        defects = detector.detect(image)
    py_time = time.time() - py_start
    py_avg = py_time / iterations
    logger.info(f"Python implementation: {py_avg:.6f} seconds per iteration")
    
    # Check if Rust is available
    if RustBridge.is_available():
        # Benchmark Rust implementation
        logger.info(f"Running Rust implementation ({iterations} iterations)...")
        rust_start = time.time()
        for i in range(iterations):
            result = RustBridge.detect_contamination(image)
        rust_time = time.time() - rust_start
        rust_avg = rust_time / iterations
        logger.info(f"Rust implementation: {rust_avg:.6f} seconds per iteration")
        
        # Calculate speedup
        speedup = py_avg / rust_avg
        logger.info(f"Speedup: {speedup:.2f}x")
    else:
        logger.warning("Rust implementation not available")

def benchmark_image_processing(iterations: int = 10, image_path: str = None):
    """Benchmark image processing pipelines
    
    Args:
        iterations: Number of iterations
        image_path: Path to test image (optional)
    """
    logger.info("Benchmarking image processing pipelines")
    
    # Load or generate test image
    if image_path and os.path.exists(image_path):
        image = cv2.imread(image_path)
        logger.info(f"Loaded test image from {image_path}")
    else:
        # Generate a test image
        source = SimulationImageSource("benchmark_source", {
            "width": 640,
            "height": 480,
            "pattern": "bottle",
            "inject_defects": True,
            "defect_probability": 1.0
        })
        source.open()
        _, image = source.read()
        source.close()
        logger.info("Generated test image")
    
    # Save the test image for reference
    cv2.imwrite("benchmark_image.jpg", image)
    
    # Run benchmark
    result = RustBridge.benchmark_processing(image, iterations)
    
    # Print results
    logger.info(f"Basic pipeline: {result['basic_pipeline_time']:.6f} seconds per iteration")
    logger.info(f"Contamination pipeline: {result['contamination_pipeline_time']:.6f} seconds per iteration")

def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(description="Benchmark Heimdall Vision System")
    parser.add_argument("-i", "--iterations", type=int, default=10, help="Number of iterations")
    parser.add_argument("-t", "--test", choices=["detection", "processing", "all"], default="all", help="Test to run")
    parser.add_argument("-f", "--file", type=str, help="Path to test image")
    
    args = parser.parse_args()
    
    logger.info("Starting benchmark")
    logger.info(f"Rust components available: {RustBridge.is_available()}")
    
    if args.test in ["detection", "all"]:
        benchmark_contamination_detection(args.iterations, args.file)
        
    if args.test in ["processing", "all"]:
        benchmark_image_processing(args.iterations, args.file)
        
    logger.info("Benchmark completed")

if __name__ == "__main__":
    main()
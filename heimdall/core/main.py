#!/usr/bin/env python3
# heimdall/test_basic.py
import time
import logging
import numpy as np
import cv2

from heimdall.core.acquisition import SimulationImageSource
from heimdall.core.pipeline import Pipeline, GaussianBlurStage, CannyEdgeStage

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)

def test_simulation_source():
    """Test the simulation image source"""
    print("\n=== Testing Simulation Image Source ===")
    
    # Create a simulation source
    config = {
        "width": 640,
        "height": 480,
        "pattern": "bottle",
        "inject_defects": True,
        "defect_probability": 0.5
    }
    source = SimulationImageSource("test_source", config)
    
    # Open the source
    source.open()
    
    # Read a few frames
    for i in range(3):
        success, image = source.read()
        if success:
            print(f"Frame {i+1}: {image.shape}")
            
            # Display the image
            cv2.imshow(f"Frame {i+1}", image)
            cv2.waitKey(1000)  # Wait 1 second
            
    # Close the source
    source.close()
    cv2.destroyAllWindows()

def test_basic_pipeline():
    """Test a basic processing pipeline"""
    print("\n=== Testing Basic Pipeline ===")
    
    # Create a test image
    image = np.zeros((480, 640, 3), dtype=np.uint8)
    cv2.rectangle(image, (100, 100), (400, 300), (255, 255, 255), -1)
    cv2.circle(image, (300, 200), 50, (0, 0, 255), -1)
    
    # Create a pipeline
    pipeline = Pipeline("test_pipeline")
    pipeline.add_stage(GaussianBlurStage("blur", {"kernel_size": 5}))
    pipeline.add_stage(CannyEdgeStage("edges"))
    
    # Process the image
    result = pipeline.process(image)
    
    # Display results
    cv2.imshow("Original", image)
    cv2.imshow("Result", result["result_image"])
    cv2.waitKey(3000)  # Wait 3 seconds
    cv2.destroyAllWindows()
    
    # Print processing times
    print("Processing times:")
    for stage, time_taken in result["stage_times"].items():
        print(f"  {stage}: {time_taken:.4f}s")
    print(f"  Total: {result['total_time']:.4f}s")

if __name__ == "__main__":
    test_simulation_source()
    test_basic_pipeline()
    print("\nAll tests completed successfully!")

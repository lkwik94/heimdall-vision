#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
Heimdall Systems - Environment Test Script
Tests that the development environment is properly set up.
"""

import sys
import platform
import subprocess
import numpy as np
import cv2

def print_section(title):
    print(f"\n{'-' * 80}")
    print(f"{title}")
    print(f"{'-' * 80}")

def print_info(label, info):
    print(f"{label.ljust(25)}: {info}")

def run_command(cmd):
    try:
        output = subprocess.check_output(cmd, shell=True, stderr=subprocess.STDOUT)
        return output.decode('utf-8').strip()
    except subprocess.CalledProcessError as e:
        return f"Error: {e.output.decode('utf-8').strip()}"

def test_opencv_processing():
    print_section("OpenCV Test")
    
    # Create test image
    h, w = 1920, 1080
    img = np.random.randint(0, 255, (h, w, 3), dtype=np.uint8)
    
    # Measure CPU processing time
    cpu_start = cv2.getTickCount()
    cpu_blur = cv2.GaussianBlur(img, (19, 19), 5)
    cpu_time = (cv2.getTickCount() - cpu_start) / cv2.getTickFrequency()
    
    print_info("CPU processing time", f"{cpu_time:.5f} seconds")
    print_info("Image size processed", f"{w}x{h}")

def main():
    print_section("System Information")
    print_info("Python Version", platform.python_version())
    print_info("Platform", platform.platform())
    print_info("Processor", platform.processor())
    
    print_section("OpenCV")
    print_info("OpenCV Version", cv2.__version__)
    print_info("OpenCV Build Info", cv2.getBuildInformation().split('\n')[0])
    
    # Test OpenCV
    test_opencv_processing()
    
    # Check if numpy and other scientific libraries work
    print_section("Scientific Libraries")
    print_info("NumPy Version", np.__version__)
    
    try:
        import matplotlib
        print_info("Matplotlib Version", matplotlib.__version__)
    except ImportError:
        print_info("Matplotlib", "Not installed")
    
    try:
        import torch
        print_info("PyTorch Version", torch.__version__)
        print_info("PyTorch CUDA Available", torch.cuda.is_available() if hasattr(torch, 'cuda') else "N/A")
    except ImportError:
        print_info("PyTorch", "Not installed")
    
    try:
        import tensorflow as tf
        print_info("TensorFlow Version", tf.__version__)
    except ImportError:
        print_info("TensorFlow", "Not installed")
    
    # Check PyQt5
    try:
        from PyQt5.QtCore import QT_VERSION_STR
        print_info("Qt Version", QT_VERSION_STR)
    except ImportError:
        print_info("Qt", "Not installed")
    
    print_section("Environment Test Complete")

if __name__ == "__main__":
    main()

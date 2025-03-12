# Heimdall Vision System

An open-source industrial vision system for high-speed bottle inspection at 90,000 BPH (25 bottles/second).

## Overview

Heimdall Vision is a disruptive open-source industrial vision system designed to compete with proprietary solutions at a fraction of the cost. The system uses a hybrid architecture combining Python and Rust to achieve real-time performance while maintaining flexibility and ease of development.

### Key Features

- **High-speed processing**: Optimized for 25 bottles/second (90,000 BPH)
- **Hybrid architecture**: Python for orchestration, Rust for performance-critical components
- **Defect detection**: Contamination, structural defects, and deformations
- **Real-time operation**: Designed for Debian RT with minimal latency
- **Open-source stack**: No proprietary components
- **Web dashboard**: Real-time monitoring and configuration

## Architecture

The system uses a hybrid architecture:

- **Core (Rust)**: High-performance image acquisition, preprocessing, and primary defect detection
- **Analysis (Python)**: Advanced defect classification, statistical analysis, and optimization
- **Interface (Python)**: Dashboard, visualization, and system configuration

## Getting Started

### Prerequisites

- Python 3.8+
- OpenCV
- NumPy
- Rust (optional, for high-performance components)

### Installation

1. Clone the repository:
   ```
   git clone https://github.com/lkwik94/heimdall-vision.git
   cd heimdall-vision
   ```

2. Install Python dependencies:
   ```
   pip install numpy opencv-python pyyaml flask
   ```

3. Build Rust components (optional):
   ```
   cd rust/heimdall-core
   cargo build --release
   ```

### Running the System

#### Test Contamination Detection

```
python -m heimdall.test_contamination
```

#### Run the Web Dashboard

```
python dashboard.py
```

#### Run the Web Viewer for Test Results

```
python web_viewer.py
```

#### Benchmark Performance

```
python benchmark.py
```

## Components

### Core System

- `heimdall/core/`: Core system components
  - `acquisition.py`: Image acquisition from cameras and files
  - `pipeline.py`: Image processing pipelines
  - `system.py`: System coordination and management
  - `config.py`: Configuration management

### Inspection

- `heimdall/inspection/`: Inspection modules
  - `base_inspector.py`: Base class for all inspectors
  - `contamination_inspector.py`: Contamination detection

### Detectors

- `heimdall/detectors/`: Defect detection algorithms
  - `base.py`: Base detector classes
  - `contamination_detector.py`: Contamination detection algorithm

### Rust Components

- `rust/heimdall-core/`: High-performance Rust components
  - `src/lib.rs`: Main Rust library
  - `src/acquisition.rs`: Fast image acquisition
  - `src/processing.rs`: Optimized image processing
  - `src/detection.rs`: High-speed defect detection

## Development

### Adding a New Detector

1. Create a new detector class in `heimdall/detectors/`
2. Implement the `detect()` method
3. Create a corresponding inspector in `heimdall/inspection/`

### Adding a New Processing Pipeline

1. Add a new pipeline type in `heimdall/core/pipeline.py`
2. Implement the corresponding stages

### Optimizing with Rust

1. Identify performance-critical components
2. Implement in Rust under `rust/heimdall-core/src/`
3. Create Python bindings using PyO3
4. Update `heimdall/rust_bridge.py` to use the new components

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- OpenCV for computer vision algorithms
- PyO3 for Rust-Python integration
- The open-source community for making industrial automation more accessible
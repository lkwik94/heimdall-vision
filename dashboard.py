#!/usr/bin/env python3
# dashboard.py
"""
Web dashboard for Heimdall Vision System.
"""

import os
import sys
import time
import logging
import threading
import json
import base64
import io
from typing import Dict, List, Any, Optional
import numpy as np
import cv2
from flask import Flask, render_template, request, jsonify, Response

from heimdall.core.system import System
from heimdall.core.acquisition import SimulationImageSource
from heimdall.inspection.contamination_inspector import ContaminationInspector
from heimdall.rust_bridge import RustBridge

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)

logger = logging.getLogger("dashboard")

# Create Flask app
app = Flask(__name__, 
            static_folder="dashboard/static", 
            template_folder="dashboard/templates")

# Global state
system = None
system_running = False
inspectors = {}
latest_results = {}
processing_stats = {
    "total_images": 0,
    "total_defects": 0,
    "avg_processing_time": 0,
    "defect_rate": 0,
    "start_time": time.time()
}

# Create dashboard directories if they don't exist
os.makedirs("dashboard/static", exist_ok=True)
os.makedirs("dashboard/templates", exist_ok=True)

# Create HTML template
with open("dashboard/templates/index.html", "w") as f:
    f.write("""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Heimdall Vision Dashboard</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.2.3/dist/css/bootstrap.min.css">
    <style>
        body {
            padding-top: 20px;
            background-color: #f5f5f5;
        }
        .card {
            margin-bottom: 20px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
        }
        .card-header {
            font-weight: bold;
            background-color: #343a40;
            color: white;
        }
        .stats-value {
            font-size: 24px;
            font-weight: bold;
        }
        .defect-badge {
            position: absolute;
            top: 10px;
            right: 10px;
            font-size: 14px;
        }
        .image-container {
            position: relative;
            overflow: hidden;
            height: 300px;
            display: flex;
            align-items: center;
            justify-content: center;
            background-color: #eee;
        }
        .image-container img {
            max-width: 100%;
            max-height: 100%;
        }
        #system-status {
            font-size: 18px;
            font-weight: bold;
        }
        .status-running {
            color: #28a745;
        }
        .status-stopped {
            color: #dc3545;
        }
    </style>
</head>
<body>
    <div class="container">
        <header class="d-flex justify-content-between align-items-center mb-4">
            <h1>Heimdall Vision Dashboard</h1>
            <div>
                <span id="system-status" class="me-3 status-stopped">Stopped</span>
                <button id="start-btn" class="btn btn-success me-2">Start</button>
                <button id="stop-btn" class="btn btn-danger">Stop</button>
            </div>
        </header>
        
        <div class="row">
            <!-- System Stats -->
            <div class="col-md-3">
                <div class="card">
                    <div class="card-header">System Statistics</div>
                    <div class="card-body">
                        <div class="mb-3">
                            <div class="text-muted">Total Images</div>
                            <div id="total-images" class="stats-value">0</div>
                        </div>
                        <div class="mb-3">
                            <div class="text-muted">Total Defects</div>
                            <div id="total-defects" class="stats-value">0</div>
                        </div>
                        <div class="mb-3">
                            <div class="text-muted">Defect Rate</div>
                            <div id="defect-rate" class="stats-value">0%</div>
                        </div>
                        <div class="mb-3">
                            <div class="text-muted">Avg. Processing Time</div>
                            <div id="avg-time" class="stats-value">0 ms</div>
                        </div>
                        <div>
                            <div class="text-muted">Uptime</div>
                            <div id="uptime" class="stats-value">00:00:00</div>
                        </div>
                    </div>
                </div>
                
                <div class="card">
                    <div class="card-header">System Configuration</div>
                    <div class="card-body">
                        <div class="mb-3">
                            <label for="camera-select" class="form-label">Camera</label>
                            <select id="camera-select" class="form-select">
                                <option value="simulation">Simulation</option>
                                <option value="file">File</option>
                                <option value="camera">Camera</option>
                            </select>
                        </div>
                        <div class="mb-3">
                            <label for="pipeline-select" class="form-label">Pipeline</label>
                            <select id="pipeline-select" class="form-select">
                                <option value="contamination">Contamination</option>
                                <option value="basic">Basic</option>
                            </select>
                        </div>
                        <div class="mb-3">
                            <label for="threshold-input" class="form-label">Threshold</label>
                            <input type="range" class="form-range" id="threshold-input" min="5" max="50" value="25">
                            <div class="d-flex justify-content-between">
                                <small>5</small>
                                <small id="threshold-value">25</small>
                                <small>50</small>
                            </div>
                        </div>
                        <div class="form-check form-switch mb-3">
                            <input class="form-check-input" type="checkbox" id="rust-switch" checked>
                            <label class="form-check-label" for="rust-switch">Use Rust (if available)</label>
                        </div>
                        <button id="apply-btn" class="btn btn-primary w-100">Apply Settings</button>
                    </div>
                </div>
            </div>
            
            <!-- Live Feed -->
            <div class="col-md-9">
                <div class="card">
                    <div class="card-header">Live Inspection</div>
                    <div class="card-body p-0">
                        <div class="row g-0">
                            <div class="col-md-6">
                                <div class="image-container">
                                    <img id="original-image" src="/static/placeholder.jpg" alt="Original Image">
                                </div>
                                <div class="p-3 bg-light border-top">
                                    <h5>Original Image</h5>
                                </div>
                            </div>
                            <div class="col-md-6">
                                <div class="image-container">
                                    <img id="processed-image" src="/static/placeholder.jpg" alt="Processed Image">
                                    <span id="defect-badge" class="badge bg-danger defect-badge">0 defects</span>
                                </div>
                                <div class="p-3 bg-light border-top">
                                    <h5>Processed Image</h5>
                                    <div id="processing-time" class="text-muted">Processing time: 0 ms</div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                
                <div class="card">
                    <div class="card-header">Defect Details</div>
                    <div class="card-body">
                        <table class="table table-striped">
                            <thead>
                                <tr>
                                    <th>ID</th>
                                    <th>Type</th>
                                    <th>Position</th>
                                    <th>Size</th>
                                    <th>Confidence</th>
                                </tr>
                            </thead>
                            <tbody id="defects-table">
                                <tr>
                                    <td colspan="5" class="text-center">No defects detected</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>
    </div>
    
    <script src="https://cdn.jsdelivr.net/npm/bootstrap@5.2.3/dist/js/bootstrap.bundle.min.js"></script>
    <script>
        // Dashboard functionality
        document.addEventListener('DOMContentLoaded', function() {
            // Elements
            const startBtn = document.getElementById('start-btn');
            const stopBtn = document.getElementById('stop-btn');
            const applyBtn = document.getElementById('apply-btn');
            const systemStatus = document.getElementById('system-status');
            const originalImage = document.getElementById('original-image');
            const processedImage = document.getElementById('processed-image');
            const defectBadge = document.getElementById('defect-badge');
            const defectsTable = document.getElementById('defects-table');
            const processingTime = document.getElementById('processing-time');
            const totalImages = document.getElementById('total-images');
            const totalDefects = document.getElementById('total-defects');
            const defectRate = document.getElementById('defect-rate');
            const avgTime = document.getElementById('avg-time');
            const uptime = document.getElementById('uptime');
            const thresholdInput = document.getElementById('threshold-input');
            const thresholdValue = document.getElementById('threshold-value');
            
            // Update threshold value display
            thresholdInput.addEventListener('input', function() {
                thresholdValue.textContent = this.value;
            });
            
            // Start system
            startBtn.addEventListener('click', function() {
                fetch('/api/start', { method: 'POST' })
                    .then(response => response.json())
                    .then(data => {
                        if (data.success) {
                            systemStatus.textContent = 'Running';
                            systemStatus.classList.remove('status-stopped');
                            systemStatus.classList.add('status-running');
                        }
                    });
            });
            
            // Stop system
            stopBtn.addEventListener('click', function() {
                fetch('/api/stop', { method: 'POST' })
                    .then(response => response.json())
                    .then(data => {
                        if (data.success) {
                            systemStatus.textContent = 'Stopped';
                            systemStatus.classList.remove('status-running');
                            systemStatus.classList.add('status-stopped');
                        }
                    });
            });
            
            // Apply settings
            applyBtn.addEventListener('click', function() {
                const settings = {
                    camera: document.getElementById('camera-select').value,
                    pipeline: document.getElementById('pipeline-select').value,
                    threshold: parseInt(thresholdInput.value),
                    use_rust: document.getElementById('rust-switch').checked
                };
                
                fetch('/api/settings', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify(settings)
                })
                .then(response => response.json())
                .then(data => {
                    if (data.success) {
                        alert('Settings applied successfully');
                    } else {
                        alert('Failed to apply settings: ' + data.error);
                    }
                });
            });
            
            // Update images and stats
            function updateDashboard() {
                // Get latest results
                fetch('/api/latest')
                    .then(response => response.json())
                    .then(data => {
                        if (data.original_image) {
                            originalImage.src = 'data:image/jpeg;base64,' + data.original_image;
                        }
                        
                        if (data.processed_image) {
                            processedImage.src = 'data:image/jpeg;base64,' + data.processed_image;
                        }
                        
                        // Update defect badge
                        const defectCount = data.defects ? data.defects.length : 0;
                        defectBadge.textContent = defectCount + ' defects';
                        
                        // Update processing time
                        processingTime.textContent = 'Processing time: ' + 
                            (data.processing_time * 1000).toFixed(2) + ' ms';
                        
                        // Update defects table
                        if (data.defects && data.defects.length > 0) {
                            let tableHtml = '';
                            data.defects.forEach((defect, index) => {
                                tableHtml += `<tr>
                                    <td>${index + 1}</td>
                                    <td>${defect.type}</td>
                                    <td>(${defect.position[0]}, ${defect.position[1]})</td>
                                    <td>${defect.size.toFixed(1)}</td>
                                    <td>${(defect.confidence * 100).toFixed(1)}%</td>
                                </tr>`;
                            });
                            defectsTable.innerHTML = tableHtml;
                        } else {
                            defectsTable.innerHTML = '<tr><td colspan="5" class="text-center">No defects detected</td></tr>';
                        }
                    });
                
                // Get system stats
                fetch('/api/stats')
                    .then(response => response.json())
                    .then(data => {
                        totalImages.textContent = data.total_images;
                        totalDefects.textContent = data.total_defects;
                        defectRate.textContent = data.defect_rate.toFixed(1) + '%';
                        avgTime.textContent = data.avg_processing_time.toFixed(2) + ' ms';
                        
                        // Format uptime
                        const seconds = Math.floor(data.uptime);
                        const hours = Math.floor(seconds / 3600);
                        const minutes = Math.floor((seconds % 3600) / 60);
                        const secs = seconds % 60;
                        uptime.textContent = 
                            hours.toString().padStart(2, '0') + ':' +
                            minutes.toString().padStart(2, '0') + ':' +
                            secs.toString().padStart(2, '0');
                        
                        // Update system status
                        if (data.running) {
                            systemStatus.textContent = 'Running';
                            systemStatus.classList.remove('status-stopped');
                            systemStatus.classList.add('status-running');
                        } else {
                            systemStatus.textContent = 'Stopped';
                            systemStatus.classList.remove('status-running');
                            systemStatus.classList.add('status-stopped');
                        }
                    });
            }
            
            // Initial update
            updateDashboard();
            
            // Update every second
            setInterval(updateDashboard, 1000);
        });
    </script>
</body>
</html>
""")

# Create placeholder image
placeholder = np.ones((300, 400, 3), dtype=np.uint8) * 200
cv2.putText(placeholder, "No Image", (150, 150), cv2.FONT_HERSHEY_SIMPLEX, 1, (100, 100, 100), 2)
cv2.imwrite("dashboard/static/placeholder.jpg", placeholder)

# Processing thread
def processing_thread():
    """Background thread for continuous image processing"""
    global latest_results, processing_stats
    
    logger.info("Processing thread started")
    
    # Create image source
    source = SimulationImageSource("dashboard_source", {
        "width": 640,
        "height": 480,
        "pattern": "bottle",
        "inject_defects": True,
        "defect_probability": 0.3
    })
    source.open()
    
    # Create inspector
    inspector = ContaminationInspector("dashboard_inspector")
    
    # Processing loop
    running = True
    use_rust = RustBridge.is_available()
    threshold = 25
    system_running = False
    
    while running:
        try:
            # Check if system is running
            if not system_running:
                time.sleep(0.5)
                continue
                
            # Read image
            success, image = source.read()
            if not success:
                time.sleep(0.1)
                continue
                
            # Process image
            start_time = time.time()
            
            if use_rust:
                # Use Rust implementation
                result = RustBridge.detect_contamination(image, threshold=threshold)
                defects = result["defects"]
                processing_time = result["processing_time"]
                
                # Create visualization
                processed_image = image.copy()
                for defect in defects:
                    pos = defect["position"]
                    conf = defect["confidence"]
                    
                    # Draw circle
                    cv2.circle(processed_image, (pos[1], pos[0]), 10, (0, 0, 255), 2)
                    
                    # Draw confidence
                    cv2.putText(
                        processed_image,
                        f"{conf:.2f}",
                        (pos[1] + 15, pos[0]),
                        cv2.FONT_HERSHEY_SIMPLEX,
                        0.5,
                        (0, 0, 255),
                        1
                    )
            else:
                # Use Python implementation
                result = inspector.inspect(image)
                defects = result.defects
                processing_time = result.processing_time
                processed_image = result.images.get("visualization", image.copy())
            
            # Update statistics
            processing_stats["total_images"] += 1
            processing_stats["total_defects"] += len(defects)
            
            # Update average processing time with exponential moving average
            if processing_stats["avg_processing_time"] == 0:
                processing_stats["avg_processing_time"] = processing_time * 1000  # ms
            else:
                processing_stats["avg_processing_time"] = (
                    0.9 * processing_stats["avg_processing_time"] + 
                    0.1 * processing_time * 1000
                )
                
            # Update defect rate
            if processing_stats["total_images"] > 0:
                processing_stats["defect_rate"] = (
                    processing_stats["total_defects"] / processing_stats["total_images"] * 100
                )
            
            # Convert images to base64 for web display
            _, original_buffer = cv2.imencode(".jpg", image)
            original_base64 = base64.b64encode(original_buffer).decode("utf-8")
            
            _, processed_buffer = cv2.imencode(".jpg", processed_image)
            processed_base64 = base64.b64encode(processed_buffer).decode("utf-8")
            
            # Update latest results
            latest_results = {
                "original_image": original_base64,
                "processed_image": processed_base64,
                "defects": [d.to_dict() if hasattr(d, "to_dict") else d for d in defects],
                "processing_time": processing_time,
                "timestamp": time.time()
            }
            
            # Limit processing rate
            time.sleep(0.1)
            
        except Exception as e:
            logger.error(f"Error in processing thread: {str(e)}")
            time.sleep(1)
    
    # Clean up
    source.close()
    logger.info("Processing thread stopped")

# API routes
@app.route("/")
def index():
    """Serve the dashboard"""
    return render_template("index.html")

@app.route("/api/latest")
def api_latest():
    """Get latest processing results"""
    return jsonify(latest_results)

@app.route("/api/stats")
def api_stats():
    """Get system statistics"""
    global system, system_running, processing_stats
    
    # Calculate uptime
    uptime = time.time() - processing_stats["start_time"]
    
    return jsonify({
        "total_images": processing_stats["total_images"],
        "total_defects": processing_stats["total_defects"],
        "defect_rate": processing_stats["defect_rate"],
        "avg_processing_time": processing_stats["avg_processing_time"],
        "uptime": uptime,
        "running": system_running
    })

@app.route("/api/start", methods=["POST"])
def api_start():
    """Start the system"""
    global system, system_running
    
    try:
        if not system:
            system = System()
            
        system.start()
        system_running = True
        return jsonify({"success": True})
    except Exception as e:
        logger.error(f"Error starting system: {str(e)}")
        return jsonify({"success": False, "error": str(e)})

@app.route("/api/stop", methods=["POST"])
def api_stop():
    """Stop the system"""
    global system, system_running
    
    try:
        if system:
            system.stop()
        system_running = False
        return jsonify({"success": True})
    except Exception as e:
        logger.error(f"Error stopping system: {str(e)}")
        return jsonify({"success": False, "error": str(e)})

@app.route("/api/settings", methods=["POST"])
def api_settings():
    """Update system settings"""
    global use_rust, threshold
    
    try:
        data = request.json
        
        # Update settings
        use_rust = data.get("use_rust", True) and RustBridge.is_available()
        threshold = data.get("threshold", 25)
        
        return jsonify({"success": True})
    except Exception as e:
        logger.error(f"Error updating settings: {str(e)}")
        return jsonify({"success": False, "error": str(e)})

def main():
    """Main entry point"""
    global system, system_running
    
    # Parse command line arguments
    import argparse
    parser = argparse.ArgumentParser(description="Heimdall Vision Dashboard")
    parser.add_argument("-p", "--port", type=int, default=59858, help="Port to run the server on")
    parser.add_argument("-d", "--debug", action="store_true", help="Run in debug mode")
    
    args = parser.parse_args()
    
    # Initialize system
    system = System()
    system_running = False
    
    # Start processing thread
    thread = threading.Thread(target=processing_thread)
    thread.daemon = True
    thread.start()
    
    # Start web server
    logger.info(f"Starting dashboard on port {args.port}")
    app.run(host="0.0.0.0", port=args.port, debug=args.debug, threaded=True)

if __name__ == "__main__":
    main()
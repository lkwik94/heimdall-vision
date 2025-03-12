#!/usr/bin/env python3
import os
import http.server
import socketserver
import webbrowser
from urllib.parse import parse_qs, urlparse
import json
import threading
import time

# Configuration
PORT = 52829
HOST = "0.0.0.0"
DIRECTORY = os.path.dirname(os.path.abspath(__file__))

class HeimdallRequestHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIRECTORY, **kwargs)
        
    def do_GET(self):
        # Parse URL
        parsed_url = urlparse(self.path)
        path = parsed_url.path
        
        # Serve the main page
        if path == "/" or path == "/index.html":
            self.send_response(200)
            self.send_header("Content-type", "text/html")
            self.end_headers()
            
            # Create HTML content
            html = self.generate_html()
            self.wfile.write(html.encode())
            return
            
        # API endpoint to get image list
        elif path == "/api/images":
            self.send_response(200)
            self.send_header("Content-type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            
            # Get image list
            images = self.get_image_list()
            self.wfile.write(json.dumps(images).encode())
            return
            
        # Default: serve files
        return super().do_GET()
        
    def generate_html(self):
        """Generate the HTML for the main page"""
        html = """
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Heimdall Vision - Web Viewer</title>
            <style>
                body {
                    font-family: Arial, sans-serif;
                    margin: 0;
                    padding: 20px;
                    background-color: #f5f5f5;
                }
                h1 {
                    color: #333;
                    text-align: center;
                }
                .container {
                    max-width: 1200px;
                    margin: 0 auto;
                    background-color: white;
                    padding: 20px;
                    border-radius: 5px;
                    box-shadow: 0 2px 5px rgba(0,0,0,0.1);
                }
                .image-set {
                    display: flex;
                    flex-wrap: wrap;
                    margin-bottom: 30px;
                    border-bottom: 1px solid #eee;
                    padding-bottom: 20px;
                }
                .image-container {
                    margin: 10px;
                    text-align: center;
                }
                .image-container img {
                    max-width: 100%;
                    max-height: 300px;
                    border: 1px solid #ddd;
                }
                .image-container h3 {
                    margin: 5px 0;
                    font-size: 14px;
                }
                .header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: 20px;
                }
                .refresh-btn {
                    padding: 8px 16px;
                    background-color: #4CAF50;
                    color: white;
                    border: none;
                    border-radius: 4px;
                    cursor: pointer;
                }
                .refresh-btn:hover {
                    background-color: #45a049;
                }
            </style>
        </head>
        <body>
            <div class="container">
                <div class="header">
                    <h1>Heimdall Vision - Web Viewer</h1>
                    <button class="refresh-btn" onclick="loadImages()">Refresh</button>
                </div>
                <div id="image-sets-container">
                    Loading images...
                </div>
            </div>
            
            <script>
                // Load images from the API
                function loadImages() {
                    fetch('/api/images')
                        .then(response => response.json())
                        .then(data => {
                            const container = document.getElementById('image-sets-container');
                            container.innerHTML = '';
                            
                            // Group images by set
                            const imageSets = {};
                            data.forEach(image => {
                                const parts = image.name.split('_');
                                const type = parts[0];
                                const setNumber = parts[1].split('.')[0];
                                
                                if (!imageSets[setNumber]) {
                                    imageSets[setNumber] = [];
                                }
                                
                                imageSets[setNumber].push({
                                    type: type,
                                    path: image.path
                                });
                            });
                            
                            // Create HTML for each image set
                            Object.keys(imageSets).sort().forEach(setNumber => {
                                const setDiv = document.createElement('div');
                                setDiv.className = 'image-set';
                                
                                const setTitle = document.createElement('h2');
                                setTitle.textContent = `Image Set ${setNumber}`;
                                setDiv.appendChild(setTitle);
                                
                                // Sort images by type
                                const sortOrder = {
                                    'original': 1,
                                    'processed': 2,
                                    'visualization': 3
                                };
                                
                                imageSets[setNumber].sort((a, b) => {
                                    return sortOrder[a.type] - sortOrder[b.type];
                                });
                                
                                // Add each image
                                imageSets[setNumber].forEach(image => {
                                    const imageDiv = document.createElement('div');
                                    imageDiv.className = 'image-container';
                                    
                                    const title = document.createElement('h3');
                                    title.textContent = image.type.charAt(0).toUpperCase() + image.type.slice(1);
                                    
                                    const img = document.createElement('img');
                                    img.src = image.path;
                                    img.alt = image.type;
                                    
                                    imageDiv.appendChild(title);
                                    imageDiv.appendChild(img);
                                    setDiv.appendChild(imageDiv);
                                });
                                
                                container.appendChild(setDiv);
                            });
                        })
                        .catch(error => {
                            console.error('Error loading images:', error);
                            document.getElementById('image-sets-container').innerHTML = 
                                '<p>Error loading images. Please try refreshing.</p>';
                        });
                }
                
                // Load images when the page loads
                window.onload = loadImages;
            </script>
        </body>
        </html>
        """
        return html
        
    def get_image_list(self):
        """Get a list of images in the results directory"""
        results_dir = os.path.join(DIRECTORY, "results")
        images = []
        
        if os.path.exists(results_dir):
            for filename in os.listdir(results_dir):
                if filename.endswith(('.jpg', '.jpeg', '.png')):
                    images.append({
                        "name": filename,
                        "path": f"/results/{filename}"
                    })
                    
        return images

def run_server():
    """Run the HTTP server"""
    handler = HeimdallRequestHandler
    
    with socketserver.TCPServer((HOST, PORT), handler) as httpd:
        print(f"Server running at http://{HOST}:{PORT}")
        httpd.serve_forever()

if __name__ == "__main__":
    # Start the server
    server_thread = threading.Thread(target=run_server)
    server_thread.daemon = True
    server_thread.start()
    
    # Open the browser
    print(f"Opening browser at http://localhost:{PORT}")
    webbrowser.open(f"http://localhost:{PORT}")
    
    # Keep the main thread running
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("Server stopped")
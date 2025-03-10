# heimdall/core/acquisition.py
from abc import ABC, abstractmethod
from typing import Dict, List, Tuple, Optional, Any, Union
import os
import time
import logging
import numpy as np
import cv2

logger = logging.getLogger("heimdall.acquisition")

class ImageSource(ABC):
    """Abstract base class for image sources"""
    
    def __init__(self, source_id: str, config: Dict[str, Any]):
        """Initialize the image source
        
        Args:
            source_id: Unique identifier for this source
            config: Configuration for this source
        """
        self.source_id = source_id
        self.config = config
        self.is_open = False
        self.logger = logging.getLogger(f"heimdall.acquisition.{source_id}")
        
    @abstractmethod
    def open(self) -> bool:
        """Open the image source
        
        Returns:
            True if successful, False otherwise
        """
        pass
        
    @abstractmethod
    def close(self) -> None:
        """Close the image source"""
        pass
        
    @abstractmethod
    def read(self) -> Tuple[bool, Optional[np.ndarray]]:
        """Read an image from the source
        
        Returns:
            Tuple containing:
                - Success flag (True if read was successful)
                - Image data (or None if unsuccessful)
        """
        pass
        
    def __enter__(self):
        """Support for context manager pattern"""
        self.open()
        return self
        
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Support for context manager pattern"""
        self.close()


class FileImageSource(ImageSource):
    """Image source that reads from a file"""
    
    def __init__(self, source_id: str, config: Dict[str, Any]):
        """Initialize the file image source
        
        Args:
            source_id: Unique identifier for this source
            config: Configuration containing file_path
        """
        super().__init__(source_id, config)
        self.file_path = config.get("file_path")
        self._image = None
        
    def open(self) -> bool:
        """Open the image file
        
        Returns:
            True if successful, False otherwise
        """
        if not self.file_path:
            self.logger.error("No file path specified")
            return False
            
        if not os.path.exists(self.file_path):
            self.logger.error(f"File not found: {self.file_path}")
            return False
            
        self._image = cv2.imread(self.file_path)
        success = self._image is not None
        
        if success:
            self.is_open = True
            self.logger.info(f"Loaded image: {self.file_path}, "
                           f"shape: {self._image.shape}")
        else:
            self.logger.error(f"Failed to load image: {self.file_path}")
            
        return success
        
    def close(self) -> None:
        """Close the image source"""
        self._image = None
        self.is_open = False
        
    def read(self) -> Tuple[bool, Optional[np.ndarray]]:
        """Read the image
        
        Returns:
            Tuple containing:
                - Success flag
                - Image data (or None if unsuccessful)
        """
        if self._image is None:
            success = self.open()
            if not success:
                return False, None
                
        return True, self._image.copy()


class DirectoryImageSource(ImageSource):
    """Image source that reads all images from a directory"""
    
    def __init__(self, source_id: str, config: Dict[str, Any]):
        """Initialize the directory image source
        
        Args:
            source_id: Unique identifier for this source
            config: Configuration containing directory_path
        """
        super().__init__(source_id, config)
        self.directory_path = config.get("directory_path")
        self.extensions = config.get("extensions", ['.jpg', '.jpeg', '.png', '.bmp'])
        self.loop = config.get("loop", False)
        self.file_paths = []
        self.current_index = 0
        
    def open(self) -> bool:
        """Open the directory and enumerate image files
        
        Returns:
            True if successful, False otherwise
        """
        if not self.directory_path:
            self.logger.error("No directory path specified")
            return False
            
        if not os.path.exists(self.directory_path):
            self.logger.error(f"Directory not found: {self.directory_path}")
            return False
            
        # Find all image files
        self.file_paths = []
        for file in os.listdir(self.directory_path):
            ext = os.path.splitext(file)[1].lower()
            if ext in self.extensions:
                self.file_paths.append(os.path.join(self.directory_path, file))
                
        self.file_paths.sort()  # Sort files for consistent ordering
        self.current_index = 0
        
        if not self.file_paths:
            self.logger.warning(f"No image files found in: {self.directory_path}")
            return False
            
        self.is_open = True
        self.logger.info(f"Found {len(self.file_paths)} images in: {self.directory_path}")
        return True
        
    def close(self) -> None:
        """Close the image source"""
        self.current_index = 0
        self.is_open = False
        
    def read(self) -> Tuple[bool, Optional[np.ndarray]]:
        """Read the next image
        
        Returns:
            Tuple containing:
                - Success flag
                - Image data (or None if unsuccessful)
        """
        if not self.file_paths:
            success = self.open()
            if not success:
                return False, None
                
        if self.current_index >= len(self.file_paths):
            if self.loop:
                self.current_index = 0
            else:
                self.logger.info("Reached end of directory")
                return False, None
                
        file_path = self.file_paths[self.current_index]
        self.current_index += 1
        
        image = cv2.imread(file_path)
        if image is None:
            self.logger.warning(f"Failed to load image: {file_path}")
            return False, None
            
        return True, image


class CameraImageSource(ImageSource):
    """Image source that captures from a camera"""
    
    def __init__(self, source_id: str, config: Dict[str, Any]):
        """Initialize the camera image source
        
        Args:
            source_id: Unique identifier for this source
            config: Configuration for the camera
        """
        super().__init__(source_id, config)
        self.camera_id = config.get("camera_id", 0)
        self.width = config.get("width", 640)
        self.height = config.get("height", 480)
        self.fps = config.get("fps", 30)
        self.camera = None
        
    def open(self) -> bool:
        """Open the camera
        
        Returns:
            True if successful, False otherwise
        """
        self.camera = cv2.VideoCapture(self.camera_id)
        
        if not self.camera.isOpened():
            self.logger.error(f"Failed to open camera {self.camera_id}")
            return False
            
        self.camera.set(cv2.CAP_PROP_FRAME_WIDTH, self.width)
        self.camera.set(cv2.CAP_PROP_FRAME_HEIGHT, self.height)
        self.camera.set(cv2.CAP_PROP_FPS, self.fps)
        
        actual_width = self.camera.get(cv2.CAP_PROP_FRAME_WIDTH)
        actual_height = self.camera.get(cv2.CAP_PROP_FRAME_HEIGHT)
        actual_fps = self.camera.get(cv2.CAP_PROP_FPS)
        
        self.is_open = True
        self.logger.info(f"Opened camera {self.camera_id}, "
                       f"resolution: {actual_width}x{actual_height}, "
                       f"fps: {actual_fps}")
        return True
        
    def close(self) -> None:
        """Close the camera"""
        if self.camera is not None:
            self.camera.release()
            self.camera = None
        self.is_open = False
        
    def read(self) -> Tuple[bool, Optional[np.ndarray]]:
        """Read a frame from the camera
        
        Returns:
            Tuple containing:
                - Success flag
                - Image data (or None if unsuccessful)
        """
        if self.camera is None:
            success = self.open()
            if not success:
                return False, None
                
        success, frame = self.camera.read()
        
        if not success:
            self.logger.warning("Failed to capture frame")
            return False, None
            
        return True, frame


class SimulationImageSource(ImageSource):
    """Image source that generates synthetic images for testing"""
    
    def __init__(self, source_id: str, config: Dict[str, Any]):
        """Initialize the simulation image source
        
        Args:
            source_id: Unique identifier for this source
            config: Configuration for the simulation
        """
        super().__init__(source_id, config)
        self.width = config.get("width", 640)
        self.height = config.get("height", 480)
        self.pattern = config.get("pattern", "bottle")
        self.inject_defects = config.get("inject_defects", True)
        self.defect_probability = config.get("defect_probability", 0.3)
        self.is_open = False
        self.frame_count = 0
        
    def open(self) -> bool:
        """Initialize the simulation
        
        Returns:
            True
        """
        self.is_open = True
        self.frame_count = 0
        return True
        
    def close(self) -> None:
        """Close the simulation"""
        self.is_open = False
        
    def _create_bottle_image(self, with_defect: bool = False) -> np.ndarray:
        """Create a synthetic bottle image
        
        Args:
            with_defect: Whether to include a defect
            
        Returns:
            Synthetic image
        """
        # Create a blank image
        image = np.ones((self.height, self.width, 3), dtype=np.uint8) * 220
        
        # Draw a bottle shape
        center_x = self.width // 2
        center_y = self.height // 2
        bottle_width = min(self.width, self.height) // 3
        bottle_height = min(self.width, self.height) // 2
        
        # Draw bottle outline
        cv2.rectangle(image, 
                     (center_x - bottle_width // 2, center_y - bottle_height // 2),
                     (center_x + bottle_width // 2, center_y + bottle_height // 2),
                     (100, 100, 100), 2)
        
        # Draw bottle bottom as a circle
        cv2.circle(image, 
                  (center_x, center_y + bottle_height // 2 - 20),
                  bottle_width // 2 - 5,
                  (80, 80, 80), -1)
        
        if with_defect:
            # Add a random defect (dark spot)
            defect_x = np.random.randint(center_x - bottle_width // 3, 
                                       center_x + bottle_width // 3)
            defect_y = np.random.randint(center_y - bottle_height // 3, 
                                       center_y + bottle_height // 3)
            defect_radius = np.random.randint(3, 10)
            
            cv2.circle(image, (defect_x, defect_y), defect_radius, (40, 40, 40), -1)
            
            # Add a text label for visualization
            cv2.putText(image, "DEFECT", (10, 30), 
                      cv2.FONT_HERSHEY_SIMPLEX, 1, (0, 0, 255), 2)
        
        # Add some info text
        cv2.putText(image, f"Frame: {self.frame_count}", (10, self.height - 10),
                  cv2.FONT_HERSHEY_SIMPLEX, 0.5, (0, 0, 0), 1)
        
        return image
        
    def read(self) -> Tuple[bool, Optional[np.ndarray]]:
        """Generate a new synthetic image
        
        Returns:
            Tuple containing:
                - Success flag (always True)
                - Synthetic image
        """
        if not self.is_open:
            self.open()
            
        self.frame_count += 1
        
        # Decide if this frame should have a defect
        include_defect = self.inject_defects and np.random.random() < self.defect_probability
        
        # Generate image based on selected pattern
        if self.pattern == "bottle":
            image = self._create_bottle_image(include_defect)
        else:
            # Default checkerboard pattern
            image = np.zeros((self.height, self.width, 3), dtype=np.uint8)
            tile_size = 50
            for i in range(0, self.height, tile_size):
                for j in range(0, self.width, tile_size):
                    if (i // tile_size + j // tile_size) % 2 == 0:
                        image[i:i+tile_size, j:j+tile_size, :] = 255
            
            if include_defect:
                # Add a random defect
                defect_x = np.random.randint(0, self.width)
                defect_y = np.random.randint(0, self.height)
                defect_radius = np.random.randint(10, 30)
                
                cv2.circle(image, (defect_x, defect_y), defect_radius, (0, 0, 255), -1)
        
        # Simulate some processing time
        time.sleep(0.01)
        
        return True, image


class ImageSourceFactory:
    """Factory for creating image sources"""
    
    @staticmethod
    def create_source(source_id: str, config: Dict[str, Any]) -> ImageSource:
        """Create an image source based on configuration
        
        Args:
            source_id: Unique identifier for the source
            config: Source configuration
            
        Returns:
            An image source instance
            
        Raises:
            ValueError: If the source type is not supported
        """
        source_type = config.get("type", "simulation")
        
        if source_type == "file":
            return FileImageSource(source_id, config)
        elif source_type == "directory":
            return DirectoryImageSource(source_id, config)
        elif source_type == "camera":
            return CameraImageSource(source_id, config)
        elif source_type == "simulation":
            return SimulationImageSource(source_id, config)
        else:
            raise ValueError(f"Unsupported image source type: {source_type}")

# heimdall/core/pipeline.py
import time
import logging
from typing import Dict, List, Any, Callable, Optional, Union, Tuple
import numpy as np
import cv2

class ProcessingStage:
    """Base class for all processing stages in a pipeline"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        """Initialize a processing stage
        
        Args:
            name: Name of the stage
            config: Configuration for the stage
        """
        self.name = name
        self.config = config or {}
        self.logger = logging.getLogger(f"heimdall.pipeline.{name}")
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        """Process an image
        
        Args:
            image: Input image
            context: Processing context
            
        Returns:
            Processed image
        """
        raise NotImplementedError("Subclasses must implement this method")
        
    def __call__(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        """Make the stage callable
        
        Args:
            image: Input image
            context: Processing context
            
        Returns:
            Processed image
        """
        if context is None:
            context = {}
            
        start_time = time.time()
        result = self.process(image, context)
        processing_time = time.time() - start_time
        
        # Store processing time in context
        stage_times = context.get("stage_times", {})
        stage_times[self.name] = processing_time
        context["stage_times"] = stage_times
        
        self.logger.debug(f"Stage {self.name} completed in {processing_time:.4f}s")
        return result


class Pipeline:
    """A pipeline for image processing"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        """Initialize a pipeline
        
        Args:
            name: Name of the pipeline
            config: Configuration for the pipeline
        """
        self.name = name
        self.config = config or {}
        self.stages: List[ProcessingStage] = []
        self.logger = logging.getLogger(f"heimdall.pipeline.{name}")
        
    def add_stage(self, stage: ProcessingStage) -> 'Pipeline':
        """Add a processing stage to the pipeline
        
        Args:
            stage: The processing stage to add
            
        Returns:
            The pipeline instance for chaining
        """
        self.stages.append(stage)
        return self
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> Dict[str, Any]:
        """Process an image through the pipeline
        
        Args:
            image: Input image
            context: Processing context
            
        Returns:
            Dictionary with results and context
        """
        if context is None:
            context = {}
            
        # Initialize the context
        context["pipeline_name"] = self.name
        context["start_time"] = time.time()
        context["original_image"] = image
        context["current_image"] = image.copy()
        context["stage_results"] = {}
        context["stage_times"] = {}
        
        try:
            # Process through each stage
            for stage in self.stages:
                context["current_image"] = stage(context["current_image"], context)
                context["stage_results"][stage.name] = context["current_image"].copy()
                
            context["result_image"] = context["current_image"]
            context["success"] = True
            
        except Exception as e:
            self.logger.error(f"Error in pipeline: {str(e)}")
            context["success"] = False
            context["error"] = str(e)
            context["error_stage"] = getattr(stage, "name", "unknown")
            
        finally:
            context["total_time"] = time.time() - context["start_time"]
            self.logger.info(f"Pipeline {self.name} completed in {context['total_time']:.4f}s")
            
        return context


class PipelineFactory:
    """Factory for creating pipelines"""
    
    @staticmethod
    def create_pipeline(name: str, pipeline_type: str, config: Dict[str, Any] = None) -> Pipeline:
        """Create a pipeline based on type
        
        Args:
            name: Name for the pipeline
            pipeline_type: Type of pipeline to create
            config: Configuration for the pipeline
            
        Returns:
            A configured pipeline
            
        Raises:
            ValueError: If the pipeline type is not supported
        """
        pipeline = Pipeline(name, config)
        
        if pipeline_type == "basic":
            # Basic pipeline for testing
            pipeline.add_stage(GrayscaleStage("grayscale"))
            pipeline.add_stage(GaussianBlurStage("blur", {"kernel_size": 5}))
            pipeline.add_stage(CannyEdgeStage("edges"))
            
        elif pipeline_type == "bottle_base":
            # Pipeline for bottle base inspection
            pipeline.add_stage(GrayscaleStage("grayscale"))
            pipeline.add_stage(GaussianBlurStage("blur", {"kernel_size": 5}))
            pipeline.add_stage(AdaptiveThresholdStage("threshold"))
            pipeline.add_stage(MorphologyStage("morphology", {
                "operation": cv2.MORPH_CLOSE,
                "kernel_size": 5
            }))
            pipeline.add_stage(ContourDetectionStage("contours", {
                "min_area": 50,
                "draw": True
            }))
            
        elif pipeline_type == "sidewall":
            # Pipeline for sidewall inspection
            pipeline.add_stage(GrayscaleStage("grayscale"))
            pipeline.add_stage(GaussianBlurStage("blur", {"kernel_size": 3}))
            pipeline.add_stage(CannyEdgeStage("edges", {
                "threshold1": 30,
                "threshold2": 120
            }))
            pipeline.add_stage(HoughLinesStage("lines"))
            
        elif pipeline_type == "preform":
            # Pipeline for preform inspection
            pipeline.add_stage(GrayscaleStage("grayscale"))
            pipeline.add_stage(HistogramEqualizationStage("equalize"))
            pipeline.add_stage(GaussianBlurStage("blur", {"kernel_size": 3}))
            pipeline.add_stage(ThresholdStage("threshold", {
                "method": cv2.THRESH_OTSU
            }))
         
        elif pipeline_type == "contamination":
            # Pipeline optimisé pour la détection d'impuretés/contaminations
            pipeline.add_stage(GrayscaleStage("grayscale"))
            pipeline.add_stage(GaussianBlurStage("blur", {"kernel_size": 3}))  # Flou léger
            pipeline.add_stage(ThresholdStage("threshold", {
                "method": "THRESH_BINARY_INV",  # Inverse les couleurs: noir -> blanc
                "threshold": 50  # Valeur basse pour détecter les zones très sombres
            }))
            pipeline.add_stage(MorphologyStage("morphology", {
                "operation": cv2.MORPH_OPEN,
                "kernel_size": 3,
                "iterations": 1
            }))
        else:
            raise ValueError(f"Unsupported pipeline type: {pipeline_type}")
            
        return pipeline


class GrayscaleStage(ProcessingStage):
    """Convert an image to grayscale"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        if len(image.shape) == 3:
            return cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        return image


class GaussianBlurStage(ProcessingStage):
    """Apply Gaussian blur to an image"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        self.kernel_size = self.config.get("kernel_size", 5)
        self.sigma = self.config.get("sigma", 0)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        return cv2.GaussianBlur(
            image, 
            (self.kernel_size, self.kernel_size), 
            self.sigma
        )


class CannyEdgeStage(ProcessingStage):
    """Apply Canny edge detection to an image"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        self.threshold1 = self.config.get("threshold1", 50)
        self.threshold2 = self.config.get("threshold2", 150)
        self.aperture_size = self.config.get("aperture_size", 3)
        self.L2gradient = self.config.get("L2gradient", False)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        # Convert to grayscale if needed
        if len(image.shape) == 3:
            gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        else:
            gray = image
            
        return cv2.Canny(
            gray, 
            self.threshold1, 
            self.threshold2,
            apertureSize=self.aperture_size,
            L2gradient=self.L2gradient
        )


class AdaptiveThresholdStage(ProcessingStage):
    """Apply adaptive thresholding to an image"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        self.max_value = self.config.get("max_value", 255)
        self.method = getattr(cv2, self.config.get("method", "ADAPTIVE_THRESH_GAUSSIAN_C"))
        self.threshold_type = getattr(cv2, self.config.get("threshold_type", "THRESH_BINARY"))
        self.block_size = self.config.get("block_size", 11)
        self.C = self.config.get("C", 2)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        # Convert to grayscale if needed
        if len(image.shape) == 3:
            gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        else:
            gray = image
            
        return cv2.adaptiveThreshold(
            gray,
            self.max_value,
            self.method,
            self.threshold_type,
            self.block_size,
            self.C
        )


class MorphologyStage(ProcessingStage):
    """Apply morphological operations to an image"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        
        # Get operation type from config
        operation_str = self.config.get("operation", "MORPH_OPEN")
        if isinstance(operation_str, str) and hasattr(cv2, operation_str):
            self.operation = getattr(cv2, operation_str)
        else:
            self.operation = self.config.get("operation", cv2.MORPH_OPEN)
            
        self.kernel_size = self.config.get("kernel_size", 5)
        self.iterations = self.config.get("iterations", 1)
        
        # Create kernel
        kernel_shape = self.config.get("kernel_shape", "rect")
        if kernel_shape == "rect":
            self.kernel = cv2.getStructuringElement(
                cv2.MORPH_RECT, 
                (self.kernel_size, self.kernel_size)
            )
        elif kernel_shape == "ellipse":
            self.kernel = cv2.getStructuringElement(
                cv2.MORPH_ELLIPSE, 
                (self.kernel_size, self.kernel_size)
            )
        elif kernel_shape == "cross":
            self.kernel = cv2.getStructuringElement(
                cv2.MORPH_CROSS, 
                (self.kernel_size, self.kernel_size)
            )
        else:
            self.kernel = np.ones((self.kernel_size, self.kernel_size), np.uint8)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        return cv2.morphologyEx(
            image, 
            self.operation, 
            self.kernel, 
            iterations=self.iterations
        )


class ThresholdStage(ProcessingStage):
    """Apply thresholding to an image"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        self.threshold = self.config.get("threshold", 127)
        self.max_value = self.config.get("max_value", 255)
        
        # Get method from config
        method_str = self.config.get("method", "THRESH_BINARY")
        if isinstance(method_str, str) and hasattr(cv2, method_str):
            self.method = getattr(cv2, method_str)
        else:
            self.method = self.config.get("method", cv2.THRESH_BINARY)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        # Convert to grayscale if needed
        if len(image.shape) == 3:
            gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        else:
            gray = image
            
        if self.method in [cv2.THRESH_OTSU, cv2.THRESH_TRIANGLE]:
            # For Otsu and Triangle methods, we ignore the threshold value
            _, thresholded = cv2.threshold(gray, 0, self.max_value, self.method)
        else:
            _, thresholded = cv2.threshold(gray, self.threshold, self.max_value, self.method)
            
        return thresholded


class ContourDetectionStage(ProcessingStage):
    """Detect contours in an image"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        
        # Get retrieval mode from config
        mode_str = self.config.get("mode", "RETR_EXTERNAL")
        if isinstance(mode_str, str) and hasattr(cv2, mode_str):
            self.mode = getattr(cv2, mode_str)
        else:
            self.mode = self.config.get("mode", cv2.RETR_EXTERNAL)
            
        # Get approximation method from config
        method_str = self.config.get("method", "CHAIN_APPROX_SIMPLE")
        if isinstance(method_str, str) and hasattr(cv2, method_str):
            self.method = getattr(cv2, method_str)
        else:
            self.method = self.config.get("method", cv2.CHAIN_APPROX_SIMPLE)
            
        self.min_area = self.config.get("min_area", 0)
        self.max_area = self.config.get("max_area", float('inf'))
        self.draw = self.config.get("draw", True)
        self.color = tuple(self.config.get("color", (0, 255, 0)))
        self.thickness = self.config.get("thickness", 2)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        if context is None:
            context = {}
            
        # Ensure we have a binary image for contour detection
        if len(image.shape) == 3:
            gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
            _, binary = cv2.threshold(gray, 127, 255, cv2.THRESH_BINARY)
        else:
            binary = image.copy()
            # Ensure binary image has values of 0 or 255
            if np.max(binary) < 255:
                _, binary = cv2.threshold(binary, 0, 255, cv2.THRESH_BINARY | cv2.THRESH_OTSU)
        
        # Find contours
        contours, hierarchy = cv2.findContours(binary, self.mode, self.method)
        
        # Filter contours by area
        filtered_contours = []
        for contour in contours:
            area = cv2.contourArea(contour)
            if self.min_area <= area <= self.max_area:
                filtered_contours.append(contour)
        
        # Store contours in context
        context['contours'] = filtered_contours
        context['contour_count'] = len(filtered_contours)
        
        if not filtered_contours:
            self.logger.debug(f"No contours found (min_area={self.min_area}, max_area={self.max_area})")
        else:
            self.logger.debug(f"Found {len(filtered_contours)} contours")
        
        # Draw contours if requested
        if self.draw:
            # Create a color image for drawing
            if len(image.shape) < 3:
                result = cv2.cvtColor(image, cv2.COLOR_GRAY2BGR)
            else:
                result = image.copy()
                
            cv2.drawContours(
                result, 
                filtered_contours, 
                -1, 
                self.color, 
                self.thickness
            )
            return result
        
        return image


class HoughLinesStage(ProcessingStage):
    """Detect lines using Hough transform"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        self.rho = self.config.get("rho", 1)
        self.theta = self.config.get("theta", np.pi / 180)
        self.threshold = self.config.get("threshold", 100)
        self.min_line_length = self.config.get("min_line_length", 50)
        self.max_line_gap = self.config.get("max_line_gap", 10)
        self.draw = self.config.get("draw", True)
        self.color = tuple(self.config.get("color", (0, 0, 255)))
        self.thickness = self.config.get("thickness", 2)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        if context is None:
            context = {}
            
        # Ensure we have a grayscale image
        if len(image.shape) == 3:
            gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        else:
            gray = image
            
        # Detect lines
        lines = cv2.HoughLinesP(
            gray, 
            self.rho, 
            self.theta, 
            self.threshold, 
            minLineLength=self.min_line_length, 
            maxLineGap=self.max_line_gap
        )
        
        # Store lines in context
        if lines is not None:
            context['lines'] = lines
            context['line_count'] = len(lines)
            self.logger.debug(f"Found {len(lines)} lines")
        else:
            context['lines'] = []
            context['line_count'] = 0
            self.logger.debug("No lines found")
            
        # Draw lines if requested
        if self.draw and lines is not None:
            # Create a color image for drawing
            if len(image.shape) < 3:
                result = cv2.cvtColor(image, cv2.COLOR_GRAY2BGR)
            else:
                result = image.copy()
                
            for line in lines:
                x1, y1, x2, y2 = line[0]
                cv2.line(result, (x1, y1), (x2, y2), self.color, self.thickness)
                
            return result
        
        return image


class HistogramEqualizationStage(ProcessingStage):
    """Apply histogram equalization to enhance contrast"""
    
    def __init__(self, name: str, config: Dict[str, Any] = None):
        super().__init__(name, config)
        self.clahe = self.config.get("clahe", False)
        self.clip_limit = self.config.get("clip_limit", 2.0)
        self.tile_size = self.config.get("tile_size", 8)
        
    def process(self, image: np.ndarray, context: Dict[str, Any] = None) -> np.ndarray:
        # Convert to grayscale if needed
        if len(image.shape) == 3:
            gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        else:
            gray = image
            
        if self.clahe:
            # Apply CLAHE (Contrast Limited Adaptive Histogram Equalization)
            clahe = cv2.createCLAHE(
                clipLimit=self.clip_limit, 
                tileGridSize=(self.tile_size, self.tile_size)
            )
            return clahe.apply(gray)
        else:
            # Apply global histogram equalization
            return cv2.equalizeHist(gray)

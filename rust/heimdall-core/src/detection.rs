use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::types::{PyDict, PyList};
use numpy::{PyArray3, PyReadonlyArray3, IntoPyArray};
use ndarray::{Array3, Axis};
use std::time::Instant;
use std::collections::HashMap;
use log::{info, debug, error};
use thiserror::Error;
use rayon::prelude::*;

#[derive(Debug)]
pub struct Defect {
    pub position: (usize, usize),
    pub size: f64,
    pub confidence: f64,
    pub metadata: HashMap<String, PyObject>,
}

#[derive(Error, Debug)]
pub enum DetectionError {
    #[error("Detection error: {0}")]
    Detection(String),
    
    #[error("Invalid image dimensions: expected 3D array")]
    InvalidDimensions,
}

impl From<DetectionError> for PyErr {
    fn from(err: DetectionError) -> PyErr {
        PyValueError::new_err(err.to_string())
    }
}

/// Find contours in a binary image
#[pyfunction]
pub fn find_contours<'py>(
    py: Python<'py>,
    image: PyReadonlyArray3<u8>,
    min_area: Option<f64>,
    max_area: Option<f64>
) -> PyResult<&'py PyList> {
    let start = Instant::now();
    
    // Convert image to ndarray
    let img_array = image.as_array();
    let (height, width, channels) = (img_array.shape()[0], img_array.shape()[1], img_array.shape()[2]);
    
    // Ensure the image is grayscale or binary
    if channels != 1 {
        return Err(DetectionError::Detection("Contour detection requires a grayscale or binary image".to_string()).into());
    }
    
    // Get parameters
    let min_area = min_area.unwrap_or(10.0);
    let max_area = max_area.unwrap_or(10000.0);
    
    // Find contours (simplified implementation)
    let mut contours = Vec::new();
    let mut visited = vec![vec![false; width]; height];
    
    for i in 0..height {
        for j in 0..width {
            if img_array[[i, j, 0]] > 127 && !visited[i][j] {
                // Found a new contour
                let mut pixels = Vec::new();
                let mut stack = vec![(i, j)];
                visited[i][j] = true;
                
                // Flood fill to find all connected pixels
                while let Some((y, x)) = stack.pop() {
                    pixels.push((y, x));
                    
                    // Check 4-connected neighbors
                    let neighbors = [
                        (y.saturating_sub(1), x),
                        (y + 1, x),
                        (y, x.saturating_sub(1)),
                        (y, x + 1)
                    ];
                    
                    for (ny, nx) in neighbors {
                        if ny < height && nx < width && img_array[[ny, nx, 0]] > 127 && !visited[ny][nx] {
                            stack.push((ny, nx));
                            visited[ny][nx] = true;
                        }
                    }
                }
                
                // Calculate contour properties
                let area = pixels.len() as f64;
                
                if area >= min_area && area <= max_area {
                    // Calculate centroid
                    let sum_y: usize = pixels.iter().map(|(y, _)| *y).sum();
                    let sum_x: usize = pixels.iter().map(|(_, x)| *x).sum();
                    let center_y = sum_y / pixels.len();
                    let center_x = sum_x / pixels.len();
                    
                    // Create contour dictionary
                    let contour_dict = PyDict::new(py);
                    contour_dict.set_item("position", (center_y, center_x))?;
                    contour_dict.set_item("area", area)?;
                    contour_dict.set_item("pixel_count", pixels.len())?;
                    
                    // Add points (simplified for large contours)
                    if pixels.len() <= 100 {
                        let points = PyList::new(py, &pixels.iter().map(|(y, x)| (*y, *x)).collect::<Vec<_>>());
                        contour_dict.set_item("points", points)?;
                    }
                    
                    contours.push(contour_dict);
                }
            }
        }
    }
    
    let duration = start.elapsed();
    debug!("Contour detection took: {:?}", duration);
    
    // Convert to Python list
    let py_contours = PyList::new(py, &contours);
    Ok(py_contours)
}

/// Detect contamination in an image
pub fn detect_contamination(
    image: ndarray::ArrayView3<u8>,
    min_size: f64,
    max_size: f64,
    threshold: f64
) -> Result<Vec<Defect>, DetectionError> {
    let (height, width, channels) = (image.shape()[0], image.shape()[1], image.shape()[2]);
    
    // Create grayscale image if needed
    let mut gray = Array3::<u8>::zeros((height, width, 1));
    
    if channels == 3 {
        // Convert to grayscale
        for i in 0..height {
            for j in 0..width {
                let r = image[[i, j, 0]] as u32;
                let g = image[[i, j, 1]] as u32;
                let b = image[[i, j, 2]] as u32;
                
                // Standard grayscale formula
                let gray_value = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) as u8;
                gray[[i, j, 0]] = gray_value;
            }
        }
    } else if channels == 1 {
        // Already grayscale
        for i in 0..height {
            for j in 0..width {
                gray[[i, j, 0]] = image[[i, j, 0]];
            }
        }
    } else {
        return Err(DetectionError::InvalidDimensions);
    }
    
    // Apply Gaussian blur
    let blur_radius = 2;
    let mut blurred = gray.clone();
    
    for i in blur_radius..(height as i32 - blur_radius) {
        for j in blur_radius..(width as i32 - blur_radius) {
            let mut sum = 0;
            let mut count = 0;
            
            for bi in -blur_radius..=blur_radius {
                for bj in -blur_radius..=blur_radius {
                    let y = (i + bi) as usize;
                    let x = (j + bj) as usize;
                    sum += gray[[y, x, 0]] as u32;
                    count += 1;
                }
            }
            
            blurred[[i as usize, j as usize, 0]] = (sum / count) as u8;
        }
    }
    
    // Apply adaptive thresholding (inverted to detect dark spots)
    let window_size = 11;
    let c = threshold as i32; // Constant subtracted from mean
    let mut binary = Array3::<u8>::zeros((height, width, 1));
    
    for i in 0..height {
        for j in 0..width {
            // Calculate local mean
            let mut sum = 0;
            let mut count = 0;
            
            let start_i = i.saturating_sub(window_size / 2);
            let end_i = std::cmp::min(i + window_size / 2, height - 1);
            let start_j = j.saturating_sub(window_size / 2);
            let end_j = std::cmp::min(j + window_size / 2, width - 1);
            
            for y in start_i..=end_i {
                for x in start_j..=end_j {
                    sum += blurred[[y, x, 0]] as u32;
                    count += 1;
                }
            }
            
            let mean = (sum / count) as i32;
            let pixel_value = blurred[[i, j, 0]] as i32;
            
            // Apply threshold (inverted)
            binary[[i, j, 0]] = if pixel_value < mean - c { 255 } else { 0 };
        }
    }
    
    // Find contours
    let mut defects = Vec::new();
    let mut visited = vec![vec![false; width]; height];
    
    for i in 0..height {
        for j in 0..width {
            if binary[[i, j, 0]] == 255 && !visited[i][j] {
                // Found a new contour
                let mut pixels = Vec::new();
                let mut stack = vec![(i, j)];
                visited[i][j] = true;
                
                // Flood fill to find all connected pixels
                while let Some((y, x)) = stack.pop() {
                    pixels.push((y, x));
                    
                    // Check 4-connected neighbors
                    let neighbors = [
                        (y.saturating_sub(1), x),
                        (y + 1, x),
                        (y, x.saturating_sub(1)),
                        (y, x + 1)
                    ];
                    
                    for (ny, nx) in neighbors {
                        if ny < height && nx < width && binary[[ny, nx, 0]] == 255 && !visited[ny][nx] {
                            stack.push((ny, nx));
                            visited[ny][nx] = true;
                        }
                    }
                }
                
                // Calculate contour properties
                let area = pixels.len() as f64;
                
                if area >= min_size && area <= max_size {
                    // Calculate centroid
                    let sum_y: usize = pixels.iter().map(|(y, _)| *y).sum();
                    let sum_x: usize = pixels.iter().map(|(_, x)| *x).sum();
                    let center_y = sum_y / pixels.len();
                    let center_x = sum_x / pixels.len();
                    
                    // Calculate intensity difference
                    let mut fg_sum = 0;
                    let mut bg_sum = 0;
                    let mut fg_count = 0;
                    let mut bg_count = 0;
                    
                    // Define a region around the contour
                    let margin = 2;
                    let start_i = center_y.saturating_sub(margin);
                    let end_i = std::cmp::min(center_y + margin, height - 1);
                    let start_j = center_x.saturating_sub(margin);
                    let end_j = std::cmp::min(center_x + margin, width - 1);
                    
                    for y in start_i..=end_i {
                        for x in start_j..=end_j {
                            if binary[[y, x, 0]] == 255 {
                                fg_sum += gray[[y, x, 0]] as u32;
                                fg_count += 1;
                            } else {
                                bg_sum += gray[[y, x, 0]] as u32;
                                bg_count += 1;
                            }
                        }
                    }
                    
                    let fg_mean = if fg_count > 0 { fg_sum as f64 / fg_count as f64 } else { 127.0 };
                    let bg_mean = if bg_count > 0 { bg_sum as f64 / bg_count as f64 } else { 127.0 };
                    let intensity_diff = (bg_mean - fg_mean).abs();
                    
                    // Calculate shape score
                    let rect_area = (pixels.iter().map(|(y, _)| *y).max().unwrap_or(0) - 
                                    pixels.iter().map(|(y, _)| *y).min().unwrap_or(0) + 1) *
                                   (pixels.iter().map(|(_, x)| *x).max().unwrap_or(0) - 
                                    pixels.iter().map(|(_, x)| *x).min().unwrap_or(0) + 1);
                    let shape_score = if rect_area > 0 { 1.0 - (area / rect_area as f64) } else { 0.5 };
                    
                    // Calculate confidence
                    let intensity_score = (intensity_diff / 30.0).min(1.0);
                    let confidence = (intensity_score * 0.7) + (shape_score * 0.3);
                    
                    // Create defect if confidence is high enough
                    if confidence >= 0.3 {
                        // Create metadata
                        let mut metadata = HashMap::new();
                        
                        // We'll add metadata in the Python wrapper
                        
                        defects.push(Defect {
                            position: (center_y, center_x),
                            size: area,
                            confidence,
                            metadata,
                        });
                    }
                }
            }
        }
    }
    
    Ok(defects)
}
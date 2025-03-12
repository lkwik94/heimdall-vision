use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyDict;
use numpy::{PyArray3, PyReadonlyArray3, IntoPyArray};
use ndarray::{Array3, Axis};
use std::time::Instant;
use log::{info, debug, error};
use thiserror::Error;
use rayon::prelude::*;

#[derive(Error, Debug)]
pub enum ProcessingError {
    #[error("Image processing error: {0}")]
    Processing(String),
    
    #[error("Invalid image dimensions: expected 3D array")]
    InvalidDimensions,
    
    #[error("OpenCV error: {0}")]
    OpenCV(String),
}

impl From<ProcessingError> for PyErr {
    fn from(err: ProcessingError) -> PyErr {
        PyValueError::new_err(err.to_string())
    }
}

/// Preprocess an image for analysis
#[pyfunction]
pub fn preprocess_image<'py>(
    py: Python<'py>,
    image: PyReadonlyArray3<u8>,
    grayscale: Option<bool>,
    blur_size: Option<i32>
) -> PyResult<&'py PyArray3<u8>> {
    let start = Instant::now();
    
    // Convert image to ndarray
    let img_array = image.as_array();
    let (height, width, channels) = (img_array.shape()[0], img_array.shape()[1], img_array.shape()[2]);
    
    // Create output array
    let mut output = Array3::<u8>::zeros((height, width, if grayscale.unwrap_or(true) { 1 } else { channels }));
    
    // Convert to grayscale if requested
    if grayscale.unwrap_or(true) {
        // Simple grayscale conversion
        for i in 0..height {
            for j in 0..width {
                let r = img_array[[i, j, 0]] as u32;
                let g = img_array[[i, j, 1]] as u32;
                let b = img_array[[i, j, 2]] as u32;
                
                // Standard grayscale formula: 0.299*R + 0.587*G + 0.114*B
                let gray = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) as u8;
                output[[i, j, 0]] = gray;
            }
        }
    } else {
        // Just copy the image
        output.assign(&img_array);
    }
    
    // Apply Gaussian blur if requested
    if let Some(blur_size) = blur_size {
        if blur_size > 0 {
            // Simple box blur implementation
            let blur_radius = blur_size / 2;
            let mut blurred = output.clone();
            
            for i in blur_radius..(height as i32 - blur_radius) {
                for j in blur_radius..(width as i32 - blur_radius) {
                    for c in 0..output.shape()[2] {
                        let mut sum = 0;
                        let mut count = 0;
                        
                        for bi in -blur_radius..=blur_radius {
                            for bj in -blur_radius..=blur_radius {
                                let y = (i + bi) as usize;
                                let x = (j + bj) as usize;
                                sum += output[[y, x, c]] as u32;
                                count += 1;
                            }
                        }
                        
                        blurred[[i as usize, j as usize, c]] = (sum / count) as u8;
                    }
                }
            }
            
            output = blurred;
        }
    }
    
    let duration = start.elapsed();
    debug!("Preprocessing took: {:?}", duration);
    
    // Convert to Python array
    Ok(output.into_pyarray(py))
}

/// Apply thresholding to an image
#[pyfunction]
pub fn apply_threshold<'py>(
    py: Python<'py>,
    image: PyReadonlyArray3<u8>,
    threshold_value: Option<u8>,
    adaptive: Option<bool>,
    inverse: Option<bool>
) -> PyResult<&'py PyArray3<u8>> {
    let start = Instant::now();
    
    // Convert image to ndarray
    let img_array = image.as_array();
    let (height, width, channels) = (img_array.shape()[0], img_array.shape()[1], img_array.shape()[2]);
    
    // Ensure the image is grayscale
    if channels != 1 {
        return Err(ProcessingError::Processing("Thresholding requires a grayscale image".to_string()).into());
    }
    
    // Create output array
    let mut output = Array3::<u8>::zeros((height, width, 1));
    
    // Get parameters
    let threshold = threshold_value.unwrap_or(127);
    let adaptive = adaptive.unwrap_or(false);
    let inverse = inverse.unwrap_or(false);
    
    if adaptive {
        // Simple adaptive thresholding
        let window_size = 11;
        let c = 2; // Constant subtracted from mean
        
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
                        sum += img_array[[y, x, 0]] as u32;
                        count += 1;
                    }
                }
                
                let mean = (sum / count) as i32;
                let pixel_value = img_array[[i, j, 0]] as i32;
                
                // Apply threshold
                if inverse {
                    output[[i, j, 0]] = if pixel_value < mean - c { 255 } else { 0 };
                } else {
                    output[[i, j, 0]] = if pixel_value > mean - c { 255 } else { 0 };
                }
            }
        }
    } else {
        // Simple global thresholding
        for i in 0..height {
            for j in 0..width {
                let pixel_value = img_array[[i, j, 0]];
                
                if inverse {
                    output[[i, j, 0]] = if pixel_value < threshold { 255 } else { 0 };
                } else {
                    output[[i, j, 0]] = if pixel_value > threshold { 255 } else { 0 };
                }
            }
        }
    }
    
    let duration = start.elapsed();
    debug!("Thresholding took: {:?}", duration);
    
    // Convert to Python array
    Ok(output.into_pyarray(py))
}

/// Basic image processing pipeline
pub fn basic_pipeline(image: ndarray::ArrayView3<u8>) -> Result<Array3<u8>, ProcessingError> {
    let (height, width, _) = (image.shape()[0], image.shape()[1], image.shape()[2]);
    
    // Create grayscale image
    let mut gray = Array3::<u8>::zeros((height, width, 1));
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
    
    // Apply thresholding
    let threshold = 127;
    let mut binary = Array3::<u8>::zeros((height, width, 1));
    
    for i in 0..height {
        for j in 0..width {
            binary[[i, j, 0]] = if blurred[[i, j, 0]] > threshold { 255 } else { 0 };
        }
    }
    
    // Convert back to 3-channel image for visualization
    let mut result = Array3::<u8>::zeros((height, width, 3));
    for i in 0..height {
        for j in 0..width {
            let value = binary[[i, j, 0]];
            result[[i, j, 0]] = value;
            result[[i, j, 1]] = value;
            result[[i, j, 2]] = value;
        }
    }
    
    Ok(result)
}

/// Contamination detection pipeline
pub fn contamination_pipeline(image: ndarray::ArrayView3<u8>) -> Result<(Array3<u8>, Vec<(usize, usize, f64)>), ProcessingError> {
    let (height, width, _) = (image.shape()[0], image.shape()[1], image.shape()[2]);
    
    // Create grayscale image
    let mut gray = Array3::<u8>::zeros((height, width, 1));
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
    let c = 15; // Constant subtracted from mean
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
    
    // Find contours (simplified implementation)
    let mut contours = Vec::new();
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
                if pixels.len() >= 3 {
                    // Calculate centroid
                    let sum_y: usize = pixels.iter().map(|(y, _)| *y).sum();
                    let sum_x: usize = pixels.iter().map(|(_, x)| *x).sum();
                    let center_y = sum_y / pixels.len();
                    let center_x = sum_x / pixels.len();
                    
                    // Calculate confidence (simplified)
                    let confidence = 0.75; // Fixed confidence for now
                    
                    contours.push((center_y, center_x, confidence));
                }
            }
        }
    }
    
    // Convert binary to 3-channel image for visualization
    let mut result = Array3::<u8>::zeros((height, width, 3));
    for i in 0..height {
        for j in 0..width {
            let value = binary[[i, j, 0]];
            result[[i, j, 0]] = value;
            result[[i, j, 1]] = value;
            result[[i, j, 2]] = value;
        }
    }
    
    // Draw contours on the result image
    for (y, x, _) in &contours {
        // Draw a small cross at each contour center
        let radius = 3;
        for i in y.saturating_sub(radius)..=std::cmp::min(*y + radius, height - 1) {
            if i < height && *x < width {
                result[[i, *x, 0]] = 0;
                result[[i, *x, 1]] = 0;
                result[[i, *x, 2]] = 255; // Red
            }
        }
        
        for j in x.saturating_sub(radius)..=std::cmp::min(*x + radius, width - 1) {
            if *y < height && j < width {
                result[[*y, j, 0]] = 0;
                result[[*y, j, 1]] = 0;
                result[[*y, j, 2]] = 255; // Red
            }
        }
    }
    
    Ok((result, contours))
}
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use pyo3::types::{PyDict, PyList};
use numpy::{PyArray, PyReadonlyArray2, PyReadonlyArray3, PyArray2, PyArray3};
use ndarray::{Array2, Array3, Axis};
use std::time::Instant;
use log::{info, debug, error};

mod acquisition;
mod processing;
mod detection;

/// Heimdall Core module for Python
#[pymodule]
fn heimdall_core(_py: Python, m: &PyModule) -> PyResult<()> {
    // Initialize logger
    env_logger::init();
    
    // Register functions
    m.add_function(wrap_pyfunction!(process_image, m)?)?;
    m.add_function(wrap_pyfunction!(detect_contamination, m)?)?;
    m.add_function(wrap_pyfunction!(benchmark_processing, m)?)?;
    
    // Register submodules
    let acquisition = PyModule::new(_py, "acquisition")?;
    acquisition.add_function(wrap_pyfunction!(acquisition::acquire_image, m)?)?;
    m.add_submodule(acquisition)?;
    
    let processing = PyModule::new(_py, "processing")?;
    processing.add_function(wrap_pyfunction!(processing::preprocess_image, m)?)?;
    processing.add_function(wrap_pyfunction!(processing::apply_threshold, m)?)?;
    m.add_submodule(processing)?;
    
    let detection = PyModule::new(_py, "detection")?;
    detection.add_function(wrap_pyfunction!(detection::find_contours, m)?)?;
    m.add_submodule(detection)?;
    
    Ok(())
}

/// Process an image using the high-performance Rust pipeline
#[pyfunction]
fn process_image<'py>(
    py: Python<'py>,
    image: PyReadonlyArray3<u8>,
    pipeline_type: &str,
    params: Option<&PyDict>
) -> PyResult<&'py PyDict> {
    let start = Instant::now();
    
    // Convert image to ndarray
    let img_array = image.as_array();
    debug!("Processing image with shape: {:?}", img_array.shape());
    
    // Create result dictionary
    let result = PyDict::new(py);
    
    // Process based on pipeline type
    match pipeline_type {
        "basic" => {
            // Basic processing pipeline
            let processed = processing::basic_pipeline(img_array)?;
            
            // Convert back to Python
            let py_processed = PyArray3::from_array(py, &processed);
            result.set_item("processed_image", py_processed)?;
        },
        "contamination" => {
            // Contamination detection pipeline
            let (processed, contours) = processing::contamination_pipeline(img_array)?;
            
            // Convert back to Python
            let py_processed = PyArray3::from_array(py, &processed);
            result.set_item("processed_image", py_processed)?;
            
            // Convert contours to Python list
            let py_contours = PyList::new(py, &contours);
            result.set_item("contours", py_contours)?;
        },
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                format!("Unsupported pipeline type: {}", pipeline_type)
            ));
        }
    }
    
    // Add timing information
    let duration = start.elapsed();
    result.set_item("processing_time", duration.as_secs_f64())?;
    
    Ok(result)
}

/// Detect contamination in an image
#[pyfunction]
fn detect_contamination<'py>(
    py: Python<'py>,
    image: PyReadonlyArray3<u8>,
    min_size: Option<f64>,
    max_size: Option<f64>,
    threshold: Option<f64>
) -> PyResult<&'py PyDict> {
    let start = Instant::now();
    
    // Set default parameters
    let min_size = min_size.unwrap_or(10.0);
    let max_size = max_size.unwrap_or(3000.0);
    let threshold = threshold.unwrap_or(25.0);
    
    // Convert image to ndarray
    let img_array = image.as_array();
    
    // Detect contamination
    let defects = detection::detect_contamination(img_array, min_size, max_size, threshold)?;
    
    // Create result dictionary
    let result = PyDict::new(py);
    
    // Convert defects to Python list
    let py_defects = PyList::new(py, &[]);
    for defect in defects {
        let py_defect = PyDict::new(py);
        py_defect.set_item("position", (defect.position.0, defect.position.1))?;
        py_defect.set_item("size", defect.size)?;
        py_defect.set_item("confidence", defect.confidence)?;
        
        // Add metadata
        let metadata = PyDict::new(py);
        for (key, value) in defect.metadata {
            metadata.set_item(key, value)?;
        }
        py_defect.set_item("metadata", metadata)?;
        
        py_defects.append(py_defect)?;
    }
    result.set_item("defects", py_defects)?;
    
    // Add timing information
    let duration = start.elapsed();
    result.set_item("processing_time", duration.as_secs_f64())?;
    
    Ok(result)
}

/// Benchmark image processing performance
#[pyfunction]
fn benchmark_processing<'py>(
    py: Python<'py>,
    image: PyReadonlyArray3<u8>,
    iterations: Option<usize>
) -> PyResult<&'py PyDict> {
    let iterations = iterations.unwrap_or(100);
    let img_array = image.as_array();
    
    // Create result dictionary
    let result = PyDict::new(py);
    
    // Benchmark basic pipeline
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = processing::basic_pipeline(img_array)?;
    }
    let basic_duration = start.elapsed();
    
    // Benchmark contamination pipeline
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = processing::contamination_pipeline(img_array)?;
    }
    let contamination_duration = start.elapsed();
    
    // Add results
    result.set_item("basic_pipeline_time", basic_duration.as_secs_f64() / iterations as f64)?;
    result.set_item("contamination_pipeline_time", contamination_duration.as_secs_f64() / iterations as f64)?;
    result.set_item("iterations", iterations)?;
    
    Ok(result)
}
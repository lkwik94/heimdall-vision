# High-Speed Bottle Inspection System Architecture

This document outlines the comprehensive architecture for a high-speed bottle inspection system in Rust, designed to handle 100,000 bottles per hour with 4 GigE cameras at 2MP resolution.

## 1. Component Diagram and Interactions

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                      Bottle Inspection System                                    │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
                                                │
                ┌───────────────────────────────┼───────────────────────────────┐
                │                               │                               │
┌───────────────▼───────────────┐  ┌────────────▼─────────────┐  ┌─────────────▼─────────────────┐
│    Acquisition Subsystem      │  │  Processing Subsystem    │  │     Control & Monitoring      │
│                               │  │                          │  │                               │
│ ┌─────────────────────────┐  │  │ ┌────────────────────┐   │  │ ┌─────────────────────────┐   │
│ │ Camera Manager          │  │  │ │ Image Processor    │   │  │ │ System Monitor          │   │
│ │ - Camera configuration  │  │  │ │ - Pre-processing   │   │  │ │ - Performance metrics   │   │
│ │ - Trigger management    │  │  │ │ - Feature extraction│   │  │ │ - Health checks        │   │
│ │ - Frame acquisition     │◄─┼──┼─┼─► - Defect detection │   │  │ │ - Resource usage       │   │
│ └─────────────────────────┘  │  │ └────────────────────┘   │  │ └─────────────────────────┘   │
│                               │  │           │              │  │               ▲               │
│ ┌─────────────────────────┐  │  │           ▼              │  │               │               │
│ │ Frame Buffer            │  │  │ ┌────────────────────┐   │  │ ┌─────────────▼─────────────┐ │
│ │ - Lock-free ring buffer │◄─┼──┼─┼─► Result Analyzer   │   │  │ │ Configuration Manager     │ │
│ │ - Memory pre-allocation │  │  │ │ - Classification    │   │  │ │ - System configuration    │ │
│ │ - Zero-copy interfaces  │  │  │ │ - Decision making   │◄──┼──┼─┼─► - Camera parameters      │ │
│ └─────────────────────────┘  │  │ └────────────────────┘   │  │ │ - Processing parameters   │ │
│                               │  │           │              │  │ └───────────────────────────┘ │
└───────────────────────────────┘  │           ▼              │  │               ▲               │
                                   │ ┌────────────────────┐   │  │               │               │
                                   │ │ Result Repository  │   │  │ ┌─────────────▼─────────────┐ │
                                   │ │ - Result storage   │◄──┼──┼─┼─► Fault Manager            │ │
                                   │ │ - Statistics       │   │  │ │ - Error detection         │ │
                                   │ │ - Historical data  │   │  │ │ - Recovery strategies     │ │
                                   │ └────────────────────┘   │  │ │ - Degradation policies    │ │
                                   │           │              │  │ └───────────────────────────┘ │
                                   └───────────┼──────────────┘  │               ▲               │
                                               │                 │               │               │
                                               ▼                 │ ┌─────────────▼─────────────┐ │
                                   ┌────────────────────────┐    │ │ External Interface        │ │
                                   │ Communication Gateway  │◄───┼─┼─► - REST API              │ │
                                   │ - IPC mechanisms       │    │ │ - WebSocket notifications │ │
                                   │ - External interfaces  │    │ │ - Metrics export          │ │
                                   └────────────────────────┘    │ └───────────────────────────┘ │
                                                                 └───────────────────────────────┘
```

## 2. Interface Definitions

### 2.1 Camera Interface

```rust
/// Camera configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub id: String,
    pub ip_address: String,
    pub pixel_format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub exposure_time_us: u32,
    pub gain: f32,
    pub frame_rate: f32,
    pub trigger_mode: TriggerMode,
    pub position: CameraPosition,
}

/// Camera position in the inspection system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraPosition {
    Top,
    Bottom,
    Left,
    Right,
}

/// Camera trigger mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerMode {
    Continuous,
    External,
    Software,
}

/// Camera interface trait
#[async_trait]
pub trait Camera: Send + Sync {
    /// Initialize the camera with the given configuration
    async fn initialize(&mut self, config: CameraConfig) -> Result<(), CameraError>;
    
    /// Start image acquisition
    async fn start_acquisition(&mut self) -> Result<(), CameraError>;
    
    /// Stop image acquisition
    async fn stop_acquisition(&mut self) -> Result<(), CameraError>;
    
    /// Trigger a single frame acquisition (for software trigger mode)
    async fn trigger_acquisition(&mut self) -> Result<(), CameraError>;
    
    /// Get the next frame from the camera
    async fn get_frame(&mut self, timeout_ms: u32) -> Result<Frame, CameraError>;
    
    /// Get camera status
    fn get_status(&self) -> CameraStatus;
    
    /// Update camera parameters
    async fn update_parameters(&mut self, params: CameraParameters) -> Result<(), CameraError>;
}

/// Camera implementation for GigE Vision cameras using Aravis
pub struct GigECamera {
    device: Arc<Mutex<aravis::Camera>>,
    stream: Arc<Mutex<aravis::Stream>>,
    config: CameraConfig,
    status: Arc<AtomicCameraStatus>,
    buffer_pool: Arc<FrameBufferPool>,
}
```

### 2.2 Frame and Buffer Interface

```rust
/// Image frame with metadata
#[derive(Clone)]
pub struct Frame {
    /// Unique frame identifier
    pub id: u64,
    
    /// Camera that captured this frame
    pub camera_id: String,
    
    /// Camera position
    pub camera_position: CameraPosition,
    
    /// Timestamp when the frame was captured (nanoseconds since epoch)
    pub timestamp_ns: u64,
    
    /// Frame sequence number from the camera
    pub sequence_number: u64,
    
    /// Image data
    pub image: ImageBuffer,
    
    /// Frame metadata
    pub metadata: FrameMetadata,
}

/// Image buffer with zero-copy capabilities
#[derive(Clone)]
pub struct ImageBuffer {
    /// Image width in pixels
    pub width: u32,
    
    /// Image height in pixels
    pub height: u32,
    
    /// Pixel format
    pub format: PixelFormat,
    
    /// Image data storage
    pub data: Arc<ImageData>,
    
    /// Offset in the data buffer
    pub offset: usize,
    
    /// Stride (bytes per row)
    pub stride: usize,
}

/// Image data storage
pub enum ImageData {
    /// Owned data buffer
    Owned(Vec<u8>),
    
    /// Memory-mapped data buffer
    Mapped(memmap2::Mmap),
    
    /// Shared memory data buffer
    Shared(SharedMemoryBuffer),
}

/// Frame buffer pool for efficient memory management
pub struct FrameBufferPool {
    buffers: Vec<Arc<Mutex<Option<Frame>>>>,
    available: Arc<Semaphore>,
    size: usize,
}

impl FrameBufferPool {
    /// Create a new frame buffer pool with pre-allocated buffers
    pub fn new(size: usize, width: u32, height: u32, format: PixelFormat) -> Self;
    
    /// Get an available buffer from the pool
    pub fn get_buffer(&self, timeout_ms: u32) -> Result<PooledBuffer, BufferError>;
    
    /// Return a buffer to the pool
    pub fn return_buffer(&self, buffer: PooledBuffer);
}

/// Lock-free ring buffer for frame passing between threads
pub struct FrameRingBuffer<T> {
    buffer: Arc<lockfree::queue::Queue<T>>,
    capacity: usize,
}

impl<T: Clone + Send + 'static> FrameRingBuffer<T> {
    /// Create a new ring buffer with the given capacity
    pub fn new(capacity: usize) -> Self;
    
    /// Push an item to the buffer, returns false if buffer is full
    pub fn push(&self, item: T) -> bool;
    
    /// Pop an item from the buffer, returns None if buffer is empty
    pub fn pop(&self) -> Option<T>;
    
    /// Try to pop an item with timeout
    pub fn pop_timeout(&self, timeout_ms: u32) -> Option<T>;
}
```

### 2.3 Processing Pipeline Interface

```rust
/// Processing stage trait
#[async_trait]
pub trait ProcessingStage: Send + Sync {
    type Input;
    type Output;
    type Error;
    
    /// Process a single input and produce an output
    async fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
    
    /// Get the stage name
    fn name(&self) -> &str;
    
    /// Get stage statistics
    fn stats(&self) -> StageStatistics;
}

/// Processing pipeline that connects multiple stages
pub struct Pipeline<I, O, E> {
    stages: Vec<Box<dyn ProcessingStage<Input = I, Output = O, Error = E>>>,
    stats: Arc<PipelineStatistics>,
}

impl<I, O, E> Pipeline<I, O, E> {
    /// Create a new pipeline with the given stages
    pub fn new(stages: Vec<Box<dyn ProcessingStage<Input = I, Output = O, Error = E>>>) -> Self;
    
    /// Process a single input through the pipeline
    pub async fn process(&self, input: I) -> Result<O, PipelineError<E>>;
    
    /// Get pipeline statistics
    pub fn stats(&self) -> PipelineStatistics;
}

/// Image processor for bottle inspection
pub struct BottleInspector {
    preprocessing_pipeline: Pipeline<Frame, ProcessedFrame, ProcessingError>,
    detection_pipeline: Pipeline<ProcessedFrame, DetectionResult, DetectionError>,
    classification_pipeline: Pipeline<DetectionResult, InspectionResult, ClassificationError>,
}

impl BottleInspector {
    /// Create a new bottle inspector with the given configuration
    pub fn new(config: InspectionConfig) -> Self;
    
    /// Process a frame and produce an inspection result
    pub async fn process_frame(&self, frame: Frame) -> Result<InspectionResult, InspectionError>;
    
    /// Update inspection parameters
    pub fn update_parameters(&mut self, params: InspectionParameters) -> Result<(), ConfigError>;
}
```

### 2.4 Result and Statistics Interface

```rust
/// Inspection result for a single bottle
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectionResult {
    /// Unique result identifier
    pub id: Uuid,
    
    /// Bottle identifier
    pub bottle_id: u64,
    
    /// Timestamp when the inspection was performed
    pub timestamp: DateTime<Utc>,
    
    /// Inspection decision
    pub decision: InspectionDecision,
    
    /// Detected defects
    pub defects: Vec<Defect>,
    
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    
    /// Processing time in microseconds
    pub processing_time_us: u64,
    
    /// Original frames used for inspection
    pub frames: HashMap<CameraPosition, FrameReference>,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Inspection decision
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectionDecision {
    Pass,
    Fail(FailureReason),
    Uncertain,
}

/// Failure reason
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureReason {
    Contamination,
    Deformation,
    ColorDeviation,
    SizeDeviation,
    Other,
}

/// Detected defect
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Defect {
    /// Defect type
    pub defect_type: DefectType,
    
    /// Defect location in the image
    pub location: Rect,
    
    /// Defect severity (0.0 - 1.0)
    pub severity: f32,
    
    /// Camera that detected the defect
    pub camera_position: CameraPosition,
    
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
}

/// Production statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProductionStatistics {
    /// Total number of inspected bottles
    pub total_inspected: u64,
    
    /// Number of passed bottles
    pub passed: u64,
    
    /// Number of failed bottles
    pub failed: u64,
    
    /// Number of uncertain results
    pub uncertain: u64,
    
    /// Failure statistics by reason
    pub failures_by_reason: HashMap<FailureReason, u64>,
    
    /// Average processing time in microseconds
    pub avg_processing_time_us: f64,
    
    /// Maximum processing time in microseconds
    pub max_processing_time_us: u64,
    
    /// Minimum processing time in microseconds
    pub min_processing_time_us: u64,
    
    /// Throughput in bottles per second
    pub throughput: f64,
    
    /// Start time of the statistics period
    pub start_time: DateTime<Utc>,
    
    /// End time of the statistics period
    pub end_time: DateTime<Utc>,
}

/// Result repository for storing and retrieving inspection results
#[async_trait]
pub trait ResultRepository: Send + Sync {
    /// Store an inspection result
    async fn store_result(&self, result: InspectionResult) -> Result<(), RepositoryError>;
    
    /// Get an inspection result by ID
    async fn get_result(&self, id: Uuid) -> Result<Option<InspectionResult>, RepositoryError>;
    
    /// Get production statistics for a time period
    async fn get_statistics(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) 
        -> Result<ProductionStatistics, RepositoryError>;
    
    /// Get recent results with pagination
    async fn get_recent_results(&self, limit: usize, offset: usize) 
        -> Result<Vec<InspectionResult>, RepositoryError>;
    
    /// Search results by criteria
    async fn search_results(&self, criteria: SearchCriteria) 
        -> Result<Vec<InspectionResult>, RepositoryError>;
}
```

## 3. Concurrency Model for Real-Time Constraints

```rust
/// Real-time thread configuration
#[derive(Debug, Clone)]
pub struct RtThreadConfig {
    /// Thread priority (0-99, higher is more priority)
    pub priority: u8,
    
    /// CPU core to pin the thread to (-1 for no pinning)
    pub cpu_core: i32,
    
    /// Scheduling policy
    pub policy: SchedPolicy,
    
    /// Memory lock (prevent paging)
    pub memory_lock: bool,
}

/// Scheduling policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedPolicy {
    /// SCHED_OTHER (normal)
    Normal,
    
    /// SCHED_FIFO (real-time, first-in-first-out)
    Fifo,
    
    /// SCHED_RR (real-time, round-robin)
    RoundRobin,
    
    /// SCHED_BATCH (batch scheduling)
    Batch,
    
    /// SCHED_IDLE (idle scheduling)
    Idle,
}

/// Real-time thread builder
pub struct RtThreadBuilder {
    config: RtThreadConfig,
    name: String,
}

impl RtThreadBuilder {
    /// Create a new real-time thread builder with default configuration
    pub fn new(name: &str) -> Self;
    
    /// Set thread priority
    pub fn priority(mut self, priority: u8) -> Self;
    
    /// Set CPU core for thread pinning
    pub fn cpu_core(mut self, cpu_core: i32) -> Self;
    
    /// Set scheduling policy
    pub fn policy(mut self, policy: SchedPolicy) -> Self;
    
    /// Enable or disable memory locking
    pub fn memory_lock(mut self, lock: bool) -> Self;
    
    /// Spawn a thread with the given function
    pub fn spawn<F, T>(self, f: F) -> Result<JoinHandle<T>, RtError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
}

/// Real-time executor for async tasks
pub struct RtExecutor {
    rt: tokio::runtime::Runtime,
    config: RtExecutorConfig,
}

impl RtExecutor {
    /// Create a new real-time executor with the given configuration
    pub fn new(config: RtExecutorConfig) -> Result<Self, RtError>;
    
    /// Spawn a task on the executor
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;
    
    /// Block on a future
    pub fn block_on<F: Future>(&self, future: F) -> F::Output;
}

/// Real-time task scheduler
pub struct RtTaskScheduler {
    executor: Arc<RtExecutor>,
    tasks: HashMap<String, RtTask>,
}

impl RtTaskScheduler {
    /// Create a new real-time task scheduler
    pub fn new(executor: Arc<RtExecutor>) -> Self;
    
    /// Register a periodic task
    pub fn register_periodic_task<F, Fut>(&mut self, name: &str, period_ms: u64, f: F) -> Result<(), RtError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), Error>> + Send + 'static;
    
    /// Register a one-shot task
    pub fn register_oneshot_task<F, Fut>(&mut self, name: &str, delay_ms: u64, f: F) -> Result<(), RtError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), Error>> + Send + 'static;
    
    /// Start all registered tasks
    pub fn start_all(&mut self) -> Result<(), RtError>;
    
    /// Stop all tasks
    pub fn stop_all(&mut self) -> Result<(), RtError>;
}
```

## 4. Communication Mechanisms

```rust
/// Message types for inter-component communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// New frame available
    NewFrame(FrameInfo),
    
    /// Inspection result
    InspectionResult(InspectionResultInfo),
    
    /// System status update
    StatusUpdate(SystemStatus),
    
    /// Configuration change
    ConfigChange(ConfigChangeInfo),
    
    /// Command message
    Command(CommandMessage),
    
    /// Error notification
    Error(ErrorInfo),
}

/// Message broker for pub/sub communication
pub struct MessageBroker {
    topics: HashMap<String, Topic>,
}

impl MessageBroker {
    /// Create a new message broker
    pub fn new() -> Self;
    
    /// Create a new topic
    pub fn create_topic(&mut self, name: &str) -> Result<(), BrokerError>;
    
    /// Get a publisher for a topic
    pub fn get_publisher(&self, topic: &str) -> Result<Publisher, BrokerError>;
    
    /// Subscribe to a topic
    pub fn subscribe(&self, topic: &str) -> Result<Subscriber, BrokerError>;
}

/// Message publisher
pub struct Publisher {
    sender: mpsc::Sender<Message>,
    topic: String,
}

impl Publisher {
    /// Publish a message to the topic
    pub async fn publish(&self, message: Message) -> Result<(), PublishError>;
    
    /// Try to publish a message without waiting
    pub fn try_publish(&self, message: Message) -> Result<(), PublishError>;
}

/// Message subscriber
pub struct Subscriber {
    receiver: mpsc::Receiver<Message>,
    topic: String,
}

impl Subscriber {
    /// Receive the next message
    pub async fn receive(&mut self) -> Option<Message>;
    
    /// Try to receive a message without waiting
    pub fn try_receive(&mut self) -> Option<Message>;
    
    /// Receive messages as a stream
    pub fn into_stream(self) -> impl Stream<Item = Message>;
}

/// IPC channel for inter-process communication
pub struct IpcChannel<T> {
    name: String,
    sender: Option<IpcSender<T>>,
    receiver: Option<IpcReceiver<T>>,
}

impl<T: Serialize + for<'de> Deserialize<'de> + Send + 'static> IpcChannel<T> {
    /// Create a new IPC channel
    pub fn new(name: &str) -> Result<Self, IpcError>;
    
    /// Connect as sender
    pub fn connect_as_sender(&mut self) -> Result<(), IpcError>;
    
    /// Connect as receiver
    pub fn connect_as_receiver(&mut self) -> Result<(), IpcError>;
    
    /// Send a message
    pub async fn send(&self, message: T) -> Result<(), IpcError>;
    
    /// Receive a message
    pub async fn receive(&self) -> Result<T, IpcError>;
}
```

## 5. Resource Management

```rust
/// Memory pool for efficient memory allocation
pub struct MemoryPool {
    chunks: Vec<Arc<Mutex<Option<MemoryChunk>>>>,
    available: Arc<Semaphore>,
    chunk_size: usize,
    total_size: usize,
}

impl MemoryPool {
    /// Create a new memory pool with pre-allocated chunks
    pub fn new(chunk_size: usize, num_chunks: usize) -> Self;
    
    /// Allocate a memory chunk from the pool
    pub fn allocate(&self, timeout_ms: u32) -> Result<PooledMemory, MemoryError>;
    
    /// Get pool statistics
    pub fn stats(&self) -> MemoryPoolStats;
}

/// Thread pool with real-time capabilities
pub struct RtThreadPool {
    inner: rayon::ThreadPool,
    stats: Arc<ThreadPoolStats>,
}

impl RtThreadPool {
    /// Create a new thread pool with real-time configuration
    pub fn new(config: RtThreadPoolConfig) -> Result<Self, ThreadPoolError>;
    
    /// Execute a task on the thread pool
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static;
    
    /// Get thread pool statistics
    pub fn stats(&self) -> ThreadPoolStats;
}

/// I/O manager for efficient I/O operations
pub struct IoManager {
    runtime: tokio::runtime::Runtime,
}

impl IoManager {
    /// Create a new I/O manager
    pub fn new(num_threads: usize) -> Result<Self, IoError>;
    
    /// Spawn an async task on the I/O runtime
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;
    
    /// Create a file with optimized settings
    pub async fn create_file(&self, path: &Path) -> Result<File, IoError>;
    
    /// Open a file with optimized settings
    pub async fn open_file(&self, path: &Path) -> Result<File, IoError>;
}

/// Resource monitor for tracking system resources
pub struct ResourceMonitor {
    cpu_monitor: CpuMonitor,
    memory_monitor: MemoryMonitor,
    disk_monitor: DiskMonitor,
    network_monitor: NetworkMonitor,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new() -> Result<Self, MonitorError>;
    
    /// Start monitoring
    pub fn start(&mut self, interval_ms: u64) -> Result<(), MonitorError>;
    
    /// Stop monitoring
    pub fn stop(&mut self) -> Result<(), MonitorError>;
    
    /// Get current resource usage
    pub fn get_usage(&self) -> ResourceUsage;
    
    /// Get resource usage history
    pub fn get_history(&self, duration: Duration) -> Vec<ResourceUsage>;
}
```

## 6. Fault Tolerance and Recovery

```rust
/// Error types for the inspection system
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Camera error: {0}")]
    Camera(#[from] CameraError),
    
    #[error("Processing error: {0}")]
    Processing(#[from] ProcessingError),
    
    #[error("Detection error: {0}")]
    Detection(#[from] DetectionError),
    
    #[error("Classification error: {0}")]
    Classification(#[from] ClassificationError),
    
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Communication error: {0}")]
    Communication(#[from] CommunicationError),
    
    #[error("System error: {0}")]
    System(#[from] SystemError),
}

/// Fault manager for handling system faults
pub struct FaultManager {
    handlers: HashMap<ErrorType, Vec<Box<dyn FaultHandler>>>,
    error_history: RingBuffer<ErrorEvent>,
    status: Arc<AtomicSystemStatus>,
}

impl FaultManager {
    /// Create a new fault manager
    pub fn new(config: FaultManagerConfig) -> Self;
    
    /// Register a fault handler for a specific error type
    pub fn register_handler<H: FaultHandler + 'static>(&mut self, error_type: ErrorType, handler: H);
    
    /// Handle an error
    pub async fn handle_error(&self, error: Error) -> FaultHandlingResult;
    
    /// Get error history
    pub fn get_error_history(&self) -> Vec<ErrorEvent>;
    
    /// Get current system status
    pub fn get_system_status(&self) -> SystemStatus;
}

/// Fault handler trait
#[async_trait]
pub trait FaultHandler: Send + Sync {
    /// Handle a fault
    async fn handle(&self, error: &Error) -> FaultHandlingResult;
    
    /// Get handler name
    fn name(&self) -> &str;
    
    /// Get handler priority (higher values are executed first)
    fn priority(&self) -> u8;
}

/// Circuit breaker for preventing cascading failures
pub struct CircuitBreaker {
    state: Arc<AtomicCircuitState>,
    failure_threshold: u32,
    reset_timeout: Duration,
    half_open_timeout: Duration,
    failure_counter: Arc<AtomicU32>,
    last_failure: Arc<AtomicTime>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self;
    
    /// Execute a function with circuit breaker protection
    pub async fn execute<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: Future<Output = Result<T, E>> + Send,
        E: Into<Error> + Send;
    
    /// Get current circuit state
    pub fn state(&self) -> CircuitState;
    
    /// Reset the circuit breaker
    pub fn reset(&self);
}

/// Watchdog for detecting system hangs
pub struct Watchdog {
    timeout: Duration,
    last_pet: Arc<AtomicTime>,
    handler: Box<dyn WatchdogHandler>,
    thread: Option<JoinHandle<()>>,
}

impl Watchdog {
    /// Create a new watchdog
    pub fn new(timeout: Duration, handler: impl WatchdogHandler + 'static) -> Self;
    
    /// Start the watchdog
    pub fn start(&mut self) -> Result<(), WatchdogError>;
    
    /// Stop the watchdog
    pub fn stop(&mut self) -> Result<(), WatchdogError>;
    
    /// Pet the watchdog to prevent timeout
    pub fn pet(&self);
}
```

## 7. Complete Data Model

### 7.1 System Configuration

```rust
/// System configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// System name
    pub name: String,
    
    /// System version
    pub version: String,
    
    /// Camera configurations
    pub cameras: Vec<CameraConfig>,
    
    /// Processing configuration
    pub processing: ProcessingConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
    
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// Real-time configuration
    pub realtime: RealTimeConfig,
    
    /// Fault tolerance configuration
    pub fault_tolerance: FaultToleranceConfig,
}

/// Processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Number of processing threads
    pub num_threads: usize,
    
    /// Thread priority
    pub thread_priority: u8,
    
    /// CPU cores to use for processing
    pub cpu_cores: Vec<u32>,
    
    /// Memory allocation size in bytes
    pub memory_allocation: usize,
    
    /// Pipeline configuration
    pub pipeline: PipelineConfig,
    
    /// Inspection parameters
    pub inspection: InspectionParameters,
}

/// Pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Preprocessing stages
    pub preprocessing: Vec<PreprocessingStageConfig>,
    
    /// Detection stages
    pub detection: Vec<DetectionStageConfig>,
    
    /// Classification stages
    pub classification: Vec<ClassificationStageConfig>,
}

/// Real-time configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeConfig {
    /// Scheduling policy
    pub scheduling_policy: String,
    
    /// Thread priorities
    pub thread_priorities: HashMap<String, u8>,
    
    /// CPU affinity
    pub cpu_affinity: HashMap<String, Vec<u32>>,
    
    /// Memory locking
    pub memory_lock: bool,
    
    /// Real-time kernel
    pub rt_kernel: bool,
    
    /// Preemption model
    pub preemption_model: String,
}

/// Fault tolerance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultToleranceConfig {
    /// Error handling strategies
    pub error_handling: HashMap<String, String>,
    
    /// Circuit breaker configurations
    pub circuit_breakers: HashMap<String, CircuitBreakerConfig>,
    
    /// Watchdog configuration
    pub watchdog: WatchdogConfig,
    
    /// Retry policies
    pub retry_policies: HashMap<String, RetryPolicy>,
}
```

### 7.2 Image Representation

```rust
/// Image representation
#[derive(Clone)]
pub struct Image {
    /// Image width in pixels
    pub width: u32,
    
    /// Image height in pixels
    pub height: u32,
    
    /// Pixel format
    pub format: PixelFormat,
    
    /// Image data
    pub data: Arc<ImageData>,
    
    /// Stride (bytes per row)
    pub stride: usize,
}

/// Pixel format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    /// 8-bit grayscale
    Mono8,
    
    /// 16-bit grayscale
    Mono16,
    
    /// 8-bit RGB
    RGB8,
    
    /// 8-bit BGR
    BGR8,
    
    /// 8-bit RGBA
    RGBA8,
    
    /// 8-bit BGRA
    BGRA8,
    
    /// Bayer RG 8-bit
    BayerRG8,
    
    /// Bayer GB 8-bit
    BayerGB8,
    
    /// YUV 4:2:2
    YUV422,
}

/// Region of interest
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Roi {
    /// X coordinate of the top-left corner
    pub x: u32,
    
    /// Y coordinate of the top-left corner
    pub y: u32,
    
    /// Width of the region
    pub width: u32,
    
    /// Height of the region
    pub height: u32,
}

/// Image processing result
#[derive(Clone)]
pub struct ProcessedImage {
    /// Original image
    pub original: Image,
    
    /// Processed image
    pub processed: Image,
    
    /// Processing metadata
    pub metadata: ProcessingMetadata,
}

/// Processing metadata
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProcessingMetadata {
    /// Processing timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Processing duration in microseconds
    pub duration_us: u64,
    
    /// Processing parameters
    pub parameters: HashMap<String, String>,
    
    /// Regions of interest
    pub regions: Vec<Roi>,
    
    /// Processing stages
    pub stages: Vec<ProcessingStageInfo>,
}
```

### 7.3 Bottle Metadata

```rust
/// Bottle metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BottleMetadata {
    /// Bottle identifier
    pub id: u64,
    
    /// Bottle type
    pub bottle_type: BottleType,
    
    /// Production batch
    pub batch: String,
    
    /// Production timestamp
    pub production_time: DateTime<Utc>,
    
    /// Expected dimensions
    pub dimensions: BottleDimensions,
    
    /// Expected color
    pub expected_color: Color,
    
    /// Expected fill level
    pub expected_fill_level: f32,
    
    /// Expected cap type
    pub expected_cap: CapType,
    
    /// Expected label information
    pub expected_label: LabelInfo,
    
    /// Additional metadata
    pub additional: HashMap<String, String>,
}

/// Bottle type
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BottleType {
    Glass,
    PET,
    HDPE,
    Aluminum,
    Other(String),
}

/// Bottle dimensions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BottleDimensions {
    /// Height in millimeters
    pub height_mm: f32,
    
    /// Diameter in millimeters
    pub diameter_mm: f32,
    
    /// Volume in milliliters
    pub volume_ml: f32,
    
    /// Weight in grams
    pub weight_g: f32,
}

/// Cap type
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapType {
    Screw,
    Crown,
    Flip,
    Push,
    None,
    Other(String),
}

/// Label information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabelInfo {
    /// Label type
    pub label_type: LabelType,
    
    /// Label position
    pub position: LabelPosition,
    
    /// Label dimensions
    pub dimensions: Dimensions2D,
    
    /// Label content
    pub content: String,
}

/// Label type
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LabelType {
    Paper,
    Plastic,
    Shrink,
    Direct,
    None,
    Other(String),
}

/// Label position
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LabelPosition {
    Front,
    Back,
    Wrap,
    Top,
    Bottom,
    None,
}
```

### 7.4 Inspection Results

```rust
/// Inspection result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectionResult {
    /// Unique result identifier
    pub id: Uuid,
    
    /// Bottle identifier
    pub bottle_id: u64,
    
    /// Inspection timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Inspection decision
    pub decision: InspectionDecision,
    
    /// Detected defects
    pub defects: Vec<Defect>,
    
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    
    /// Processing time in microseconds
    pub processing_time_us: u64,
    
    /// Frame references
    pub frames: HashMap<CameraPosition, FrameReference>,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Defect type
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefectType {
    /// Foreign object contamination
    ForeignObject,
    
    /// Crack in the bottle
    Crack,
    
    /// Chip in the bottle
    Chip,
    
    /// Deformation of the bottle
    Deformation,
    
    /// Color deviation
    ColorDeviation,
    
    /// Fill level issue
    FillLevel,
    
    /// Cap issue
    CapIssue,
    
    /// Label issue
    LabelIssue,
    
    /// Other defect
    Other(String),
}

/// Frame reference
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrameReference {
    /// Frame identifier
    pub frame_id: u64,
    
    /// Camera position
    pub camera_position: CameraPosition,
    
    /// Storage path
    pub storage_path: Option<String>,
    
    /// Thumbnail
    pub thumbnail: Option<Vec<u8>>,
}

/// Inspection parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectionParameters {
    /// Sensitivity (0.0 - 1.0)
    pub sensitivity: f32,
    
    /// Minimum confidence threshold (0.0 - 1.0)
    pub min_confidence: f32,
    
    /// Maximum allowed defects
    pub max_defects: u32,
    
    /// Defect type thresholds
    pub defect_thresholds: HashMap<DefectType, f32>,
    
    /// Region of interest definitions
    pub regions_of_interest: HashMap<String, Roi>,
    
    /// Feature extraction parameters
    pub feature_extraction: FeatureExtractionParams,
    
    /// Classification parameters
    pub classification: ClassificationParams,
}
```

### 7.5 Production Statistics

```rust
/// Production statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProductionStatistics {
    /// Total number of inspected bottles
    pub total_inspected: u64,
    
    /// Number of passed bottles
    pub passed: u64,
    
    /// Number of failed bottles
    pub failed: u64,
    
    /// Number of uncertain results
    pub uncertain: u64,
    
    /// Failure statistics by reason
    pub failures_by_reason: HashMap<FailureReason, u64>,
    
    /// Defect statistics by type
    pub defects_by_type: HashMap<DefectType, u64>,
    
    /// Average processing time in microseconds
    pub avg_processing_time_us: f64,
    
    /// Maximum processing time in microseconds
    pub max_processing_time_us: u64,
    
    /// Minimum processing time in microseconds
    pub min_processing_time_us: u64,
    
    /// Processing time percentiles
    pub processing_time_percentiles: HashMap<String, u64>,
    
    /// Throughput in bottles per second
    pub throughput: f64,
    
    /// Camera statistics
    pub camera_stats: HashMap<String, CameraStatistics>,
    
    /// Processing stage statistics
    pub stage_stats: HashMap<String, StageStatistics>,
    
    /// Start time of the statistics period
    pub start_time: DateTime<Utc>,
    
    /// End time of the statistics period
    pub end_time: DateTime<Utc>,
}

/// Camera statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CameraStatistics {
    /// Total frames captured
    pub frames_captured: u64,
    
    /// Frames per second
    pub fps: f64,
    
    /// Frame drop rate
    pub drop_rate: f64,
    
    /// Average exposure time in microseconds
    pub avg_exposure_us: f64,
    
    /// Average gain
    pub avg_gain: f64,
    
    /// Average frame size in bytes
    pub avg_frame_size: u64,
    
    /// Camera temperature
    pub temperature: Option<f32>,
    
    /// Camera errors
    pub errors: u64,
}

/// Processing stage statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StageStatistics {
    /// Total items processed
    pub items_processed: u64,
    
    /// Average processing time in microseconds
    pub avg_processing_time_us: f64,
    
    /// Maximum processing time in microseconds
    pub max_processing_time_us: u64,
    
    /// Minimum processing time in microseconds
    pub min_processing_time_us: u64,
    
    /// Processing time percentiles
    pub processing_time_percentiles: HashMap<String, u64>,
    
    /// Error count
    pub errors: u64,
    
    /// Queue size
    pub queue_size: u64,
}
```

This comprehensive architecture design provides a solid foundation for building a high-speed bottle inspection system in Rust. The design prioritizes:

1. **Modularity**: Clear separation of concerns with well-defined interfaces
2. **Real-time performance**: Optimized for high throughput with real-time constraints
3. **Fault tolerance**: Robust error handling and recovery mechanisms
4. **Resource efficiency**: Careful memory and thread management
5. **Flexibility**: Configurable and adaptable to different requirements

The architecture allows for partial updates and maintains real-time performance while processing 100,000 bottles per hour with 4 GigE cameras at 2MP resolution.
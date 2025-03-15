pub mod serial;
pub mod ethernet;
pub mod simulator;

#[cfg(feature = "raspberry_pi")]
pub mod gpio;
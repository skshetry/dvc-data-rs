use std::time::{SystemTime, SystemTimeError};

pub fn unix_time(system_time: SystemTime) -> Result<f64, SystemTimeError> {
    let duration = system_time.duration_since(SystemTime::UNIX_EPOCH)?;
    Ok(duration.as_secs_f64())
}

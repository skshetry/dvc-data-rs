use std::time::{SystemTime, UNIX_EPOCH};

pub fn unix_time(time: SystemTime) -> f64 {
    if time > UNIX_EPOCH {
        let d = time.duration_since(UNIX_EPOCH).unwrap();
        d.as_secs_f64()
    } else {
        let d = UNIX_EPOCH.duration_since(time).unwrap();
        -d.as_secs_f64()
    }
}

use std::time::Duration;

pub const MAX_RETRIES: i32 = 20;
pub const RETRY_DELAY: Duration = Duration::from_millis(5000);

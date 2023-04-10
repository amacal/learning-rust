use chrono::{DateTime, Utc};
use std::fmt::{Error, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TimestampFormatError {
    #[error("Unix timestamp is out of range: {0}")]
    OutOfRange(i64),
    #[error("Format cannot be used for datetime: {0}")]
    Formatting(Error),
}

pub fn format_timestamp(timestamp: i64) -> Result<String, TimestampFormatError> {
    if timestamp == 0 {
        return Ok("---------- --:--:--".to_string());
    }

    let dt = match chrono::NaiveDateTime::from_timestamp_millis(timestamp) {
        Some(dt) => dt,
        None => return Err(TimestampFormatError::OutOfRange(timestamp)),
    };

    let dt = DateTime::<Utc>::from_utc(dt, Utc);
    let format = dt.format("%Y-%m-%d %H:%M:%S");

    let mut formatted = String::new();
    match write!(formatted, "{}", format) {
        Ok(()) => return Ok(formatted),
        Err(error) => return Err(TimestampFormatError::Formatting(error)),
    };
}

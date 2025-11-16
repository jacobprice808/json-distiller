// src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DistillError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON Parsing Error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Invalid Input: {0}")]
    InvalidInput(String),

    #[allow(dead_code)]
    #[error("Hashing Error: {0}")]
    HashingError(String),

    #[error("Internal Error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, DistillError>;
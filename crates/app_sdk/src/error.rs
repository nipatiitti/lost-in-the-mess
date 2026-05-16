#[derive(thiserror::Error, Debug)]
pub enum SdkError {
    #[error("stack: {0}")]
    Stack(#[from] litm_common::Error),
    #[error("serde: {0}")]
    Serde(String),
    #[error("subscriber lagged")]
    Lagged,
    #[error("node not started")]
    NotStarted,
}

pub type Result<T> = std::result::Result<T, SdkError>;

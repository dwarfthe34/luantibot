use thiserror::Error;

#[derive(Debug, Error)]
pub enum BotError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("network error: {0}")]
    Net(String),

    #[error("kicked: {0}")]
    Kicked(String),

    #[error("disconnected")]
    Disconnected,
}

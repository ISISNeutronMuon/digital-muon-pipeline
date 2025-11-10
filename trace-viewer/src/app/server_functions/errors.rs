use cfg_if::cfg_if;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum SessionError {
    #[error("No such session exists.")]
    DoesNotExist,
    #[error("The session's search body has already been taken.")]
    BodyAlreadyTaken,
    #[error("The session's results have not been registered.")]
    ResultsMissing,
    #[error("The session's search was cancelled by the user.")]
    SearchCancelled,
    #[error("The requested trace message does not exist in the cache.")]
    TraceNotFound,
    #[error("The requested channel does not exist in the trace message.")]
    ChannelNotFound,
    #[error("Two cancel requests were made.")]
    AttemptedToCancelTwice,
    #[error("Could not send the cancel signal.")]
    CouldNotSendCancelSignal,
    #[error("Kafka Error Code: {0}")]
    Kafka(String),
    #[error("Search Engine Error: {0}")]
    SearchEngine(String),
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use crate::finder::SearchEngineError;
        use rdkafka::error::KafkaError;

        impl From<KafkaError> for SessionError {
            fn from(value: KafkaError) -> Self {
                Self::Kafka(value.rdkafka_error_code().as_ref().map(ToString::to_string).unwrap_or_default())
            }
        }

        impl From<SearchEngineError> for SessionError {
            fn from(value: SearchEngineError) -> Self {
                Self::SearchEngine(value.to_string())
            }
        }

        impl From<KafkaError> for ServerError {
            fn from(value: KafkaError) -> Self {
                Self::Kafka(value.to_string())
            }
        }

        impl From<SearchEngineError> for ServerError {
            fn from(value: SearchEngineError) -> Self {
                Self::Kafka(value.to_string())
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum ServerError {
    #[error("Cannot get lock on Server Engine. Mutex poisoned.")]
    CannotObtainSessionEngine,
    #[error("{0}")]
    Session(SessionError),
    #[error("{0}")]
    Kafka(String),
    #[error("Kafka Error Code: {0}")]
    SearchEngine(String),
}

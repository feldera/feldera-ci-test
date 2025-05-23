use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Response to a completion token creation request.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CompletionTokenResponse {
    /// Completion token.
    ///
    /// An opaque string associated with the current position in the input stream
    /// generated by an input connector.
    /// Pass this string to the `/completion_status` endpoint to check whether all
    /// inputs associated with the token have been fully processed by the pipeline.
    pub token: String,
}

impl CompletionTokenResponse {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

/// URL-encoded arguments to the `/completion_status` endpoint.
#[derive(Clone, Debug, PartialEq, Deserialize, ToSchema)]
pub struct CompletionStatusArgs {
    /// Completion token returned by the `/completion_token` or `/ingress`
    /// endpoint.
    pub token: String,
}

/// Completion token status returned by the `/completion_status` endpoint.
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum CompletionStatus {
    /// All inputs associated with the token have been processed to completion.
    #[serde(rename = "complete")]
    Complete,

    /// The pipeline is still processing input updates associated with the token.
    #[serde(rename = "inprogress")]
    InProgress,
}

/// Response to a completion token status request.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CompletionStatusResponse {
    /// Completion token status.
    pub status: CompletionStatus,
}

impl CompletionStatusResponse {
    pub fn complete() -> Self {
        Self {
            status: CompletionStatus::Complete,
        }
    }

    pub fn inprogress() -> Self {
        Self {
            status: CompletionStatus::InProgress,
        }
    }
}

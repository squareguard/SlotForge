use serde::Serialize;
use serde_json::Value;

/// Error payload returned to the desktop UI when `ok` is false.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// Uniform success/failure envelope for Tauri commands and the React client.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorBody>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(code: &str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(ApiErrorBody {
                code: code.to_string(),
                message: message.into(),
                details: None,
            }),
        }
    }
}

/// Maps service-layer failures into a serializable API envelope for Tauri/JS.
pub fn from_anyhow<T>(result: anyhow::Result<T>) -> ApiResponse<T> {
    match result {
        Ok(data) => ApiResponse::success(data),
        Err(err) => ApiResponse::failure("INTERNAL", user_visible_error(&err)),
    }
}

/// Prefer the root cause message; avoid multi-line anyhow chains in the UI.
fn user_visible_error(err: &anyhow::Error) -> String {
    err.chain()
        .last()
        .map(|e| e.to_string())
        .unwrap_or_else(|| err.to_string())
}

use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("database initialization failed")]
    DatabaseInit(#[from] sqlx::Error),

    #[error("migration failed")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("not found: {0}")]
    NotFound(String),

    // Part of the documented error contract (docs/IPC_CONTRACT.md); constructed
    // by unique-violation handling and future guards, and exercised in tests.
    #[allow(dead_code)]
    #[error("conflicting record")]
    Conflict,

    #[allow(dead_code)]
    #[error("unsupported in this phase")]
    Unsupported,

    #[allow(dead_code)]
    #[error("internal error")]
    Internal,

    #[error("provider error")]
    Source(#[from] crate::sources::adapter::SourceError),
}

impl AppError {
    pub fn validation<T: Into<String>>(message: T) -> Self {
        AppError::Validation(message.into())
    }

    pub fn not_found<T: Into<String>>(entity: T) -> Self {
        AppError::NotFound(entity.into())
    }

    /// Wire code exposed to the UI; test-only accessor for error-mapping tests.
    #[cfg(test)]
    pub(crate) fn wire_code(&self) -> &'static str {
        self.wire().code
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireError {
    code: &'static str,
    message: String,
    retryable: bool,
}

impl AppError {
    fn wire(&self) -> WireError {
        let (code, message, retryable) = match self {
            // Database init/migration failures: the message stays generic (never
            // raw SQL or paths); the log side carries the diagnostic in dev.
            AppError::DatabaseInit(_) | AppError::Migrate(_) => (
                "DATABASE_INIT_FAILED",
                "Không thể khởi tạo cơ sở dữ liệu local.".to_string(),
                true,
            ),
            AppError::Validation(message) => ("VALIDATION_ERROR", message.clone(), false),
            AppError::NotFound(_) => ("NOT_FOUND", "Bản ghi không tồn tại.".to_string(), false),
            AppError::Conflict => (
                "DB_CONFLICT",
                "Xung đột ràng buộc dữ liệu.".to_string(),
                false,
            ),
            AppError::Unsupported => (
                "UNSUPPORTED_IN_THIS_PHASE",
                "Chức năng chưa khả dụng trong phase này.".to_string(),
                false,
            ),
            AppError::Io(_) | AppError::Internal => (
                "INTERNAL_ERROR",
                "Lỗi nội bộ không xác định.".to_string(),
                false,
            ),
            AppError::Source(source) => (
                source.code(),
                source.safe_message().to_string(),
                matches!(
                    source,
                    crate::sources::adapter::SourceError::Network
                        | crate::sources::adapter::SourceError::RateLimited(_)
                        | crate::sources::adapter::SourceError::Upstream
                ),
            ),
        };
        WireError {
            code,
            message,
            retryable,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.wire().serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of(error: &AppError) -> String {
        let value = serde_json::to_value(error).unwrap();
        value["code"].as_str().unwrap().to_string()
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(code_of(&AppError::Internal), "INTERNAL_ERROR");
        assert_eq!(
            code_of(&AppError::validation("bad bound")),
            "VALIDATION_ERROR"
        );
        assert_eq!(code_of(&AppError::not_found("manga")), "NOT_FOUND");
        assert_eq!(code_of(&AppError::Conflict), "DB_CONFLICT");
        assert_eq!(code_of(&AppError::Unsupported), "UNSUPPORTED_IN_THIS_PHASE");
        assert_eq!(
            code_of(&AppError::Io(std::io::Error::other("x"))),
            "INTERNAL_ERROR"
        );
    }

    #[test]
    fn wire_shape_is_camel_case_and_sanitized() {
        let value = serde_json::to_value(AppError::not_found("manga-1")).unwrap();
        assert_eq!(
            value.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["code", "message", "retryable"]
        );
        assert_eq!(value["message"], "Bản ghi không tồn tại.");
        assert_eq!(value["retryable"], false);
    }
}

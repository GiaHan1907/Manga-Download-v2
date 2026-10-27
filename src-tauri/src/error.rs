use serde::Serialize;
use thiserror::Error;

/// Phase 07 library/reader outcomes. Codes join the documented wire taxonomy;
/// messages are static and user-safe (never paths or provider payloads).
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("library item not found")]
    NotFound,
    #[error("local file missing")]
    FileMissing,
    #[error("local file corrupted")]
    FileCorrupted,
    #[error("invalid cbz archive")]
    CbzInvalid,
    #[error("unsupported image format")]
    UnsupportedImage,
    #[error("page resource not found")]
    PageNotFound,
    #[error("delete failed")]
    DeleteFailed,
    #[error("path outside the managed library root")]
    OutsideRoot,
}

impl LibraryError {
    pub fn code(&self) -> &'static str {
        match self {
            LibraryError::NotFound => "LIBRARY_ITEM_NOT_FOUND",
            LibraryError::FileMissing => "LOCAL_FILE_MISSING",
            LibraryError::FileCorrupted => "LOCAL_FILE_CORRUPTED",
            LibraryError::CbzInvalid => "CBZ_INVALID",
            LibraryError::UnsupportedImage => "UNSUPPORTED_IMAGE_FORMAT",
            LibraryError::PageNotFound => "PAGE_RESOURCE_NOT_FOUND",
            LibraryError::DeleteFailed => "DELETE_FAILED",
            LibraryError::OutsideRoot => "STORAGE_OUTSIDE_LIBRARY_ROOT",
        }
    }

    pub fn safe_message(&self) -> &'static str {
        match self {
            LibraryError::NotFound => "Không tìm thấy mục trong thư viện local.",
            LibraryError::FileMissing => "File đã mất khỏi đĩa — cần tải lại chương này.",
            LibraryError::FileCorrupted => "File lỗi hoặc không đọc được — cần tải lại.",
            LibraryError::CbzInvalid => "Archive CBZ không hợp lệ.",
            LibraryError::UnsupportedImage => "Định dạng ảnh không được hỗ trợ.",
            LibraryError::PageNotFound => "Không tìm thấy trang trong chương.",
            LibraryError::DeleteFailed => "Không xóa được file — giữ nguyên trạng thái hiện tại.",
            LibraryError::OutsideRoot => "Đường dẫn nằm ngoài thư viện được quản lý.",
        }
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("database initialization failed")]
    DatabaseInit(#[from] sqlx::Error),

    #[error("migration failed")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("io error")]
    Io(#[from] std::io::Error),

    /// A disk operation the user can act on (full disk, denied path, removed
    /// drive) — carries the stage name so the UI never answers "lỗi nội bộ
    /// không xác định" to something the user caused and can fix.
    #[error("{0}: {1}")]
    IoStage(String, std::io::Error),

    /// Fetched bytes that are not an accepted image. Distinct from a plain
    /// Validation error because a page fetch may succeed on a second attempt
    /// when a CDN serves a one-off non-image body.
    #[error("{0}")]
    ImageRejected(String),

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

    #[error("library error")]
    Library(#[from] LibraryError),
}

impl AppError {
    pub fn validation<T: Into<String>>(message: T) -> Self {
        AppError::Validation(message.into())
    }

    pub fn not_found<T: Into<String>>(entity: T) -> Self {
        AppError::NotFound(entity.into())
    }

    /// Name the file operation that failed. `Io` itself collapses to
    /// INTERNAL_ERROR on the wire; this keeps the actionable detail.
    pub fn io_stage<T: Into<String>>(stage: T, error: std::io::Error) -> Self {
        AppError::IoStage(stage.into(), error)
    }

    pub fn image_rejected<T: Into<String>>(message: T) -> Self {
        AppError::ImageRejected(message.into())
    }

    /// Wire code exposed to the UI; test-only accessor for error-mapping tests.
    #[cfg(test)]
    pub(crate) fn wire_code(&self) -> &'static str {
        self.wire().code
    }

    /// Persisted error pair for download tasks (safe taxonomy only).
    pub fn wire_pair(&self) -> (&'static str, String) {
        let wire = self.wire();
        (wire.code, wire.message)
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
            AppError::Validation(message) | AppError::ImageRejected(message) => {
                ("VALIDATION_ERROR", message.clone(), false)
            }
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
            AppError::IoStage(stage, error) => (
                "STORAGE_IO",
                format!("{stage}: {error}."),
                // Freeing space or reconnecting a drive fixes it; retrying the
                // same write on its own does not.
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
            AppError::Library(library) => {
                (library.code(), library.safe_message().to_string(), false)
            }
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
        assert_eq!(
            code_of(&AppError::image_rejected(
                "Ảnh trang không hợp lệ (UNSUPPORTED_TYPE)."
            )),
            "VALIDATION_ERROR"
        );
        assert_eq!(
            code_of(&AppError::io_stage(
                "Không ghi được file chương",
                std::io::Error::other("disk full")
            )),
            "STORAGE_IO"
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

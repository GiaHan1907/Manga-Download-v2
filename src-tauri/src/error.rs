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

    #[error("internal error")]
    Internal,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct WireError {
            code: &'static str,
            message: &'static str,
            retryable: bool,
        }

        let wire = match self {
            AppError::DatabaseInit(_) | AppError::Migrate(_) => WireError {
                code: "DATABASE_INIT_FAILED",
                message: "Không thể khởi tạo cơ sở dữ liệu local.",
                retryable: true,
            },
            AppError::Io(_) | AppError::Internal => WireError {
                code: "INTERNAL_ERROR",
                message: "Lỗi nội bộ không xác định.",
                retryable: false,
            },
        };

        wire.serialize(serializer)
    }
}

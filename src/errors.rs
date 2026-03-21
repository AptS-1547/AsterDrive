use actix_web::http::StatusCode;

/// 内部错误类型，字符串错误码（E001-E0xx），用于 Rust 内部、日志、调试
macro_rules! define_errors {
    ($(
        $variant:ident($code:literal, $type_name:literal)
    ),* $(,)?) => {
        #[derive(Debug, Clone)]
        pub enum AsterError {
            $($variant(String),)*
        }

        impl AsterError {
            /// 内部错误码（字符串，如 "E001"），用于日志和调试
            pub fn code(&self) -> &'static str {
                match self {
                    $(AsterError::$variant(_) => $code,)*
                }
            }

            /// 错误类型名称
            pub fn error_type(&self) -> &'static str {
                match self {
                    $(AsterError::$variant(_) => $type_name,)*
                }
            }

            /// 错误详情
            pub fn message(&self) -> &str {
                match self {
                    $(AsterError::$variant(msg) => msg.as_str(),)*
                }
            }
        }

        impl std::fmt::Display for AsterError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}: {}", self.error_type(), self.message())
            }
        }

        impl std::error::Error for AsterError {}

        // snake_case 构造函数
        paste::paste! {
            impl AsterError {
                $(
                    pub fn [<$variant:snake>](msg: impl Into<String>) -> Self {
                        Self::$variant(msg.into())
                    }
                )*
            }
        }
    };
}

define_errors! {
    // ========== E001-E009: 基础设施错误 ==========
    DatabaseConnection(  "E001", "Database Connection Error"),
    DatabaseOperation(   "E002", "Database Operation Error"),
    ConfigError(         "E003", "Configuration Error"),
    InternalError(       "E004", "Internal Server Error"),
    ValidationError(     "E005", "Validation Error"),
    RecordNotFound(      "E006", "Record Not Found"),

    // ========== E010-E019: 认证错误 ==========
    AuthInvalidCredentials("E010", "Invalid Credentials"),
    AuthTokenExpired(      "E011", "Token Expired"),
    AuthTokenInvalid(      "E012", "Token Invalid"),
    AuthForbidden(         "E013", "Forbidden"),

    // ========== E020-E029: 文件错误 ==========
    FileNotFound(         "E020", "File Not Found"),
    FileTooLarge(         "E021", "File Too Large"),
    FileTypeNotAllowed(   "E022", "File Type Not Allowed"),
    FileUploadFailed(     "E023", "Upload Failed"),

    // ========== E030-E039: 存储策略错误 ==========
    StoragePolicyNotFound("E030", "Storage Policy Not Found"),
    StorageDriverError(   "E031", "Storage Driver Error"),
    StorageQuotaExceeded( "E032", "Quota Exceeded"),
    UnsupportedDriver(    "E033", "Unsupported Driver"),

    // ========== E040-E049: 文件夹错误 ==========
    FolderNotFound(       "E040", "Folder Not Found"),

    // ========== E050-E059: 分享错误 ==========
    ShareNotFound(         "E050", "Share Not Found"),
    ShareExpired(          "E051", "Share Expired"),
    SharePasswordRequired( "E052", "Share Password Required"),
    ShareDownloadLimit(    "E053", "Share Download Limit Reached"),

    // ========== E054-E057: 分片上传错误 ==========
    UploadSessionNotFound( "E054", "Upload Session Not Found"),
    UploadSessionExpired(  "E055", "Upload Session Expired"),
    ChunkUploadFailed(     "E056", "Chunk Upload Failed"),
    UploadAssemblyFailed(  "E057", "Upload Assembly Failed"),

    // ========== E058-E058: 缩略图错误 ==========
    ThumbnailGenerationFailed("E058", "Thumbnail Generation Failed"),

    // ========== E059-E059: 资源锁定 ==========
    ResourceLocked("E059", "Resource Locked"),
}

impl AsterError {
    /// HTTP 状态码映射
    pub fn http_status(&self) -> StatusCode {
        match self {
            Self::ValidationError(_)
            | Self::FileTooLarge(_)
            | Self::FileTypeNotAllowed(_)
            | Self::UnsupportedDriver(_) => StatusCode::BAD_REQUEST,

            Self::AuthInvalidCredentials(_)
            | Self::AuthTokenExpired(_)
            | Self::AuthTokenInvalid(_) => StatusCode::UNAUTHORIZED,

            Self::AuthForbidden(_) => StatusCode::FORBIDDEN,

            Self::ResourceLocked(_) => StatusCode::LOCKED,

            Self::RecordNotFound(_)
            | Self::FileNotFound(_)
            | Self::StoragePolicyNotFound(_)
            | Self::FolderNotFound(_)
            | Self::ShareNotFound(_)
            | Self::UploadSessionNotFound(_)
            | Self::ThumbnailGenerationFailed(_) => StatusCode::NOT_FOUND,

            Self::ShareExpired(_) | Self::UploadSessionExpired(_) => StatusCode::GONE,

            Self::SharePasswordRequired(_) | Self::ShareDownloadLimit(_) => StatusCode::FORBIDDEN,

            Self::StorageQuotaExceeded(_) => StatusCode::INSUFFICIENT_STORAGE,

            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<sea_orm::DbErr> for AsterError {
    fn from(e: sea_orm::DbErr) -> Self {
        match e {
            sea_orm::DbErr::RecordNotFound(msg) => Self::RecordNotFound(msg),
            other => Self::DatabaseOperation(other.to_string()),
        }
    }
}

impl actix_web::ResponseError for AsterError {
    fn status_code(&self) -> StatusCode {
        self.http_status()
    }

    fn error_response(&self) -> actix_web::HttpResponse {
        use crate::api::response::ApiResponse;
        let error_code: crate::api::error_code::ErrorCode = self.into();
        actix_web::HttpResponse::build(self.http_status())
            .json(ApiResponse::<()>::error(error_code, self.message()))
    }
}

pub type Result<T> = std::result::Result<T, AsterError>;

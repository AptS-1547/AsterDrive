//! 存储抽象与实现导出。

pub mod driver;
pub mod drivers;
pub mod extensions;
pub mod multipart;
pub mod policy_snapshot;
pub mod registry;

pub use driver::{BlobMetadata, PresignedDownloadOptions, StorageDriver, StoragePathVisitor};
pub use extensions::{ListStorageDriver, PresignedStorageDriver, StreamUploadDriver};
pub use multipart::MultipartStorageDriver;
pub use policy_snapshot::PolicySnapshot;
pub use registry::DriverRegistry;

// 内部 re-export 供宏和错误处理使用
pub(crate) use crate::errors::MapAsterErr;

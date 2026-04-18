//! 文件下载主链路。
//!
//! 下载有两种真正的出站方式：
//! - 服务端自己流式读取并回给客户端
//! - 对满足条件的 S3 附件下载返回 presigned redirect
//!
//! route / scope 层只决定"是否允许下载"，真正的传输策略在这里统一收口。

mod build;
mod response;
mod streaming;
#[cfg(test)]
mod tests;
mod types;

pub(crate) use build::{
    build_download_outcome_with_disposition, build_stream_outcome_with_disposition,
    download_in_scope,
};
pub use build::{download, download_raw};
pub(crate) use response::outcome_to_response;
pub use types::{DownloadOutcome, StreamedFile};

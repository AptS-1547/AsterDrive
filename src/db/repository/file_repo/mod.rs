//! `file_repo` 仓储聚合入口。

mod blob;
mod common;
mod mutation;
mod query;
mod trash;

pub use blob::*;
pub use common::*;
pub use mutation::*;
pub use query::*;
pub use trash::*;

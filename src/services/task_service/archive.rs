mod common;
mod compress;
mod extract;
mod selection;

pub(crate) use compress::create_archive_compress_task_in_scope;
pub(crate) use extract::create_archive_extract_task_in_scope;
pub(crate) use selection::{prepare_archive_download_in_scope, stream_archive_download_in_scope};

use crate::entities::background_task;
use crate::errors::Result;
use crate::runtime::AppState;

pub(super) async fn process_archive_compress_task(
    state: &AppState,
    task: &background_task::Model,
) -> Result<()> {
    compress::process_archive_compress_task(state, task).await
}

pub(super) async fn process_archive_extract_task(
    state: &AppState,
    task: &background_task::Model,
) -> Result<()> {
    extract::process_archive_extract_task(state, task).await
}

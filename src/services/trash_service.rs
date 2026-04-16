mod cleanup;
mod common;
mod listing;
mod models;
mod purge;
mod restore;

pub use cleanup::cleanup_expired;
pub use listing::{list_team_trash, list_trash};
pub use models::{TrashContents, TrashFileCursor, TrashFileItem, TrashFolderItem};
pub use purge::{
    purge_all, purge_all_team, purge_file, purge_folder, purge_team_file, purge_team_folder,
};
pub use restore::{restore_file, restore_folder, restore_team_file, restore_team_folder};

const DEFAULT_RETENTION_DAYS: i64 = 7;
const PURGE_ALL_BATCH_SIZE: u64 = 100;

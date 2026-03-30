pub use sea_orm_migration::prelude::*;

mod m20250320_000001_create_table;
mod m20250321_000001_add_storage_quota;
mod m20250321_000002_create_shares;
mod m20250321_000003_chunked_upload;
mod m20250321_000004_webdav_accounts;
mod m20250321_000005_webdav_root_folder;
mod m20250321_000006_entity_properties;
mod m20250321_000007_soft_delete;
mod m20250321_000008_add_is_locked;
mod m20250321_000009_webdav_locks;
mod m20250321_000010_file_versions;
mod m20250321_000011_resource_locks;
mod m20250321_000012_presigned_upload;
mod m20250322_000001_system_config_metadata;
mod m20250322_000002_config_source_namespace;
mod m20260322_000001_create_audit_logs;
mod m20260323_000001_add_file_size;
mod m20260324_000001_s3_multipart_upload;
mod m20260325_000001_upload_session_file_id;
mod m20260327_000001_add_share_lookup_indexes;
mod m20260327_000002_add_user_preferences;
mod m20260329_000001_create_user_profiles;
mod m20260329_000002_add_user_profile_display_name;
mod m20260329_000003_add_maintenance_indexes;
mod m20260329_000004_create_upload_session_parts;
mod m20260330_000001_add_directory_lookup_indexes;
mod m20260330_000002_add_search_name_indexes;
mod m20260331_000001_add_user_session_version;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250320_000001_create_table::Migration),
            Box::new(m20250321_000001_add_storage_quota::Migration),
            Box::new(m20250321_000002_create_shares::Migration),
            Box::new(m20250321_000003_chunked_upload::Migration),
            Box::new(m20250321_000004_webdav_accounts::Migration),
            Box::new(m20250321_000005_webdav_root_folder::Migration),
            Box::new(m20250321_000006_entity_properties::Migration),
            Box::new(m20250321_000007_soft_delete::Migration),
            Box::new(m20250321_000008_add_is_locked::Migration),
            Box::new(m20250321_000009_webdav_locks::Migration),
            Box::new(m20250321_000010_file_versions::Migration),
            Box::new(m20250321_000011_resource_locks::Migration),
            Box::new(m20250321_000012_presigned_upload::Migration),
            Box::new(m20250322_000001_system_config_metadata::Migration),
            Box::new(m20250322_000002_config_source_namespace::Migration),
            Box::new(m20260322_000001_create_audit_logs::Migration),
            Box::new(m20260323_000001_add_file_size::Migration),
            Box::new(m20260324_000001_s3_multipart_upload::Migration),
            Box::new(m20260325_000001_upload_session_file_id::Migration),
            Box::new(m20260327_000001_add_share_lookup_indexes::Migration),
            Box::new(m20260327_000002_add_user_preferences::Migration),
            Box::new(m20260329_000001_create_user_profiles::Migration),
            Box::new(m20260329_000002_add_user_profile_display_name::Migration),
            Box::new(m20260329_000003_add_maintenance_indexes::Migration),
            Box::new(m20260329_000004_create_upload_session_parts::Migration),
            Box::new(m20260330_000001_add_directory_lookup_indexes::Migration),
            Box::new(m20260330_000002_add_search_name_indexes::Migration),
            Box::new(m20260331_000001_add_user_session_version::Migration),
        ]
    }
}

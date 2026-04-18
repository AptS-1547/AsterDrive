//! CLI 聚合入口。

mod config;
mod database_migration;
mod doctor;
mod shared;

pub use config::{
    ConfigCommand, ConfigCommandReport, DeleteOutput, FileArgs, KeyArgs, KeyValueArgs,
    ValidateArgs, execute_config_command, render_error, render_success,
};
pub use database_migration::{
    DatabaseMigrateArgs, DatabaseMigrateOutputFormat, execute_database_migration,
    render_database_migration_error, render_database_migration_success,
};
pub use doctor::{
    DoctorArgs, DoctorCheck, DoctorReport, DoctorStatus, execute_doctor_command,
    render_doctor_success,
};
pub use shared::{OutputFormat, cli_styles};

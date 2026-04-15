use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Teams {
    Table,
    Id,
    Name,
    Description,
    CreatedBy,
    StorageUsed,
    StorageQuota,
    PolicyGroupId,
    CreatedAt,
    UpdatedAt,
    ArchivedAt,
}

#[derive(DeriveIden)]
enum TeamMembers {
    Table,
    Id,
    TeamId,
    UserId,
    Role,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum StoragePolicyGroups {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Files {
    Table,
    Name,
    FolderId,
    TeamId,
    DeletedAt,
}

#[derive(DeriveIden)]
enum Folders {
    Table,
    Name,
    ParentId,
    TeamId,
    DeletedAt,
}

#[derive(DeriveIden)]
enum UploadSessions {
    Table,
    TeamId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Teams::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Teams::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Teams::Name).string_len(128).not_null())
                    .col(
                        ColumnDef::new(Teams::Description)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(ColumnDef::new(Teams::CreatedBy).big_integer().not_null())
                    .col(
                        ColumnDef::new(Teams::StorageUsed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Teams::StorageQuota)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Teams::PolicyGroupId).big_integer().null())
                    .col(crate::time::utc_date_time_column(manager, Teams::CreatedAt).not_null())
                    .col(crate::time::utc_date_time_column(manager, Teams::UpdatedAt).not_null())
                    .col(crate::time::utc_date_time_column(manager, Teams::ArchivedAt).null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Teams::Table, Teams::CreatedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Teams::Table, Teams::PolicyGroupId)
                            .to(StoragePolicyGroups::Table, StoragePolicyGroups::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_teams_created_by")
                    .table(Teams::Table)
                    .col(Teams::CreatedBy)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_teams_policy_group_id")
                    .table(Teams::Table)
                    .col(Teams::PolicyGroupId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_teams_archived_at")
                    .table(Teams::Table)
                    .col(Teams::ArchivedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TeamMembers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TeamMembers::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TeamMembers::TeamId).big_integer().not_null())
                    .col(ColumnDef::new(TeamMembers::UserId).big_integer().not_null())
                    .col(
                        ColumnDef::new(TeamMembers::Role)
                            .string_len(16)
                            .not_null()
                            .default("member"),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, TeamMembers::CreatedAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, TeamMembers::UpdatedAt)
                            .not_null(),
                    )
                    .check(Expr::col(TeamMembers::Role).is_in(["owner", "admin", "member"]))
                    .foreign_key(
                        ForeignKey::create()
                            .from(TeamMembers::Table, TeamMembers::TeamId)
                            .to(Teams::Table, Teams::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TeamMembers::Table, TeamMembers::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_team_members_team_user")
                    .table(TeamMembers::Table)
                    .col(TeamMembers::TeamId)
                    .col(TeamMembers::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_team_members_user_team")
                    .table(TeamMembers::Table)
                    .col(TeamMembers::UserId)
                    .col(TeamMembers::TeamId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_team_members_team_role")
                    .table(TeamMembers::Table)
                    .col(TeamMembers::TeamId)
                    .col(TeamMembers::Role)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Files::Table)
                    .add_column(ColumnDef::new(Files::TeamId).big_integer().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Folders::Table)
                    .add_column(ColumnDef::new(Folders::TeamId).big_integer().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(UploadSessions::Table)
                    .add_column(ColumnDef::new(UploadSessions::TeamId).big_integer().null())
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_files_team_id")
                        .from(Files::Table, Files::TeamId)
                        .to(Teams::Table, Teams::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_folders_team_id")
                        .from(Folders::Table, Folders::TeamId)
                        .to(Teams::Table, Teams::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_upload_sessions_team_id")
                        .from(UploadSessions::Table, UploadSessions::TeamId)
                        .to(Teams::Table, Teams::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .name("idx_files_team_id")
                    .table(Files::Table)
                    .col(Files::TeamId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_files_team_deleted_folder_name")
                    .table(Files::Table)
                    .col(Files::TeamId)
                    .col(Files::DeletedAt)
                    .col(Files::FolderId)
                    .col(Files::Name)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_folders_team_id")
                    .table(Folders::Table)
                    .col(Folders::TeamId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_folders_team_deleted_parent_name")
                    .table(Folders::Table)
                    .col(Folders::TeamId)
                    .col(Folders::DeletedAt)
                    .col(Folders::ParentId)
                    .col(Folders::Name)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_upload_sessions_team_id")
                    .table(UploadSessions::Table)
                    .col(UploadSessions::TeamId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_upload_sessions_team_id")
                    .table(UploadSessions::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_folders_team_deleted_parent_name")
                    .table(Folders::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_folders_team_id")
                    .table(Folders::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_files_team_deleted_folder_name")
                    .table(Files::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_files_team_id")
                    .table(Files::Table)
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_upload_sessions_team_id")
                        .table(UploadSessions::Table)
                        .to_owned(),
                )
                .await?;
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_folders_team_id")
                        .table(Folders::Table)
                        .to_owned(),
                )
                .await?;
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_files_team_id")
                        .table(Files::Table)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(UploadSessions::Table)
                    .drop_column(UploadSessions::TeamId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Folders::Table)
                    .drop_column(Folders::TeamId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Files::Table)
                    .drop_column(Files::TeamId)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_team_members_team_role")
                    .table(TeamMembers::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_team_members_user_team")
                    .table(TeamMembers::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_team_members_team_user")
                    .table(TeamMembers::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(TeamMembers::Table).to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_teams_archived_at")
                    .table(Teams::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_teams_policy_group_id")
                    .table(Teams::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_teams_created_by")
                    .table(Teams::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Teams::Table).to_owned())
            .await
    }
}

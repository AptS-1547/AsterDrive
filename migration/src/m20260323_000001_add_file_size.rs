use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add size column with default 0
        manager
            .alter_table(
                Table::alter()
                    .table(Files::Table)
                    .add_column(
                        ColumnDef::new(Files::Size)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Backfill from file_blobs
        let db = manager.get_connection();
        db.execute_unprepared(
            "UPDATE files SET size = (SELECT size FROM file_blobs WHERE file_blobs.id = files.blob_id)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Files::Table)
                    .drop_column(Files::Size)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Files {
    Table,
    Size,
}

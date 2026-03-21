use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // files 表加 is_locked
        manager
            .alter_table(
                Table::alter()
                    .table(Files::Table)
                    .add_column(
                        ColumnDef::new(Files::IsLocked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // folders 表加 is_locked
        manager
            .alter_table(
                Table::alter()
                    .table(Folders::Table)
                    .add_column(
                        ColumnDef::new(Folders::IsLocked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Files::Table)
                    .drop_column(Files::IsLocked)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Folders::Table)
                    .drop_column(Folders::IsLocked)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Files {
    Table,
    IsLocked,
}

#[derive(DeriveIden)]
enum Folders {
    Table,
    IsLocked,
}

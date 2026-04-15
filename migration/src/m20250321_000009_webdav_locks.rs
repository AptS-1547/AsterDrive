use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WebdavLocks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WebdavLocks::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WebdavLocks::Token)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(WebdavLocks::Path).string().not_null())
                    .col(ColumnDef::new(WebdavLocks::Principal).string().null())
                    .col(ColumnDef::new(WebdavLocks::OwnerXml).text().null())
                    .col(crate::time::utc_date_time_column(manager, WebdavLocks::TimeoutAt).null())
                    .col(
                        ColumnDef::new(WebdavLocks::Shared)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(WebdavLocks::Deep)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, WebdavLocks::CreatedAt)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_webdav_locks_path")
                    .table(WebdavLocks::Table)
                    .col(WebdavLocks::Path)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WebdavLocks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum WebdavLocks {
    Table,
    Id,
    Token,
    Path,
    Principal,
    OwnerXml,
    TimeoutAt,
    Shared,
    Deep,
    CreatedAt,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EntityProperties::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EntityProperties::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EntityProperties::EntityType)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EntityProperties::EntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EntityProperties::Namespace)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(EntityProperties::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(EntityProperties::Value).text().null())
                    .to_owned(),
            )
            .await?;

        // 唯一索引
        manager
            .create_index(
                Index::create()
                    .name("idx_entity_properties_unique")
                    .table(EntityProperties::Table)
                    .col(EntityProperties::EntityType)
                    .col(EntityProperties::EntityId)
                    .col(EntityProperties::Namespace)
                    .col(EntityProperties::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 查询索引
        manager
            .create_index(
                Index::create()
                    .name("idx_entity_properties_entity")
                    .table(EntityProperties::Table)
                    .col(EntityProperties::EntityType)
                    .col(EntityProperties::EntityId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EntityProperties::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum EntityProperties {
    Table,
    Id,
    EntityType,
    EntityId,
    Namespace,
    Name,
    Value,
}

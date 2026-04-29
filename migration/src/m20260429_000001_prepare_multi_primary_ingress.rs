//! 数据库迁移：为多 primary follower ingress 准备本地隔离命名空间。

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ManagedFollowers {
    Table,
    Namespace,
}

#[derive(DeriveIden)]
enum MasterBindings {
    Table,
    Id,
    StorageNamespace,
    Namespace,
}

#[derive(DeriveIden)]
enum ManagedIngressProfiles {
    Table,
    MasterBindingId,
    ProfileKey,
    IsDefault,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_master_binding_storage_namespace(manager).await?;
        drop_managed_follower_namespace(manager).await?;
        scope_managed_ingress_profiles(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        unscope_managed_ingress_profiles(manager).await?;
        restore_managed_follower_namespace(manager).await?;
        restore_master_binding_namespace(manager).await?;
        Ok(())
    }
}

async fn add_master_binding_storage_namespace(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(MasterBindings::Table)
                .add_column(
                    ColumnDef::new(MasterBindings::StorageNamespace)
                        .string_len(128)
                        .null(),
                )
                .to_owned(),
        )
        .await?;

    backfill_storage_namespaces(manager).await?;
    require_string_column(
        manager,
        MasterBindings::Table,
        MasterBindings::StorageNamespace,
    )
    .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_master_bindings_storage_namespace")
                .table(MasterBindings::Table)
                .col(MasterBindings::StorageNamespace)
                .unique()
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(MasterBindings::Table)
                .drop_column(MasterBindings::Namespace)
                .to_owned(),
        )
        .await
}

async fn drop_managed_follower_namespace(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_managed_followers_namespace")
                .table(ManagedFollowers::Table)
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(ManagedFollowers::Table)
                .drop_column(ManagedFollowers::Namespace)
                .to_owned(),
        )
        .await
}

async fn scope_managed_ingress_profiles(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_managed_ingress_profiles_profile_key")
                .table(ManagedIngressProfiles::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_index(
            Index::drop()
                .name("idx_managed_ingress_profiles_is_default")
                .table(ManagedIngressProfiles::Table)
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(ManagedIngressProfiles::Table)
                .add_column(
                    ColumnDef::new(ManagedIngressProfiles::MasterBindingId)
                        .big_integer()
                        .null(),
                )
                .to_owned(),
        )
        .await?;

    backfill_ingress_profile_binding(manager).await?;
    require_big_integer_column(
        manager,
        ManagedIngressProfiles::Table,
        ManagedIngressProfiles::MasterBindingId,
    )
    .await?;

    if manager.get_database_backend() != DbBackend::Sqlite {
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_managed_ingress_profiles_master_binding_id")
                    .from(
                        ManagedIngressProfiles::Table,
                        ManagedIngressProfiles::MasterBindingId,
                    )
                    .to(MasterBindings::Table, MasterBindings::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;
    }

    manager
        .create_index(
            Index::create()
                .name("idx_managed_ingress_profiles_binding_profile_key")
                .table(ManagedIngressProfiles::Table)
                .col(ManagedIngressProfiles::MasterBindingId)
                .col(ManagedIngressProfiles::ProfileKey)
                .unique()
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_managed_ingress_profiles_binding_default")
                .table(ManagedIngressProfiles::Table)
                .col(ManagedIngressProfiles::MasterBindingId)
                .col(ManagedIngressProfiles::IsDefault)
                .to_owned(),
        )
        .await
}

async fn unscope_managed_ingress_profiles(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_managed_ingress_profiles_binding_default")
                .table(ManagedIngressProfiles::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_index(
            Index::drop()
                .name("idx_managed_ingress_profiles_binding_profile_key")
                .table(ManagedIngressProfiles::Table)
                .to_owned(),
        )
        .await?;

    if manager.get_database_backend() != DbBackend::Sqlite {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_managed_ingress_profiles_master_binding_id")
                    .table(ManagedIngressProfiles::Table)
                    .to_owned(),
            )
            .await?;
    }

    manager
        .alter_table(
            Table::alter()
                .table(ManagedIngressProfiles::Table)
                .drop_column(ManagedIngressProfiles::MasterBindingId)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_managed_ingress_profiles_profile_key")
                .table(ManagedIngressProfiles::Table)
                .col(ManagedIngressProfiles::ProfileKey)
                .unique()
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_managed_ingress_profiles_is_default")
                .table(ManagedIngressProfiles::Table)
                .col(ManagedIngressProfiles::IsDefault)
                .to_owned(),
        )
        .await
}

async fn restore_managed_follower_namespace(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(ManagedFollowers::Table)
                .add_column(
                    ColumnDef::new(ManagedFollowers::Namespace)
                        .string_len(128)
                        .null(),
                )
                .to_owned(),
        )
        .await?;

    fill_namespace_from_id(manager, "managed_followers", "mf").await?;
    require_string_column(
        manager,
        ManagedFollowers::Table,
        ManagedFollowers::Namespace,
    )
    .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_managed_followers_namespace")
                .table(ManagedFollowers::Table)
                .col(ManagedFollowers::Namespace)
                .unique()
                .to_owned(),
        )
        .await
}

async fn restore_master_binding_namespace(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(MasterBindings::Table)
                .add_column(
                    ColumnDef::new(MasterBindings::Namespace)
                        .string_len(128)
                        .null(),
                )
                .to_owned(),
        )
        .await?;

    copy_master_binding_storage_namespace_to_namespace(manager).await?;
    require_string_column(manager, MasterBindings::Table, MasterBindings::Namespace).await?;

    manager
        .drop_index(
            Index::drop()
                .name("idx_master_bindings_storage_namespace")
                .table(MasterBindings::Table)
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(MasterBindings::Table)
                .drop_column(MasterBindings::StorageNamespace)
                .to_owned(),
        )
        .await
}

async fn backfill_storage_namespaces(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = db.get_database_backend();
    let rows = db
        .query_all_raw(Statement::from_string(
            backend,
            "SELECT id FROM master_bindings ORDER BY id".to_string(),
        ))
        .await?;

    if rows.is_empty() {
        return Ok(());
    }

    let mut values = Vec::with_capacity(rows.len() * 2);
    let mut cases = Vec::with_capacity(rows.len());
    let mut ids = Vec::with_capacity(rows.len());
    for row in rows {
        let id = row
            .try_get_by_index::<i64>(0)
            .map_err(|error| DbErr::Migration(format!("read master_bindings.id: {error}")))?;
        let namespace = new_storage_namespace();
        let case_id_bind = bind_param(backend, values.len() + 1)?;
        values.push(id.into());
        let namespace_bind = bind_param(backend, values.len() + 1)?;
        values.push(namespace.into());
        cases.push(format!("WHEN {case_id_bind} THEN {namespace_bind}"));
        ids.push(id);
    }

    let mut id_binds = Vec::with_capacity(ids.len());
    for id in ids {
        let bind = bind_param(backend, values.len() + 1)?;
        values.push(id.into());
        id_binds.push(bind);
    }

    db.execute_raw(Statement::from_sql_and_values(
        backend,
        format!(
            "UPDATE master_bindings \
             SET storage_namespace = CASE id {} END \
             WHERE id IN ({})",
            cases.join(" "),
            id_binds.join(", ")
        ),
        values,
    ))
    .await?;

    Ok(())
}

async fn backfill_ingress_profile_binding(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = db.get_database_backend();
    let binding_id = db
        .query_one_raw(Statement::from_string(
            backend,
            "SELECT id FROM master_bindings ORDER BY is_enabled DESC, id LIMIT 1".to_string(),
        ))
        .await?
        .map(|row| {
            row.try_get_by_index::<i64>(0).map_err(|error| {
                DbErr::Migration(format!("read selected master_bindings.id: {error}"))
            })
        })
        .transpose()?;

    match binding_id {
        Some(id) => {
            let bind = bind_param(backend, 1)?;
            db.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE managed_ingress_profiles SET master_binding_id = {bind} WHERE master_binding_id IS NULL"
                ),
                vec![id.into()],
            ))
            .await?;
        }
        None => {
            // Do not delete managed_ingress_profiles here: a missing
            // master_bindings row means the database is in an unexpected state,
            // and silently deleting ingress profiles would turn a migration
            // repair problem into permanent data loss. A deliberate cleanup, if
            // ever needed, should be documented as its own migration.
        }
    }

    Ok(())
}

async fn fill_namespace_from_id(
    manager: &SchemaManager<'_>,
    table: &str,
    prefix: &str,
) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Postgres => {
            format!("UPDATE {table} SET namespace = '{prefix}_' || id WHERE namespace IS NULL")
        }
        DbBackend::MySql => format!(
            "UPDATE {table} SET namespace = CONCAT('{prefix}_', id) WHERE namespace IS NULL"
        ),
        DbBackend::Sqlite => {
            format!("UPDATE {table} SET namespace = '{prefix}_' || id WHERE namespace IS NULL")
        }
        backend => {
            return Err(DbErr::Migration(format!(
                "unsupported database backend for namespace restore: {backend:?}"
            )));
        }
    };
    db.execute_unprepared(&sql).await?;
    Ok(())
}

async fn copy_master_binding_storage_namespace_to_namespace(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            "UPDATE master_bindings SET namespace = storage_namespace WHERE namespace IS NULL",
        )
        .await?;
    Ok(())
}

async fn require_string_column<T, C>(
    manager: &SchemaManager<'_>,
    table: T,
    column: C,
) -> Result<(), DbErr>
where
    T: IntoIden,
    C: IntoIden,
{
    if manager.get_database_backend() == DbBackend::Sqlite {
        return Ok(());
    }

    manager
        .alter_table(
            Table::alter()
                .table(table)
                .modify_column(ColumnDef::new(column).string_len(128).not_null())
                .to_owned(),
        )
        .await
}

async fn require_big_integer_column<T, C>(
    manager: &SchemaManager<'_>,
    table: T,
    column: C,
) -> Result<(), DbErr>
where
    T: IntoIden,
    C: IntoIden,
{
    if manager.get_database_backend() == DbBackend::Sqlite {
        return Ok(());
    }

    manager
        .alter_table(
            Table::alter()
                .table(table)
                .modify_column(ColumnDef::new(column).big_integer().not_null())
                .to_owned(),
        )
        .await
}

fn new_storage_namespace() -> String {
    format!("mb_{}", uuid::Uuid::new_v4().simple())
}

fn bind_param(backend: DbBackend, index: usize) -> Result<String, DbErr> {
    match backend {
        DbBackend::Postgres => Ok(format!("${index}")),
        DbBackend::MySql | DbBackend::Sqlite => Ok("?".to_string()),
        backend => Err(DbErr::Migration(format!(
            "unsupported database backend for bind param rendering: {backend:?}"
        ))),
    }
}

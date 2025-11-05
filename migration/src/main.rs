use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DbErr};

#[tokio::main]
async fn main() -> Result<(), DbErr> {
    dotenvy::dotenv().ok();
    
    let database_url = std::env::var("DATABASE__URL")
        .expect("DATABASE__URL must be set in environment");
    
    let db = Database::connect(&database_url).await?;
    
    println!("Running migrations...");
    Migrator::up(&db, None).await?;
    println!("Migrations completed successfully!");
    
    Ok(())
}

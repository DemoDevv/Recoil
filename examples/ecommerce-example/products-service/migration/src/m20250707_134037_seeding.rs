use entity::product;
use sea_orm_migration::{
    prelude::*,
    sea_orm::{ActiveModelTrait, ActiveValue::Set},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        product::ActiveModel {
            id: Set(1234),
            name: Set("Test Product".into()),
            price: Set(100.0),
            quantity: Set(10),
        }
        .insert(db)
        .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

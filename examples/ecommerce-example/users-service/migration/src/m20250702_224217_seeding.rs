use entity::user;
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

        user::ActiveModel {
            id: Set(1234),
            email: Set("test@example.com".into()),
            name: Set("Test User".into()),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

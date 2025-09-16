use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Order::Table)
                    .if_not_exists()
                    .col(pk_auto(Order::Id))
                    .col(integer(Order::UserId))
                    .col(
                        ColumnDef::new(Order::CreatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Order::UpdatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OrderItem::Table)
                    .if_not_exists()
                    .col(pk_auto(OrderItem::Id))
                    .col(integer(OrderItem::OrderId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-order-item_id")
                            .from(OrderItem::Table, OrderItem::OrderId)
                            .to(Order::Table, Order::Id),
                    )
                    .col(integer(OrderItem::ProductId))
                    .col(integer(OrderItem::Quantity))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrderItem::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Order::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Order {
    Table,
    Id,
    UserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum OrderItem {
    Table,
    Id,
    OrderId,
    ProductId,
    Quantity,
}

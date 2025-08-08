use std::env;

use anyhow::Result;
use axum::{Router, extract::State, http::StatusCode, routing::get};
use sea_orm::{Database, DatabaseConnection, EntityTrait};

use migration::{Migrator, MigratorTrait};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file");
    let host = env::var("HOST").expect("HOST is not set in .env file");
    let port = env::var("PORT").expect("PORT is not set in .env file");
    let server_url = format!("{host}:{port}");

    let conn = Database::connect(db_url)
        .await
        .expect("Database connection failed");

    Migrator::up(&conn, None).await.unwrap();

    let app = Router::new()
        .route(
            "/health",
            get(|| async { (StatusCode::OK, "Orders service is up") }),
        )
        .route("/orders", get(get_orders))
        .with_state(AppState { db_conn: conn });

    let listener = tokio::net::TcpListener::bind(server_url).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

#[derive(Clone)]
struct AppState {
    db_conn: DatabaseConnection,
}

async fn get_orders(state: State<AppState>) -> Result<(), StatusCode> {
    unimplemented!()
}

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

    // build our application with a single route
    let app = Router::new()
        .route(
            "/health",
            get(|| async { (StatusCode::OK, "Products service is up") }),
        )
        .route("/products", get(get_products))
        .with_state(AppState { db_conn: conn });

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(server_url).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

#[derive(Clone)]
struct AppState {
    db_conn: DatabaseConnection,
}

async fn get_products(state: State<AppState>) -> (StatusCode, String) {
    let products = entity::product::Entity::find()
        .all(&state.db_conn)
        .await
        .expect("Cannot find users in db");

    (StatusCode::OK, serde_json::to_string(&products).unwrap())
}

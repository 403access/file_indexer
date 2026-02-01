use std::env;

use axum::{
    routing::{get, post},
    Router,
};

use file_indexer::api::{create_user, root};
use file_indexer::modules::{
    arguments::check_arguments::check_arguments,
    // commands::commands_loop::commands_loop,
    environment::check_vars::check_vars,
};
use file_indexer::states::app_state;

// fn main() -> io::Result<()> {
#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    check_arguments(&args).unwrap();

    // Load environment variables
    check_vars();
    let path = file_indexer::modules::environment::env_vars::get_cwd();
    let database_url = file_indexer::modules::environment::env_vars::get_database_url();

    // Initialize application state
    app_state::init(path, database_url);

    // commands_loop()

    // initialize tracing
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `POST /users` goes to `create_user`
        .route("/users", post(create_user));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

use axum::{
    body::Bytes,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tracing::{event, span, Level};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let format = tracing_subscriber::fmt::format()
        .compact()
        .with_source_location(true);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .event_format(format)
        .init();

    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `GET /` goes to `root`
        .route("/v1/exec", post(exec));

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

async fn exec(req: Bytes) -> &'static str {
    let span = span!(Level::INFO, "exec");
    let _guard = span.enter();
    event!(Level::DEBUG, "got payload: {:?}", req);
    "Hello, World!"
}

use anyhow::anyhow;
use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Router,
};
use std::{net::SocketAddr, sync::Arc};
use tracing::{event, span, Level};
use tracing_subscriber::EnvFilter;
use wasmtime::{Config, Engine, Instance, Module, Store};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let format = tracing_subscriber::fmt::format()
        .compact()
        .with_source_location(true);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .event_format(format)
        .init();

    let state = {
        let mut conf = Config::new();
        conf.consume_fuel(true);
        State {
            engine: Engine::new(&conf)?,
        }
    };
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `GET /` goes to `root`
        .route("/v1/exec", post(exec))
        .layer(Extension(Arc::new(state)));

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

struct State {
    engine: Engine,
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

async fn exec(state: Extension<Arc<State>>, req: Bytes) -> AppResult<String> {
    let span = span!(Level::INFO, "exec");
    let _guard = span.enter();
    event!(Level::DEBUG, "exec, payload = {:?}", req);

    let module = Module::new(&state.engine, &req)?;

    let mut store = Store::new(&state.engine, 4);
    store.add_fuel(1_000)?;

    let instance = Instance::new(&mut store, &module, &[])?;
    let add = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "add")?;

    let output = add.call(&mut store, (2, 2))?;

    Ok(format!("2 + 2 = {}", output))
}

enum AppError {
    Unknown(anyhow::Error),
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Unknown(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        }
        .into_response()
    }
}

type AppResult<T> = Result<T, AppError>;
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Unknown(e)
    }
}
impl From<wasmtime::Trap> for AppError {
    fn from(e: wasmtime::Trap) -> Self {
        AppError::Unknown(anyhow!("wasm trap: {}", e))
    }
}

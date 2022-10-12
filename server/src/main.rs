use anyhow::anyhow;
use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Router,
};
use std::{net::SocketAddr, sync::{Arc, Mutex}, collections::BTreeMap};
use tracing::{event, span, Level};
use tracing_subscriber::EnvFilter;
use wasmtime::{Caller, Config, Engine, Func, Instance, Module, Store};

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
            store: Default::default(),
        }
    };
    let app = Router::new()
        .route("/readyz", get(readyz))
        .route("/livez", get(livez))
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
    store: Mutex<BTreeMap<i32, i32>>,
}

async fn readyz() -> &'static str {
    "ok"
}

async fn livez() -> &'static str {
    "ok"
}

async fn exec(state: Extension<Arc<State>>, req: Bytes) -> AppResult<String> {
    let span = span!(Level::INFO, "exec");
    let _guard = span.enter();
    event!(Level::DEBUG, "exec, payload = {:?}", req);

    let module = {
        let span = span!(Level::INFO, "compile");
        let _guard = span.enter();
        Module::new(&state.engine, &req)?
    };

    // The store here has access to the application-wide shared BTreeMap, so the
    // wasmtime instance can access and modify shared state. Subsequent API
    // calls will be able to see any modifications.
    let mut store = Store::new(&state.engine, &state.store);
    store.add_fuel(1_000)?;

    let host_log = Func::wrap(&mut store, |caller: Caller<'_, _>, param: i32| {
        event!(
            Level::DEBUG,
            "host_log({}), state = {:?}",
            param,
            caller.data()
        );
    });

    let host_increment = Func::wrap(&mut store, |caller: Caller<'_, &Mutex<BTreeMap<i32, i32>>>, param: i32| {
        let mut store = caller.data().lock().unwrap();
        let value = store.entry(param).or_default();
        *value += 1;
        event!(
            Level::DEBUG,
            "host_increment({}) -> {}",
            param,
            value,
        );
    });

    let instance = Instance::new(&mut store, &module, &[host_log.into(), host_increment.into()])?;
    let add = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "add")?;

    let output = {
        let span = span!(Level::INFO, "call");
        let _guard = span.enter();
        add.call(&mut store, (3, 4))?
    };

    Ok(format!("3 + 4 = {}", output))
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

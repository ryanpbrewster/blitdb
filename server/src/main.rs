use anyhow::anyhow;
use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Extension, Router, extract::Path,
};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{Arc, Mutex}, str::Utf8Error, num::ParseIntError,
};
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
        .route("/v1/get/:key", get(get_by_key))
        .route("/v1/set/:key", post(set_by_key))
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

async fn get_by_key(state: Extension<Arc<State>>, Path(key): Path<i32>) -> AppResult<String> {
    Ok(state.store.lock().unwrap().get(&key).cloned().unwrap_or_default().to_string())
}

async fn set_by_key(state: Extension<Arc<State>>, Path(key): Path<i32>, value: Bytes) -> AppResult<String> {
    let v: i32 = std::str::from_utf8(&value)?.parse()?;
    Ok(state.store.lock().unwrap().insert(key, v).unwrap_or_default().to_string())
}

async fn exec(state: Extension<Arc<State>>, req: Bytes) -> AppResult<String> {
    let span = span!(Level::INFO, "exec");
    let _guard = span.enter();
    event!(Level::DEBUG, "exec, payload = {} bytes", req.len());

    let module = {
        let span = span!(Level::INFO, "compile");
        let _guard = span.enter();
        Module::new(&state.engine, &req)?
    };

    // The store here has access to the application-wide shared BTreeMap, so the
    // wasmtime instance can access and modify shared state. Subsequent API
    // calls will be able to see any modifications.
    let mut store = Store::new(&state.engine, &state.store);
    store.add_fuel(1_000_000)?;

    let host_get = Func::wrap(
        &mut store,
        |caller: Caller<'_, &Mutex<BTreeMap<i32, i32>>>, key: i32| {
            let span = span!(Level::INFO, "host_get");
            let _guard = span.enter();
            let value = caller
                .data()
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .unwrap_or_default();
            event!(Level::DEBUG, "host_get({}) = {}", key, value);
            value
        },
    );

    let host_set = Func::wrap(
        &mut store,
        |caller: Caller<'_, &Mutex<BTreeMap<i32, i32>>>, key: i32, value: i32| {
            let span = span!(Level::INFO, "host_set");
            let _guard = span.enter();
            event!(Level::DEBUG, "host_set({}, {})", key, value,);
            caller
                .data()
                .lock()
                .unwrap()
                .insert(key, value)
                .unwrap_or_default()
        },
    );

    let mut imports = Vec::new();
    for import in module.imports() {
        match import.name() {
            "host_get" => imports.push(host_get.into()),
            "host_set" => imports.push(host_set.into()),
            _ => {},
        };
    }
    let instance = Instance::new(&mut store, &module, &imports)?;
    let add = instance.get_typed_func::<(), i32, _>(&mut store, "exec")?;

    let output = {
        let span = span!(Level::INFO, "call");
        let _guard = span.enter();
        add.call(&mut store, ())?
    };

    Ok(output.to_string())
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
impl From<Utf8Error> for AppError {
    fn from(e: Utf8Error) -> Self {
        AppError::Unknown(e.into())
    }
}
impl From<ParseIntError> for AppError {
    fn from(e: ParseIntError) -> Self {
        AppError::Unknown(e.into())
    }
}
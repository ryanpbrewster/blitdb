use anyhow::anyhow;
use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Router,
};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
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
    store: Mutex<BTreeMap<Vec<u8>, Vec<u8>>>,
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

    let host_log = Func::wrap(
        &mut store,
        |mut caller: Caller<'_, _>, ptr: u32, len: u32| {
            let span = span!(Level::INFO, "host_log");
            let _guard = span.enter();
            event!(Level::DEBUG, "host_log({}, {})", ptr, len,);
            let memory = match caller.get_export("memory") {
                None => return 0,
                Some(m) => m,
            };
            let memory = match memory.into_memory() {
                None => return 0,
                Some(m) => m,
            };
            let x = &memory.data(&caller)[ptr as usize..(ptr + len) as usize];
            event!(Level::DEBUG, "{:?}", std::str::from_utf8(x));
            1
        },
    );

    let host_get = Func::wrap(
        &mut store,
        |mut caller: Caller<'_, &Mutex<BTreeMap<Vec<u8>, Vec<u8>>>>,
         key_ptr: u32,
         key_len: u32,
         value_ptr: u32,
         value_len: u32| {
            let span = span!(Level::INFO, "host_get");
            let _guard = span.enter();
            event!(
                Level::DEBUG,
                "host_get({}, {}, {}, {})",
                key_ptr,
                key_len,
                value_ptr,
                value_len
            );
            let memory = match caller.get_export("memory") {
                None => return 0,
                Some(m) => m,
            };
            let memory = match memory.into_memory() {
                None => return 0,
                Some(m) => m,
            };
            let key = &memory.data(&caller)[key_ptr as usize..(key_ptr + key_len) as usize];

            let data = caller.data().lock().unwrap();
            let value = match data.get(key) {
                None => return 0,
                Some(v) => v,
            };
            event!(Level::DEBUG, "{:?} = {:?}", key, value);

            let dst = &mut memory.data_mut(&mut caller)
                [value_ptr as usize..(value_ptr + value_len) as usize];
            let len = std::cmp::min(dst.len(), value.len());
            dst[..len].copy_from_slice(&value[..len]);
            len as u32
        },
    );

    let host_set = Func::wrap(
        &mut store,
        |mut caller: Caller<'_, &Mutex<BTreeMap<Vec<u8>, Vec<u8>>>>,
         key_ptr: u32,
         key_len: u32,
         value_ptr: u32,
         value_len: u32| {
            let span = span!(Level::INFO, "host_set");
            let _guard = span.enter();
            event!(
                Level::DEBUG,
                "host_set({}, {}, {}, {})",
                key_ptr,
                key_len,
                value_ptr,
                value_len
            );
            let memory = match caller.get_export("memory") {
                None => return 0,
                Some(m) => m,
            };
            let memory = match memory.into_memory() {
                None => return 0,
                Some(m) => m,
            };
            let key_range = key_ptr as usize..(key_ptr + key_len) as usize;
            let value_range = value_ptr as usize..(value_ptr + value_len) as usize;
            let key = memory.data(&caller)[key_range].to_vec();
            let value = memory.data(&caller)[value_range].to_vec();
            event!(Level::DEBUG, "setting {:?} = {:?}", key, value);
            caller.data().lock().unwrap().insert(key, value);
            1
        },
    );

    let instance = Instance::new(
        &mut store,
        &module,
        &[host_log.into(), host_get.into(), host_set.into()],
    )?;
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

use axum::{
    body::Bytes,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
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

async fn exec(req: Bytes) -> String {
    let span = span!(Level::INFO, "exec");
    let _guard = span.enter();
    event!(Level::DEBUG, "got payload: {:?}", req);

    let mut conf = Config::new();
    conf.consume_fuel(true);
    let engine = match Engine::new(&conf) {
        Ok(e) => e,
        Err(e) => return format!("invalid conf: {}", e),
    };
    let module = match Module::new(&engine, &req) {
        Ok(m) => m,
        Err(e) => return format!("error: {}", e),
    };

    let mut store = Store::new(&engine, 4);
    store
        .add_fuel(1_000)
        .expect("store.add_fuel should never error");

    let instance = match Instance::new(&mut store, &module, &[]) {
        Ok(i) => i,
        Err(e) => return format!("error: {}", e),
    };
    let add = match instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "add") {
        Ok(h) => h,
        Err(e) => return format!("error: {}", e),
    };

    // And finally we can call the wasm!
    let output = match add.call(&mut store, (2, 2)) {
        Ok(o) => o,
        Err(e) => return format!("error: {}", e),
    };

    format!("2 + 2 = {}", output)
}

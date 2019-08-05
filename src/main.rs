extern crate actix;
extern crate actix_web;
extern crate env_logger;
extern crate failure;
extern crate futures;
#[macro_use]
extern crate log;
#[macro_use]
extern crate maplit;
extern crate serde;
extern crate serde_derive;
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate prometheus;

mod graph;
mod metadata;
mod metrics;
mod scraper;

use actix::prelude::*;
use actix_web::{http::Method, middleware::Logger, server, App};
use actix_web::{HttpRequest, HttpResponse};
use failure::{Error, Fallible};
use futures::prelude::*;
use prometheus::IntCounter;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use structopt::StructOpt;

lazy_static::lazy_static! {
    static ref V1_GRAPH_INCOMING_REQS: IntCounter = register_int_counter!(opts!(
        "dumnati_v1_graph_incoming_requests_total",
        "Total number of incoming HTTP client request to /v1/graph"
    ))
    .unwrap();
    static ref UNIQUE_IDS: IntCounter = register_int_counter!(opts!(
        "dumnati_v1_graph_unique_uuids_total",
        "Total number of unique node UUIDs (per-instance Bloom filter)."
    ))
    .unwrap();
}

fn main() -> Fallible<()> {
    env_logger::Builder::from_default_env().try_init()?;

    let opts = CliOptions::from_args();
    trace!("starting with config: {:#?}", opts);

    let sys = actix::System::new("dumnati");
    let (port, _param, _path) = opts.split();

    let scraper_addr = scraper::Scraper::new("testing")?.start();

    let node_population = Arc::new(cbloom::Filter::new(10 * 1024 * 1024, 1_000_000));
    let app_state = AppState {
        scraper_addr,
        population: Arc::clone(&node_population),
    };

    server::new(move || {
        App::with_state(app_state.clone())
            .middleware(Logger::default())
            .route("/v1/graph", Method::GET, serve_graph)
            .route(
                "/private-will-move/metrics",
                Method::GET,
                metrics::serve_metrics,
            )
    })
    .bind((IpAddr::from(Ipv4Addr::UNSPECIFIED), port))?
    .start();

    sys.run();
    Ok(())
}

#[derive(Clone, Debug)]
pub(crate) struct AppState {
    scraper_addr: Addr<scraper::Scraper>,
    population: Arc<cbloom::Filter>,
}

pub(crate) fn serve_graph(
    req: HttpRequest<AppState>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    record_metrics(&req);

    let uuid = req
        .query()
        .get("node_uuid")
        .map(String::from)
        .unwrap_or_default();
    let wariness = {
        // Left limit not included in range.
        const COMPUTED_MIN: f64 = 0.0 + 0.000001;
        const COMPUTED_MAX: f64 = 1.0;
        let mut hasher = DefaultHasher::new();
        uuid.hash(&mut hasher);
        let digest = hasher.finish();
        // Scale down.
        let scaled = (digest as f64) / (std::u64::MAX as f64);
        // Clamp within limits.
        scaled.max(COMPUTED_MIN).min(COMPUTED_MAX)
    };

    let cached_graph = req
        .state()
        .scraper_addr
        .send(scraper::GetCachedGraph::default())
        .flatten();

    let resp = cached_graph
        .map(move |graph| graph.throttle_rollouts(wariness))
        .map(|graph| graph.filter_deadends())
        .and_then(|graph| {
            serde_json::to_string_pretty(&graph).map_err(|e| failure::format_err!("{}", e))
        })
        .map(|json| {
            HttpResponse::Ok()
                .content_type("application/json")
                .body(json)
        });

    Box::new(resp)
}

pub(crate) fn record_metrics(req: &HttpRequest<AppState>) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    V1_GRAPH_INCOMING_REQS.inc();

    let population = &req.state().population;
    if let Some(uuid) = req.query().get("node_uuid") {
        let mut hasher = DefaultHasher::default();
        uuid.hash(&mut hasher);
        let client_uuid = hasher.finish();
        if !population.maybe_contains(client_uuid) {
            population.insert(client_uuid);
            UNIQUE_IDS.inc();
        }
    }
}

#[derive(Debug, StructOpt)]
pub(crate) struct CliOptions {
    /// Port to which the server will bind.
    #[structopt(short = "p", long = "port", default_value = "9876")]
    port: u16,

    /// Client parameter for current version.
    #[structopt(short = "c", long = "client-parameter", default_value = "current_os")]
    client_param: String,

    /// Path to release payload.
    #[structopt(parse(from_str))]
    payload: String,
}

impl CliOptions {
    pub(crate) fn split(self) -> (u16, String, String) {
        (self.port, self.client_param, self.payload)
    }
}

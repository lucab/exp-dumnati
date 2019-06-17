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

use actix_web::{http::Method, middleware::Logger, server, App};
use actix_web::{HttpRequest, HttpResponse};
use failure::{Error, Fallible};
use futures::future;
use futures::prelude::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use structopt::StructOpt;

fn main() -> Fallible<()> {
    env_logger::Builder::from_default_env().try_init()?;

    let opts = CliOptions::from_args();
    trace!("starting with config: {:#?}", opts);

    let sys = actix::System::new("dumnati");
    let (port, param, path) = opts.split();
    let payload = CincinnatiPayload::from_file(path)?;
    let app_state = AppState { param, payload };

    server::new(move || {
        App::with_state(app_state.clone())
            .middleware(Logger::default())
            .route("/v1/graph", Method::GET, serve_graph)
    })
    .bind((IpAddr::from(Ipv4Addr::UNSPECIFIED), port))?
    .start();

    sys.run();
    Ok(())
}

#[derive(Clone, Debug)]
pub(crate) struct AppState {
    param: String,
    payload: CincinnatiPayload,
}

pub(crate) fn serve_graph(
    req: HttpRequest<AppState>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    // Get current client version.
    let param = &req.state().param;
    let os = match req.query().get(param) {
        None => {
            return Box::new(future::ok(HttpResponse::BadRequest().finish()));
        }
        Some(v) => v.to_string(),
    };
    trace!("client request, running OS: {}", os);

    // Assemble a simple graph.
    let current = CincinnatiPayload {
        version: "client-os-version".to_string(),
        payload: os,
        metadata: hashmap!{
            "org.fedoraproject.coreos.scheme".to_string() => "checksum".to_string(),
        },
    };
    let next = req.state().payload.clone();
    let graph = Graph {
        nodes: vec![current, next],
        edges: vec![(0, 1)],
    };

    // Return the graph as JSON.
    let resp = future::result(serde_json::to_string_pretty(&graph))
        .from_err()
        .map(|json| {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(json)
    });

    Box::new(resp)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Graph {
    pub(crate) nodes: Vec<CincinnatiPayload>,
    pub(crate) edges: Vec<(u64, u64)>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CincinnatiPayload {
    pub(crate) version: String,
    pub(crate) metadata: HashMap<String, String>,
    pub(crate) payload: String,
}

impl CincinnatiPayload {
    pub(crate) fn from_file<S: AsRef<str>>(path: S) -> Fallible<Self> {
        let fp = std::fs::File::open(path.as_ref())?;
        let payload = serde_json::from_reader(fp)?;
        Ok(payload)
    }
}

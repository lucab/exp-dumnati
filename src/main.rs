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

mod graph;
mod scraper;

use actix::prelude::*;
use actix_web::{http::Method, middleware::Logger, server, App};
use actix_web::{HttpRequest, HttpResponse};
use failure::{Error, Fallible};
use futures::prelude::*;
use std::net::{IpAddr, Ipv4Addr};
use structopt::StructOpt;

fn main() -> Fallible<()> {
    env_logger::Builder::from_default_env().try_init()?;

    let opts = CliOptions::from_args();
    trace!("starting with config: {:#?}", opts);

    let sys = actix::System::new("dumnati");
    let (port, _param, _path) = opts.split();

    let scraper_addr = scraper::Scraper::new("testing")?.start();

    let app_state = AppState { scraper_addr };

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
    scraper_addr: Addr<scraper::Scraper>,
}

pub(crate) fn serve_graph(
    req: HttpRequest<AppState>,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let resp = req
        .state()
        .scraper_addr
        .send(scraper::GetCachedGraph::default())
        .flatten()
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

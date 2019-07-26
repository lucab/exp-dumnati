use crate::graph;
use actix::prelude::*;
use failure::{Error, Fallible};
use futures::future;
use futures::prelude::*;
use prometheus::{IntCounter, IntGauge};
use reqwest::Method;
use serde_derive::Deserialize;

/// Templated URL for release index.
static RELEASES_JSON: &str =
    "https://builds.coreos.fedoraproject.org/prod/streams/${stream}/releases.json";

lazy_static::lazy_static! {
    static ref GRAPH_FINAL_RELEASES: IntGauge = register_int_gauge!(opts!(
        "dumnati_scraper_graph_final_releases",
        "Number of releases in the final graph, after processing"
    )).unwrap();
    static ref LAST_REFRESH: IntGauge = register_int_gauge!(opts!(
        "dumnati_scraper_graph_last_refresh_timestamp",
        "UTC timestamp of last graph refresh"
    )).unwrap();
    static ref UPSTREAM_SCRAPES: IntCounter = register_int_counter!(opts!(
        "dumnati_scraper_upstream_scrapes_total",
        "Total number of upstream scrapes"
    ))
    .unwrap();
}

/// Fedora CoreOS release index
#[derive(Deserialize)]
pub struct ReleaseIndex {
    pub releases: Vec<Release>,
}

#[derive(Deserialize)]
pub struct Release {
    pub commits: Vec<ReleaseCommit>,
    pub version: String,
    pub metadata: String,
}

#[derive(Deserialize)]
pub struct ReleaseCommit {
    pub architecture: String,
    pub checksum: String,
}

/// Release scraper.
#[derive(Debug)]
pub struct Scraper {
    graph: graph::Graph,
    hclient: reqwest::r#async::Client,
    url: reqwest::Url,
}

impl Scraper {
    pub fn new<S>(stream: S) -> Fallible<Self>
    where
        S: Into<String>,
    {
        let vars = hashmap! { "stream".to_string() => stream.into() };
        let releases_json = envsubst::substitute(RELEASES_JSON, &vars)?;
        let scraper = Self {
            graph: graph::Graph::default(),
            hclient: reqwest::r#async::ClientBuilder::new().build()?,
            url: reqwest::Url::parse(&releases_json)?,
        };
        Ok(scraper)
    }

    /// Return a request builder with base URL and parameters set.
    fn new_request(&self, method: reqwest::Method) -> Fallible<reqwest::r#async::RequestBuilder> {
        let builder = self.hclient.request(method, self.url.clone());
        Ok(builder)
    }

    /// Fetch releases from release-index.
    fn fetch_releases(&self) -> impl Future<Item = Vec<Release>, Error = Error> {
        let req = self.new_request(Method::GET);
        future::result(req)
            .and_then(|req| req.send().from_err())
            .and_then(|resp| resp.error_for_status().map_err(Error::from))
            .and_then(|mut resp| resp.json::<ReleaseIndex>().from_err())
            .map(|json| json.releases)
    }
}

impl Actor for Scraper {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Kick-start the state machine.
        Self::tick_now(ctx);
    }
}

pub(crate) struct RefreshTick {}

impl Message for RefreshTick {
    type Result = Result<(), Error>;
}

impl Handler<RefreshTick> for Scraper {
    type Result = ResponseActFuture<Self, (), Error>;

    fn handle(&mut self, _msg: RefreshTick, ctx: &mut Self::Context) -> Self::Result {
        UPSTREAM_SCRAPES.inc();

        let update_graph = self
            .fetch_releases()
            .and_then(|releases| graph::Graph::from_releases(releases));

        let update_graph = actix::fut::wrap_future::<_, Self>(update_graph)
            .map_err(|err, _actor, _ctx| log::error!("{}", err))
            .map(|graph, actor, _ctx| {
                actor.graph = graph;
                let refresh_timestamp = chrono::Utc::now();
                LAST_REFRESH.set(refresh_timestamp.timestamp());
                GRAPH_FINAL_RELEASES.set(actor.graph.nodes.len() as i64)
            })
            .then(|_r, _actor, ctx| {
                Self::tick_later(ctx, std::time::Duration::from_secs(30));
                actix::fut::ok(())
            });

        ctx.wait(update_graph);

        Box::new(actix::fut::ok(()))
    }
}

pub(crate) struct GetCachedGraph {
    pub(crate) basearch: String,
    pub(crate) stream: String,
}

impl Default for GetCachedGraph {
    fn default() -> Self {
        Self {
            basearch: "x86_64".to_string(),
            stream: "testing".to_string(),
        }
    }
}

impl Message for GetCachedGraph {
    type Result = Result<graph::Graph, Error>;
}

impl Handler<GetCachedGraph> for Scraper {
    type Result = ResponseActFuture<Self, graph::Graph, Error>;
    fn handle(&mut self, msg: GetCachedGraph, _ctx: &mut Self::Context) -> Self::Result {
        assert_eq!(msg.basearch, "x86_64");
        assert_eq!(msg.stream, "testing");

        Box::new(actix::fut::ok(self.graph.clone()))
    }
}

impl Scraper {
    /// Schedule an immediate refresh the state machine.
    pub fn tick_now(ctx: &mut Context<Self>) {
        ctx.notify(RefreshTick {})
    }

    /// Schedule a delayed refresh of the state machine.
    pub fn tick_later(ctx: &mut Context<Self>, after: std::time::Duration) -> actix::SpawnHandle {
        ctx.notify_later(RefreshTick {}, after)
    }
}

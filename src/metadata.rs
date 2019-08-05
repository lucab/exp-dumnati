//! Fedora CoreOS metadata.

use serde_derive::Deserialize;

/// Templated URL for release index.
pub static RELEASES_JSON: &str =
    "https://builds.coreos.fedoraproject.org/prod/streams/${stream}/releases.json";

/// Templated URL for stream metadata.
pub static STREAM_JSON: &str = "https://builds.coreos.fedoraproject.org/updates/${stream}.json";

pub static SCHEME: &str = "org.fedoraproject.coreos.scheme";

pub static AGE_INDEX: &str = "org.fedoraproject.coreos.releases.age_index";

pub static DEADEND: &str = "org.fedoraproject.coreos.updates.deadend";
pub static DEADEND_REASON: &str = "org.fedoraproject.coreos.updates.deadend_reason";
pub static DURATION: &str = "org.fedoraproject.coreos.updates.duration_minutes";
pub static START_EPOCH: &str = "org.fedoraproject.coreos.updates.start_epoch";
pub static START_VALUE: &str = "org.fedoraproject.coreos.updates.start_value";

/// Fedora CoreOS release index.
#[derive(Debug, Deserialize)]
pub struct ReleasesJSON {
    pub releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
pub struct Release {
    pub commits: Vec<ReleaseCommit>,
    pub version: String,
    pub metadata: String,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseCommit {
    pub architecture: String,
    pub checksum: String,
}

/// Fedora CoreOS updates metadata
#[derive(Debug, Deserialize)]
pub struct UpdatesJSON {
    pub updates: Updates,
}

#[derive(Debug, Deserialize)]
pub struct Updates {
    pub barriers: Vec<UpdateBarrier>,
    pub deadends: Vec<UpdateDeadend>,
    pub rollouts: Vec<UpdateRollout>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBarrier {
    pub version: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeadend {
    pub version: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRollout {
    pub version: String,
    pub start_epoch: String,
    pub start_value: String,
    pub duration_minutes: Option<String>,
}

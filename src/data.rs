use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectRepo {
    pub name: String,
    pub origin: String,
    pub source_viewer: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProjectIndex {
    pub initial_ver: String,
    pub latest_ver: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProjectStats {
    pub searches: u64,
    pub autocompletes: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Project {
    pub repo: ProjectRepo,
    pub index: ProjectIndex,
    pub stats: ProjectStats,
}
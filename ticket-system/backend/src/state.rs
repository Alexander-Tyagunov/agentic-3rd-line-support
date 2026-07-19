//! Shared application state passed to every handler.

use crate::config::Config;
use crate::firestore::Fs;

#[derive(Clone)]
pub struct AppState {
    pub fs: Fs,
    pub cfg: Config,
    pub http: reqwest::Client,
}

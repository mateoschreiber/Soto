//! Shared functional API response contracts. No credentials or filesystem locators.
use serde::{Deserialize, Serialize};
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Setup {
    pub configured: bool,
    pub server_name: String,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub username: String,
    pub admin: bool,
    pub server_name: String,
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Debug)]
pub struct Media {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub year: Option<u16>,
    pub description: Option<String>,
    pub rating: Option<String>,
    pub duration_seconds: Option<u64>,
    pub poster: Option<String>,
    pub origin: String,
    pub source_count: u32,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Source {
    pub id: String,
    pub media_id: String,
    pub origin: String,
    pub kind: String,
    pub label: String,
    pub status: String,
    pub quality: Option<String>,
    pub size: Option<u64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    #[serde(default)]
    pub audio_streams: Vec<Track>,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Detail {
    pub media: Media,
    pub sources: Vec<Source>,
    #[serde(default)]
    pub episodes: Vec<Media>,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub url: String,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Playback {
    pub id: String,
    pub url: String,
    pub mode: String,
    pub position: f64,
    #[serde(default)]
    pub timeline_offset: f64,
    #[serde(default)]
    pub duration: Option<f64>,
    pub subtitles: Vec<Track>,
    pub audio_streams: Vec<Track>,
}

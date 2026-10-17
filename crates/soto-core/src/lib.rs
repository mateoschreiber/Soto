//! Shared domain contracts for the Soto clients and server.
pub mod api;

mod navigation;

pub use navigation::{Direction, FocusBox, Page, Route, nearest_in_direction};
use serde::{Deserialize, Serialize};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    #[default]
    Local,
    Torrent,
}
impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Torrent => "Torrents",
        }
    }
    pub fn path(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Torrent => "torrent",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub year: u16,
    pub genres: Vec<String>,
    pub description: String,
    pub rating: String,
    pub duration: String,
    pub cast: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceStatus {
    Available,
    Downloaded,
    Pending,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaSource {
    pub id: String,
    pub media_id: String,
    pub origin: Origin,
    pub status: SourceStatus,
    pub label: String,
    pub quality: String,
    pub size: String,
}
pub fn sources_for<'a>(
    sources: &'a [MediaSource],
    id: &str,
    origin: Origin,
) -> Vec<&'a MediaSource> {
    sources
        .iter()
        .filter(|s| s.media_id == id && s.origin == origin)
        .collect()
}
pub fn filter_media<'a>(
    items: &'a [MediaItem],
    sources: &[MediaSource],
    origin: Origin,
    query: &str,
    genre: &str,
    kind: &str,
) -> Vec<&'a MediaItem> {
    let normalized = |text: &str| {
        text.nfd()
            .filter(|c| !is_combining_mark(*c))
            .collect::<String>()
            .to_lowercase()
    };
    let needle = normalized(query.trim());
    items
        .iter()
        .filter(|item| {
            sources
                .iter()
                .any(|s| s.origin == origin && s.media_id == item.id)
                && normalized(&item.title).contains(&needle)
                && (genre == "Todos" || item.genres.iter().any(|g| g == genre))
                && (kind == "all" || item.kind == kind)
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedItem {
    pub origin: Origin,
    pub media_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    pub version: u8,
    pub tv: bool,
    pub remote_images: bool,
    pub profile: String,
    pub saved: Vec<SavedItem>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            tv: false,
            remote_images: true,
            profile: String::new(),
            saved: Vec::new(),
        }
    }
}
impl Preferences {
    pub fn from_json(json: &str) -> Self {
        serde_json::from_str::<Self>(json)
            .ok()
            .filter(|p| p.version == 1 && p.saved.len() <= 1000 && p.profile.chars().count() <= 50)
            .unwrap_or_default()
    }
    pub fn is_saved(&self, origin: Origin, id: &str) -> bool {
        self.saved
            .iter()
            .any(|s| s.origin == origin && s.media_id == id)
    }
    pub fn toggle_saved(&mut self, origin: Origin, id: &str) {
        if self.is_saved(origin, id) {
            self.saved
                .retain(|s| !(s.origin == origin && s.media_id == id));
        } else if self.saved.len() < 1000 {
            self.saved.push(SavedItem {
                origin,
                media_id: id.into(),
            });
        }
    }
}

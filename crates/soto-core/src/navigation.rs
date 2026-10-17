use crate::Origin;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Page {
    #[default]
    Home,
    Library,
    Search,
    Detail,
    Sources,
    Collections,
    Queue,
    Server,
    Providers,
    Devices,
    NotFound,
}
impl Page {
    pub fn path(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Library => "library",
            Self::Search => "search",
            Self::Detail => "detail",
            Self::Sources => "sources",
            Self::Collections => "collections",
            Self::Queue => "queue",
            Self::Server => "server",
            Self::Providers => "providers",
            Self::Devices => "devices",
            Self::NotFound => "not-found",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Route {
    pub origin: Origin,
    pub page: Page,
    pub id: Option<String>,
    pub query: String,
}
impl Route {
    pub fn new(origin: Origin, page: Page) -> Self {
        Self {
            origin,
            page,
            ..Self::default()
        }
    }
    pub fn media(origin: Origin, page: Page, id: &str) -> Self {
        Self {
            origin,
            page,
            id: Some(id.into()),
            query: String::new(),
        }
    }
    pub fn href(&self) -> String {
        let mut path = format!("#/{}/{}", self.origin.path(), self.page.path());
        if let Some(id) = &self.id {
            path.push('/');
            path.push_str(id);
        }
        if !self.query.is_empty() {
            path.push('?');
            path.push_str(
                &form_urlencoded::Serializer::new(String::new())
                    .append_pair("q", &self.query)
                    .finish(),
            );
        }
        path
    }
    pub fn parse(hash: &str) -> Self {
        if matches!(hash, "" | "#" | "#/") {
            return Self::default();
        }
        let invalid = || Self::new(Origin::Local, Page::NotFound);
        let raw = hash.strip_prefix('#').unwrap_or(hash);
        let (path, query) = raw.split_once('?').unwrap_or((raw, ""));
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        if !(2..=3).contains(&segments.len()) {
            return invalid();
        }
        let origin = match segments[0] {
            "local" => Origin::Local,
            "torrent" => Origin::Torrent,
            _ => return invalid(),
        };
        let page = match segments[1] {
            "home" => Page::Home,
            "library" => Page::Library,
            "search" => Page::Search,
            "detail" => Page::Detail,
            "sources" => Page::Sources,
            "collections" => Page::Collections,
            "queue" => Page::Queue,
            "server" => Page::Server,
            "providers" => Page::Providers,
            "devices" => Page::Devices,
            _ => return invalid(),
        };
        let id = if let Some(raw) = segments.get(2) {
            let Ok(id) = percent_encoding::percent_decode_str(raw).decode_utf8() else {
                return invalid();
            };
            if id.is_empty()
                || id.len() > 80
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return invalid();
            }
            Some(id.into_owned())
        } else {
            None
        };
        if matches!(page, Page::Detail | Page::Sources) != id.is_some() {
            return invalid();
        }
        let query = form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == "q")
            .map(|(_, value)| value.chars().take(200).collect())
            .unwrap_or_default();
        Self {
            origin,
            page,
            id,
            query,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct FocusBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}
pub fn nearest_in_direction(
    current: usize,
    boxes: &[FocusBox],
    direction: Direction,
) -> Option<usize> {
    let c = boxes.get(current)?;
    let cx = c.x + c.width / 2.;
    let cy = c.y + c.height / 2.;
    let horizontal = matches!(direction, Direction::Left | Direction::Right);
    let sign = if matches!(direction, Direction::Right | Direction::Down) {
        1.
    } else {
        -1.
    };
    boxes
        .iter()
        .enumerate()
        .filter_map(|(index, b)| {
            if index == current {
                return None;
            }
            let dx = b.x + b.width / 2. - cx;
            let dy = b.y + b.height / 2. - cy;
            let primary = if horizontal { dx } else { dy } * sign;
            let secondary = if horizontal { dy } else { dx }.abs();
            (primary >= 1.).then_some((
                index,
                primary + 3. * secondary + secondary * secondary / primary,
            ))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

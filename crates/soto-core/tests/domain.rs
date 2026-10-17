#[path = "fixtures/demo.rs"]
mod demo;
use demo::{demo_media, demo_sources};
use soto_core::*;

#[test]
fn downloaded_torrent_stays_out_of_local() {
    let items = demo_media();
    let sources = demo_sources();
    let local = filter_media(&items, &sources, Origin::Local, "", "Todos", "all");
    let torrents = filter_media(&items, &sources, Origin::Torrent, "", "Todos", "all");
    assert!(!local.iter().any(|m| m.id == "tt2543164"));
    assert!(torrents.iter().any(|m| m.id == "tt2543164"));
    assert!(
        sources
            .iter()
            .any(|s| s.media_id == "tt2543164" && s.status == SourceStatus::Downloaded)
    );
}

#[test]
fn search_and_filters_preserve_origin() {
    let items = demo_media();
    let sources = demo_sources();
    let results = filter_media(
        &items,
        &sources,
        Origin::Local,
        "DÚNE",
        "Ciencia ficción",
        "movie",
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "tt15239678");
    assert!(filter_media(&items, &sources, Origin::Torrent, "Dune", "Todos", "series").is_empty());
}

#[test]
fn routes_round_trip_and_reject_invalid_input() {
    let route = Route {
        origin: Origin::Torrent,
        page: Page::Search,
        id: None,
        query: "ciencia & ficción".into(),
    };
    assert_eq!(Route::parse(&route.href()), route);
    for invalid in [
        "#/otro/home",
        "#/local/missing",
        "#/local/detail/%E0%A4%A",
        "#/local/detail/../../secret",
        "#/local/sources",
    ] {
        assert_eq!(Route::parse(invalid).page, Page::NotFound, "{invalid}");
    }
    assert_eq!(Route::parse("").origin, Origin::Local);
}

#[test]
fn missing_source_is_not_a_playable_local_file() {
    assert!(sources_for(&demo_sources(), "tt2543164", Origin::Local).is_empty());
    let unknown = Route {
        origin: Origin::Local,
        page: Page::Detail,
        id: Some("unknown".into()),
        query: String::new(),
    };
    assert!(
        !demo_media()
            .iter()
            .any(|m| Some(m.id.to_string()) == unknown.id)
    );
}

#[test]
fn spatial_focus_prefers_aligned_option_and_does_not_wrap() {
    let boxes = [
        FocusBox {
            x: 0.,
            y: 0.,
            width: 50.,
            height: 50.,
        },
        FocusBox {
            x: 100.,
            y: 0.,
            width: 50.,
            height: 50.,
        },
        FocusBox {
            x: 0.,
            y: 100.,
            width: 50.,
            height: 50.,
        },
        FocusBox {
            x: 70.,
            y: 100.,
            width: 50.,
            height: 50.,
        },
    ];
    assert_eq!(nearest_in_direction(0, &boxes, Direction::Right), Some(1));
    assert_eq!(nearest_in_direction(0, &boxes, Direction::Down), Some(2));
    assert_eq!(nearest_in_direction(0, &boxes, Direction::Left), None);
}

#[test]
fn preferences_recover_from_corrupt_or_old_data() {
    assert!(!Preferences::from_json("{broken").tv);
    assert!(!Preferences::from_json(r#"{"version":99,"tv":true,"saved":[]}"#).tv);
    let mut preferences = Preferences {
        tv: true,
        ..Preferences::default()
    };
    preferences.toggle_saved(Origin::Torrent, "tt2543164");
    assert_eq!(
        Preferences::from_json(&serde_json::to_string(&preferences).unwrap()),
        preferences
    );
    assert!(!preferences.is_saved(Origin::Local, "tt2543164"));
}

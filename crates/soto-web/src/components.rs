use dioxus::prelude::*;
#[component]
pub fn Icon(name: String, #[props(default = 24)] size: u32) -> Element {
    let path = match name.as_str() {
        "home" => "M3 10 12 3l9 7v11h-7v-7h-4v7H3Z",
        "local" => "M3 5h6l2 3h10v13H3Z",
        "torrent" => "M12 3v13m-6-6 6 6 6-6M3 17v5h18v-5",
        "collection" => "M3 3h7v7H3ZM14 3h7v7h-7ZM3 14h7v7H3ZM14 14h7v7h-7Z",
        "clock" => "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm0 4v6l4 2",
        "server" => "M3 3h18v7H3ZM3 14h18v7H3ZM6 6h2m-2 11h2",
        "plug" => "M8 2v6m8-6v6M5 8h14v5a7 7 0 0 1-14 0Zm7 12v3",
        "tv" => "M2 4h20v14H2Zm10 14v4m-5 0h10",
        "search" => "M10.5 2a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17ZM17 17l6 6",
        "play" => "M5 3 21 12 5 21Z",
        "back" => "M15 4 7 12l8 8M7 12h15",
        "close" => "m5 5 14 14M19 5 5 19",
        "plus" => "M12 4v16M4 12h16",
        "check" => "m4 12 5 5L21 5",
        "arrow" => "M3 12h18m-7-7 7 7-7 7",
        "heart" => "M12 21 3 12C-2 5 7-1 12 6 17-1 26 5 21 12Z",
        "star" => "m12 2 3 6 7 1-5 5 1 8-6-3-6 3 1-8-5-5 7-1Z",
        "rocket" => "M9 15 5 19m4-12 8-5 5 1-1 5-8 8-4-3Zm7-1h.1M9 7 3 8l-1 5 7-1m4 3-1 7 5-1 1-6",
        "drama" => "M3 3c5 3 13 3 18 0v9c0 6-5 10-9 11-4-1-9-5-9-11Zm3 7h3m6 0h3M8 16q4-4 8 0",
        "laugh" => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20ZM7 8h2m6 0h2M6 13h12q-6 10-12 0",
        "action" => "M3 3 21 21m-1-18L3 20M3 15l6 6m6-18 6 6M3 3h6v6m6 6h6v6",
        "ghost" => "M4 22V10a8 8 0 0 1 16 0v12l-4-3-4 3-4-3Zm4-12h1m6 0h1",
        "compass" => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm5 5-3 7-7 3 3-7Z",
        "film" => "M3 3h18v18H3ZM3 8h18M3 16h18M8 3v18m8-18v18",
        "info" => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 8v7m0-11h.1",
        _ => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20ZM12 7v6m0 4h.1",
    };
    rsx! {
        svg {
            width: size,
            height: size,
            view_box: "0 0 24 24",
            fill: if name == "play" { "currentColor" } else { "none" },
            stroke: "currentColor",
            stroke_width: "1.7",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: path }
        }
    }
}

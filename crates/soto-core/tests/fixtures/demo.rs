use soto_core::{MediaItem, MediaSource, Origin, SourceStatus};
/// Fixtures only. None of these objects asserts real server or provider availability.
pub fn demo_media() -> Vec<MediaItem> {
    let rows = [
        (
            "tt15239678",
            "Dune: Parte dos",
            "movie",
            2024,
            "Ciencia ficción|Aventura",
            "El destino de un universo se escribe en la arena.",
            "8.5",
            "2 h 46 min",
        ),
        (
            "tt11280740",
            "Severance",
            "series",
            2025,
            "Suspenso|Drama|Ciencia ficción",
            "Tu vida laboral. Tu vida personal. Dos mundos que jamás deberían encontrarse. Bienvenido a Lumon.",
            "8.7",
            "Temporada 2 · 47 min por episodio",
        ),
        (
            "tt0816692",
            "Interstellar",
            "movie",
            2014,
            "Ciencia ficción|Drama|Aventura",
            "Un viaje más allá de las estrellas para encontrar nuestro lugar en el universo.",
            "8.7",
            "2 h 49 min",
        ),
        (
            "tt14452776",
            "The Bear",
            "series",
            2025,
            "Drama|Comedia",
            "Una cocina, una familia y la búsqueda de algo extraordinario.",
            "8.6",
            "3 temporadas",
        ),
        (
            "tt3581920",
            "The Last of Us",
            "series",
            2025,
            "Drama|Aventura",
            "Un vínculo inesperado en un mundo que ha cambiado para siempre.",
            "8.7",
            "2 temporadas",
        ),
        (
            "tt15398776",
            "Oppenheimer",
            "movie",
            2023,
            "Drama",
            "La historia de un descubrimiento que transformó el mundo.",
            "8.3",
            "3 h",
        ),
        (
            "tt11126994",
            "Arcane",
            "series",
            2024,
            "Animación|Acción|Drama",
            "Dos hermanas y una ciudad dividida entre el progreso y el conflicto.",
            "9.0",
            "2 temporadas",
        ),
        (
            "tt0804484",
            "Foundation",
            "series",
            2025,
            "Ciencia ficción|Drama",
            "El futuro de una civilización depende de una idea.",
            "7.6",
            "3 temporadas",
        ),
        (
            "tt2543164",
            "La llegada",
            "movie",
            2016,
            "Ciencia ficción|Drama|Suspenso",
            "Cuando doce naves llegan a la Tierra, una lingüista busca comprender su mensaje.",
            "7.9",
            "1 h 56 min",
        ),
        (
            "tt0780504",
            "Drive",
            "movie",
            2011,
            "Drama|Acción",
            "Un conductor silencioso se encuentra en el centro de una peligrosa decisión.",
            "7.8",
            "1 h 40 min",
        ),
        (
            "tt4975722",
            "Moonlight",
            "movie",
            2016,
            "Drama",
            "Tres capítulos de una vida, entre la identidad, la memoria y el afecto.",
            "7.4",
            "1 h 51 min",
        ),
        (
            "tt2582802",
            "Whiplash",
            "movie",
            2014,
            "Drama",
            "Talento, disciplina y el precio de perseguir la perfección.",
            "8.5",
            "1 h 46 min",
        ),
        (
            "tt1856101",
            "Blade Runner 2049",
            "movie",
            2017,
            "Ciencia ficción|Suspenso",
            "Un nuevo hallazgo pone en duda todo lo que un agente creía conocer.",
            "8.0",
            "2 h 44 min",
        ),
        (
            "tt6751668",
            "Parásitos",
            "movie",
            2019,
            "Drama|Suspenso",
            "Dos familias, dos mundos y una inesperada conexión.",
            "8.5",
            "2 h 12 min",
        ),
        (
            "tt1877830",
            "The Batman",
            "movie",
            2022,
            "Acción|Suspenso",
            "Las sombras de Gotham guardan una verdad por descubrir.",
            "7.8",
            "2 h 56 min",
        ),
        (
            "tt2788316",
            "Shōgun",
            "series",
            2024,
            "Drama|Aventura",
            "Un encuentro entre culturas en un Japón al borde de una nueva era.",
            "8.6",
            "1 temporada",
        ),
        (
            "tt5753856",
            "Dark",
            "series",
            2020,
            "Suspenso|Ciencia ficción",
            "El tiempo conecta a cuatro familias y un secreto que atraviesa generaciones.",
            "8.7",
            "3 temporadas",
        ),
        (
            "tt9253284",
            "Andor",
            "series",
            2025,
            "Ciencia ficción|Acción",
            "El comienzo de una rebelión se construye con pequeñas decisiones.",
            "8.4",
            "2 temporadas",
        ),
        (
            "tt14688458",
            "Silo",
            "series",
            2025,
            "Ciencia ficción|Suspenso",
            "Una comunidad vive bajo tierra, rodeada de preguntas sin respuesta.",
            "8.1",
            "2 temporadas",
        ),
        (
            "tt3659388",
            "Marte",
            "movie",
            2015,
            "Ciencia ficción|Aventura",
            "La supervivencia de un astronauta depende de la ciencia y la imaginación.",
            "8.0",
            "2 h 24 min",
        ),
        (
            "tt0470752",
            "Ex Machina",
            "movie",
            2014,
            "Ciencia ficción|Suspenso",
            "Una prueba de inteligencia artificial se convierte en una cuestión de confianza.",
            "7.7",
            "1 h 48 min",
        ),
    ];
    rows.into_iter()
        .map(
            |(id, title, kind, year, genres, description, rating, duration)| MediaItem {
                id: id.into(),
                title: title.into(),
                kind: kind.into(),
                year,
                genres: genres.split('|').map(String::from).collect(),
                description: description.into(),
                rating: rating.into(),
                duration: duration.into(),
                cast: if id == "tt11280740" {
                    vec![
                        "Adam Scott".into(),
                        "Britt Lower".into(),
                        "John Turturro".into(),
                        "Patricia Arquette".into(),
                    ]
                } else {
                    Vec::new()
                },
            },
        )
        .collect()
}
pub fn demo_sources() -> Vec<MediaSource> {
    let items = demo_media();
    let mut sources = Vec::with_capacity(40);
    for (i, m) in items.iter().enumerate() {
        if i != 8 {
            sources.push(MediaSource {
                id: format!("local-{}", m.id),
                media_id: m.id.clone(),
                origin: Origin::Local,
                status: SourceStatus::Available,
                label: "Archivo local de ejemplo".into(),
                quality: "4K HDR".into(),
                size: "18.4 GB · Ejemplo".into(),
            });
        }
        if [0, 2, 4, 5, 8, 12, 13, 14, 16, 20].contains(&i) {
            sources.push(MediaSource {
                id: format!("torrent-{}", m.id),
                media_id: m.id.clone(),
                origin: Origin::Torrent,
                status: if i == 8 {
                    SourceStatus::Downloaded
                } else {
                    SourceStatus::Pending
                },
                label: "Real-Debrid · Fuente de ejemplo".into(),
                quality: "4K HDR".into(),
                size: "12.8 GB · Ejemplo".into(),
            });
        }
    }
    sources.push(MediaSource {
        id: "torrent-arrival-hd".into(),
        media_id: "tt2543164".into(),
        origin: Origin::Torrent,
        status: SourceStatus::Pending,
        label: "Real-Debrid · Full HD · Ejemplo".into(),
        quality: "1080p".into(),
        size: "5.8 GB · Ejemplo".into(),
    });
    sources.push(MediaSource {
        id: "torrent-arrival-p2p".into(),
        media_id: "tt2543164".into(),
        origin: Origin::Torrent,
        status: SourceStatus::Unavailable,
        label: "Torrent directo · Ejemplo".into(),
        quality: "P2P".into(),
        size: "Motor pendiente de implementar".into(),
    });
    sources
}

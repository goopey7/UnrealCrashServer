use crate::{
    crash_endpoint::advance_to_next_item, crash_overview::CrashOverview, utils::decompress_data,
};
use askama::Template;
use axum::{
    Router,
    extract::{Path as AxumPath, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tower_http::trace::TraceLayer;
use tracing::info;

#[derive(Template)]
#[template(path = "crash_list.html")]
struct CrashListTemplate {
    crashes: Vec<Crash>,
}

struct Crash {
    timestamp: String,
    overview: CrashOverview,
}

async fn handle_list(State(path): State<PathBuf>) -> impl IntoResponse {
    let mut crashes = Vec::new();

    for entry in fs::read_dir(&path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let timestamp = path
            .file_name()
            .unwrap()
            .to_os_string()
            .to_str()
            .unwrap()
            .to_string();
        let json_file = fs::read(path.join("CrashOverview.json")).unwrap();
        let json_str = str::from_utf8(&json_file).unwrap();
        let overview = serde_json::from_str::<CrashOverview>(json_str).unwrap();
        crashes.push(Crash {
            timestamp,
            overview,
        });
    }

    crashes.sort_by(|c1: &Crash, c2: &Crash| c2.timestamp.cmp(&c1.timestamp));
    let crash_template = CrashListTemplate { crashes };
    (StatusCode::OK, Html(crash_template.render().unwrap())).into_response()
}

async fn handle_download(
    State(path): State<PathBuf>,
    AxumPath((timestamp, filename)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let data = get_file(&path, &timestamp, &filename);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        data,
    )
        .into_response()
}

fn get_file(path: &Path, timestamp: &str, filename: &str) -> Vec<u8> {
    let mut file = fs::File::open(path.join(timestamp).join("CrashData.zlib")).unwrap();
    let mut body = Vec::new();
    file.read_to_end(&mut body).unwrap();
    let content = decompress_data(&body).unwrap();
    let mut bytes_read = 0;
    let filename_bytes = filename.as_bytes();
    while *filename_bytes != content[bytes_read..bytes_read + filename.len()] {
        bytes_read += 1;
    }
    bytes_read += filename.len();
    bytes_read += advance_to_next_item(&content[bytes_read..]);
    let size = u32::from_le_bytes(
        content[bytes_read..bytes_read + size_of::<u32>()]
            .try_into()
            .unwrap(),
    );
    bytes_read += size_of::<u32>();
    let mut out_v = Vec::new();
    content[bytes_read..bytes_read + size as usize].clone_into(&mut out_v);
    info!("Downloading {filename} size {size} bytes");
    out_v
}

pub async fn run_webui(path: PathBuf) {
    let app = Router::new()
        .route("/", get(handle_list))
        .route("/download/{timestamp}/{filename}", get(handle_download))
        .with_state(path)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await.unwrap();
    info!("Listening on http://0.0.0.0:8081");
    axum::serve(listener, app).await.unwrap();
}

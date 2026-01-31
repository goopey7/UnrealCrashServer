use crate::{crash_overview::CrashOverview, utils::decompress_data};
use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use webhook::client::WebhookClient;

#[derive(Serialize, Deserialize)]
struct CrashReportParams {
    #[serde(rename = "AppID")]
    app_id: Option<String>,
    #[serde(rename = "AppVersion")]
    app_version: Option<String>,
    #[serde(rename = "AppEnvironment")]
    app_environment: Option<String>,
    #[serde(rename = "UploadType")]
    upload_type: Option<String>,
    #[serde(rename = "UserID")]
    user_id: Option<String>,
}

pub struct File<'a> {
    pub name: &'a str,
    contents: &'a [u8],
}

const STRING_DELIM: &[u8] = &[0x04, 0x01, 0x00, 0x00];

async fn handle_crash_report(
    State(path): State<PathBuf>,
    Query(_params): Query<CrashReportParams>,
    _headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let content = match decompress_data(&body) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to decompress request body with zlib: {e}");
            return StatusCode::BAD_REQUEST;
        }
    };

    let mut bytes_read = 0;
    if &content[0..3] != b"CR1" {
        error!("Malformed crash report file header! {:?}", &content[0..3]);
        return StatusCode::BAD_REQUEST;
    }
    bytes_read += 3;

    let crash_id = match read_string(&content[bytes_read..], &mut bytes_read) {
        Ok(crash_id_str) => crash_id_str,
        Err(e) => {
            error!("{}", e);
            return StatusCode::BAD_REQUEST;
        }
    };
    info!("Received Crash Report: {}", crash_id);

    bytes_read += advance_to_next_item(&content[bytes_read..]);
    let _crash_filename = match read_string(&content[bytes_read..], &mut bytes_read) {
        Ok(crash_filename_str) => crash_filename_str,
        Err(e) => {
            error!("{}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    bytes_read += advance_to_next_item(&content[bytes_read..]);
    let file_size = u32::from_le_bytes(content[bytes_read..bytes_read + 4].try_into().unwrap());
    bytes_read += 4;
    if file_size as usize != content.len() {
        error!("File size specified in file is different from size of data extracted!");
        return StatusCode::BAD_REQUEST;
    }

    let number_of_files = content[bytes_read];
    bytes_read += 1;

    bytes_read += advance_to_next_item(&content[bytes_read..]);
    let files = match extract_files(&content[bytes_read..], &mut bytes_read, number_of_files) {
        Ok(files) => files,
        Err(e) => {
            error!("{}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    let crash_context_file = match files
        .iter()
        .find(|file| file.name == "CrashContext.runtime-xml")
    {
        Some(crash_context_file) => crash_context_file,
        None => {
            error!("CrashContext.runtime-xml not found!");
            return StatusCode::BAD_REQUEST;
        }
    };

    let crash_context_xml = str::from_utf8(crash_context_file.contents).unwrap();
    let crash_overview = CrashOverview::parse(crash_context_xml, &files);
    info!("{}", crash_overview.error);

    let timestamp = Utc::now().format("%Y-%m-%d_%H%M%S").to_string();
    let path = Path::new(&path).join(&timestamp).join("CrashData.zlib");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(&body as &[u8]).unwrap();

    let mut json_file =
        fs::File::create(&path.parent().unwrap().join("CrashOverview.json")).unwrap();
    let json_content = serde_json::to_string_pretty(&crash_overview).unwrap();
    json_file.write_all(&json_content.as_bytes()).unwrap();

    let webhook_url = match std::env::var("CRASH_REPORT_DISCORD") {
        Ok(url) => url,
        Err(_) => return StatusCode::OK,
    };
    let base_url = match std::env::var("BASE_URL") {
        Ok(url) => url,
        Err(_) => return StatusCode::OK,
    };
    let client = WebhookClient::new(&webhook_url);
    client
        .send(|message| {
            message.username("Crash Report").embed(|embed| {
                embed
                    .title("Crash!")
                    .description(&format!("{}#{}", base_url, timestamp))
                    .field(
                        "User Description",
                        &format!("{}", crash_overview.user_description),
                        false,
                    )
                    .field("Error", &format!("{}", crash_overview.error), false)
            })
        })
        .await
        .unwrap();

    StatusCode::OK
}

fn read_string<'a>(content: &'a [u8], total_bytes_read: &mut usize) -> Result<&'a str, String> {
    let mut bytes_read = 0;
    if &content[bytes_read..bytes_read + 4] != STRING_DELIM {
        return Err(format!(
            "Expected string delimiter! Instead found {:?}",
            &content[bytes_read..bytes_read + 4]
        ));
    }
    bytes_read += 4;
    while content[bytes_read] != 0x0 {
        bytes_read += 1;
    }
    *total_bytes_read += bytes_read;
    match std::str::from_utf8(&content[4..bytes_read]) {
        Ok(string) => Ok(string),
        Err(e) => Err(format!("{}", e)),
    }
}

pub fn advance_to_next_item(content: &[u8]) -> usize {
    let mut bytes_read = 0;
    while bytes_read < content.len() && content[bytes_read] == 0 {
        bytes_read += 1;
    }
    bytes_read
}

fn extract_files<'a>(
    content: &'a [u8],
    total_bytes_read: &mut usize,
    number_of_files: u8,
) -> Result<Vec<File<'a>>, String> {
    let mut files = Vec::new();
    let mut bytes_read = 0;

    for idx in 0..number_of_files {
        let filename = match read_string(&content[bytes_read..], &mut bytes_read) {
            Ok(filename) => filename,
            Err(e) => return Err(format!("{e}")),
        };
        bytes_read += advance_to_next_item(&content[bytes_read..]);
        let file_size = u32::from_le_bytes(content[bytes_read..bytes_read + 4].try_into().unwrap());
        bytes_read += 4;
        let file_contents = &content[bytes_read..bytes_read + file_size as usize];
        bytes_read += file_size as usize;
        if idx != number_of_files - 1 {
            let file_idx_bytes: [u8; 4] = match content[bytes_read..bytes_read + 4].try_into() {
                Ok(file_idx_bytes) => file_idx_bytes,
                Err(e) => return Err(format!("{e}")),
            };
            let _file_idx = u32::from_le_bytes(file_idx_bytes);
            bytes_read += 4;
        }
        files.push(File {
            name: filename,
            contents: file_contents,
        });
    }

    *total_bytes_read += bytes_read;
    Ok(files)
}

pub async fn run_crash_endpoint(path: PathBuf) {
    let app = Router::new()
        .route("/", post(handle_crash_report))
        .with_state(path)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}

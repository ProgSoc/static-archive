use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek},
    sync::{Arc, Mutex},
    thread,
};

use http::Uri;
use hyper::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use warp::{
    http::response::Builder, path::FullPath, reject::Reject, reply::Response, Error, Filter,
    Rejection,
};

#[derive(Debug)]
struct FailedToParseUrl;
impl Reject for FailedToParseUrl {}

#[derive(Debug)]
struct FailedToReadFile;
impl Reject for FailedToReadFile {}

#[derive(Debug, Serialize, Deserialize)]
struct ArchiveSiteEntry {
    status: u16,
    location: Option<String>,
    #[serde(rename = "contentType")]
    content_type: Option<String>,
    filename: String,
}

#[tokio::main]
async fn main() {
    // Read content path from env
    let content_path = std::env::var("CONTENT_PATH").unwrap_or(String::from("./content"));
    let content_path = std::path::Path::new(&content_path);

    let files_zip_path = content_path.join("files.zip");
    let sitemap_path = content_path.join("sitemap.json");
    let overrides_path = content_path.join("overrides");

    let overrides_root_str = overrides_path.to_str().unwrap();
    let overrides = walkdir::WalkDir::new(&overrides_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let path = e.path().to_str().unwrap().to_string();
            let path = path.replace("\\", "/");
            let path = path.replace(overrides_root_str, "");
            (path, e.path().canonicalize().unwrap())
        })
        .collect::<BTreeMap<String, _>>();
    let overrides = Arc::new(overrides);

    let archive_thread = thread::spawn(|| {
        let zipfile = std::fs::File::open(files_zip_path).unwrap();
        let archive = Arc::new(Mutex::new(zip::ZipArchive::new(zipfile).unwrap()));
        println!("Opened Archive");
        archive
    });

    let sitemap_thread = thread::spawn(|| {
        let sitemap_data: BTreeMap<String, ArchiveSiteEntry> =
            serde_json::from_reader(File::open(sitemap_path).unwrap()).unwrap();
        let sitemap_data = Arc::new(sitemap_data);
        println!("Opened Sitemap");
        sitemap_data
    });

    let archive = archive_thread.join().unwrap();
    let sitemap_data = sitemap_thread.join().unwrap();

    let with_query = warp::get()
        .and(warp::filters::path::full())
        .and(warp::filters::query::raw())
        .and_then(move |path: FullPath, query| async move {
            let uri = http::uri::Builder::new()
                .path_and_query(format!("{}?{}", path.as_str(), query))
                .build()
                .map_err(|_| warp::reject::custom(FailedToParseUrl))?;

            Ok::<_, Rejection>(uri)
        });

    let without_query =
        warp::get()
            .and(warp::filters::path::full())
            .and_then(move |path: FullPath| async move {
                let uri = http::uri::Builder::new()
                    .path_and_query(path.as_str())
                    .build()
                    .map_err(|_| warp::reject::custom(FailedToParseUrl))?;

                Ok::<_, Rejection>(uri)
            });

    let with_full_path = with_query.or(without_query).unify();

    let file_server = with_full_path.map(move |url: Uri| {
        let found_override = overrides.get(&url.to_string());

        if let Some(found_override) = found_override {
            let file = File::open(&found_override);
            let mut file = match file {
                Ok(file) => file,
                Err(_) => {
                    return Builder::new()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "text/html")
                        .body(include_bytes!("../html/failed-to-read-file.html").to_vec())
                }
            };

            let mut buf = Vec::new();
            let result = file.read_to_end(&mut buf);
            match result {
                Ok(_) => {}
                Err(_) => {
                    return Builder::new()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "text/html")
                        .body(include_bytes!("../html/failed-to-read-file.html").to_vec())
                }
            };

            let mime = mime_guess::from_path(found_override);
            let mime = mime.first_raw();

            let mut builder = Builder::new().status(StatusCode::OK);

            if let Some(mime) = mime {
                builder = builder.header("Content-Type", mime);
            }

            return builder.body(buf);
        }

        let found_entry = sitemap_data.get(&url.to_string());

        let found_entry = if let Some(found) = found_entry {
            found
        } else {
            return Builder::new()
                .status(404)
                .header("Content-Type", "text/html")
                .body(include_bytes!("../html/404.html").to_vec());
        };

        let mut archive = archive.lock().unwrap();
        let file = archive.by_name(&format!("files/{}", found_entry.filename));

        let mut file = if let Ok(file) = file {
            file
        } else {
            let a = Builder::new()
                .status(404)
                .header("Content-Type", "text/html")
                .body(include_bytes!("../html/file-not-found.html").to_vec());
            return a;
        };

        // Read file into byte array
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes).unwrap();

        let mut builder = Builder::new()
            .status(found_entry.status)
            .header("Content-Length", bytes.len());

        if let Some(content_type) = &found_entry.content_type {
            builder = builder.header("Content-Type", content_type);
        }

        if let Some(location) = &found_entry.location {
            builder = builder.header("Location", location);
        }

        builder.body(bytes)
    });

    println!("Ready to serve files!");

    warp::serve(file_server).run(([127, 0, 0, 1], 3030)).await;
}

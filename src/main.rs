use std::{net::IpAddr, str::FromStr, sync::Arc};

use http::Uri;

use serde::{Deserialize, Serialize};
use warp::{http::response::Builder, path::FullPath, reject::Reject, Filter, Rejection};

use crate::static_zip::StaticZipArchive;

mod static_zip;

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
    let content_path = std::env::var("CONTENT_PATH").unwrap_or_else(|_| String::from("./content"));
    let content_path = std::path::Path::new(&content_path);

    let overrides_path = content_path.join("overrides");

    let static_zip = Arc::new(StaticZipArchive::new(content_path).await);

    let file_server = all_urls_filter().map(move |url: Uri| {
        if let Some(reply) = static_zip.get_response_from_uri(&url) {
            reply
        } else {
            Box::new(
                Builder::new()
                    .status(404)
                    .header("Content-Type", "text/html")
                    .body(include_bytes!("../html/404.html").to_vec()),
            )
        }
    });

    println!("Ready to serve files!");

    let server = warp::fs::dir(overrides_path).or(file_server);

    let addr = IpAddr::from_str("::0").unwrap();
    warp::serve(server).run((addr, 3030)).await;
}

fn all_urls_filter() -> impl Clone + Filter<Extract = (Uri,), Error = Rejection> {
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

    with_query.or(without_query).unify()
}

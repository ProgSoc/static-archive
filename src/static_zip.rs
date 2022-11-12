use std::{fs::File, io::Read, path::Path, sync::Mutex};

use sqlx::{Connection, SqliteConnection};
use warp::http::response::Builder;
use zip::ZipArchive;

#[derive(Debug)]
struct DbSiteEntry {
    status: u16,
    location: Option<String>,
    content_type: Option<String>,
    filename: String,
}

pub struct StaticZipArchive {
    archive: Mutex<ZipArchive<File>>,
    sitemap: SitemapDatabaseConnection,
}

impl StaticZipArchive {
    pub async fn new(source: &Path) -> Self {
        let files_zip_path = source.join("files.zip");
        let sql_path = source.join("sitemap.db");

        let archive_thread = tokio::task::spawn_blocking(|| {
            let zipfile = std::fs::File::open(files_zip_path).unwrap();
            let archive = Mutex::new(zip::ZipArchive::new(zipfile).unwrap());
            println!("Opened Archive");
            archive
        });

        let archive = archive_thread.await.unwrap();

        Self {
            archive,
            sitemap: SitemapDatabaseConnection::new(&sql_path).await,
        }
    }

    pub async fn get_response_from_uri(&self, uri: &http::Uri) -> Option<impl warp::Reply> {
        let found_entry = self.sitemap.get_path(&uri.to_string()).await?;

        let mut archive = self.archive.lock().unwrap();
        let file = archive.by_name(&format!("files/{}", found_entry.filename));

        let mut file = if let Ok(file) = file {
            file
        } else {
            let a = Builder::new()
                .status(404)
                .header("Content-Type", "text/html")
                .body(include_bytes!("../html/file-not-found.html").to_vec());
            return Some(Box::new(a));
        };

        // Read file into byte array
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes).unwrap();

        let mut builder = Builder::new()
            .status(found_entry.status)
            .header("Content-Length", bytes.len());

        if let Some(content_type) = found_entry.content_type {
            builder = builder.header("Content-Type", content_type);
        }

        if let Some(location) = found_entry.location {
            builder = builder.header("Location", location);
        }

        Some(Box::new(builder.body(bytes)))
    }
}

struct SitemapDatabaseConnection {
    conn: tokio::sync::Mutex<SqliteConnection>,
}

impl SitemapDatabaseConnection {
    async fn new(path: &Path) -> Self {
        let conn = SqliteConnection::connect(&format!("sqlite://{}", path.to_str().unwrap()))
            .await
            .unwrap();

        Self {
            conn: tokio::sync::Mutex::new(conn),
        }
    }

    async fn get_path(&self, path: &str) -> Option<DbSiteEntry> {
        let mut conn = self.conn.lock().await;
        let result = sqlx::query!(
            "SELECT status, location, content_type, filename FROM paths WHERE path = ?",
            path,
        )
        .fetch_one(&mut *conn)
        .await
        .ok()?;

        Some(DbSiteEntry {
            status: result.status.try_into().unwrap(),
            location: result.location,
            content_type: result.content_type,
            filename: result.filename,
        })
    }
}

use std::{collections::BTreeMap, fs::File, io::Read, path::Path, sync::Mutex};

use serde::{Deserialize, Serialize};
use warp::http::response::Builder;
use zip::ZipArchive;

use self::lookup::{LookupId, StringLookup, StringLookupBuilder};

mod lookup;

#[derive(Debug, Serialize, Deserialize)]
struct ParsedSiteEntry<'a> {
    status: u16,
    location: Option<&'a str>,
    #[serde(rename = "contentType")]
    content_type: Option<&'a str>,
    filename: &'a str,
}

struct CompressedSiteEntry {
    status: u16,
    location: Option<LookupId>,
    content_type: Option<LookupId>,
    filename: LookupId,
}

pub struct ArchiveSitemap {
    location_builder: StringLookup,
    content_type_builder: StringLookup,
    filename_builder: StringLookup,
    sitemap: BTreeMap<String, CompressedSiteEntry>,
}

impl ArchiveSitemap {
    fn get<'a>(&'a self, url: &str) -> Option<ParsedSiteEntry<'a>> {
        let found = self.sitemap.get(url)?;
        Some(ParsedSiteEntry {
            status: found.status,
            location: found.location.map(|i| self.location_builder.get(i)),
            content_type: found.content_type.map(|i| self.content_type_builder.get(i)),
            filename: self.filename_builder.get(found.filename),
        })
    }
}

pub struct StaticZipArchive {
    archive: Mutex<ZipArchive<File>>,
    sitemap: ArchiveSitemap,
}

impl StaticZipArchive {
    pub async fn new(source: &Path) -> Self {
        let files_zip_path = source.join("files.zip");
        let sitemap_path = source.join("sitemap.json");

        let archive_thread = tokio::task::spawn_blocking(|| {
            let zipfile = std::fs::File::open(files_zip_path).unwrap();
            let archive = Mutex::new(zip::ZipArchive::new(zipfile).unwrap());
            println!("Opened Archive");
            archive
        });

        let sitemap_thread = tokio::task::spawn_blocking(|| {
            let string = std::fs::read_to_string(sitemap_path).unwrap();

            let sitemap: BTreeMap<String, ParsedSiteEntry> = serde_json::from_str(&string).unwrap();

            let mut location_builder = StringLookupBuilder::new();
            let mut content_type_builder = StringLookupBuilder::new();
            let mut filename_builder = StringLookupBuilder::new();

            let mut compressed_sitemap: BTreeMap<String, CompressedSiteEntry> = BTreeMap::new();

            for (key, value) in sitemap.into_iter() {
                let compressed_value = CompressedSiteEntry {
                    status: value.status,
                    location: value.location.map(|s| location_builder.get_id(s)),
                    content_type: value.content_type.map(|s| content_type_builder.get_id(s)),
                    filename: filename_builder.get_id(value.filename),
                };

                compressed_sitemap.insert(key, compressed_value);
            }

            ArchiveSitemap {
                location_builder: location_builder.build(),
                content_type_builder: content_type_builder.build(),
                filename_builder: filename_builder.build(),
                sitemap: compressed_sitemap,
            }
        });

        let archive = archive_thread.await.unwrap();
        let sitemap_data = sitemap_thread.await.unwrap();

        Self {
            archive,
            sitemap: sitemap_data,
        }
    }

    pub fn get_response_from_uri(&self, uri: &http::Uri) -> Option<Box<dyn warp::Reply>> {
        let found_entry = self.sitemap.get(&uri.to_string())?;

        let mut archive = self.archive.lock().unwrap();
        let file = archive.by_name(&format!("files/{}", found_entry.filename));

        let mut file = if let Ok(file) = file {
            file
        } else {
            let a = Builder::new()
                .status(404)
                .header("Content-Type", "text/html")
                .body(include_bytes!("../../html/file-not-found.html").to_vec());
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

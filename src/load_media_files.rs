use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct MediaFile {
    pub path: PathBuf,
    pub category: String,
}

pub fn load(music_dir: &Path) -> Vec<MediaFile> {
    let mut media_files: Vec<MediaFile> = Vec::new();

    if let Ok(entries) = fs::read_dir(music_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(category_name) = path.file_name().and_then(|n| n.to_str()) {
                    collect_media_files(&path, category_name, &mut media_files);
                }
            }
        }
    }

    media_files
}

fn collect_media_files(dir: &Path, category: &str, media_files: &mut Vec<MediaFile>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_media_files(&path, category, media_files);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("mp4") || ext.eq_ignore_ascii_case("webm") {
                    media_files.push(MediaFile {
                        path,
                        category: category.to_string(),
                    });
                }
            }
        }
    }
}

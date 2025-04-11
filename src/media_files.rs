use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use csv::Writer;
use rand::seq::IteratorRandom;
use rand::{SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct LastChoice {
    media_file: MediaFile,
    times_chosen: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MediaFile {
    pub path: PathBuf,
    pub category: String,
    pub played: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Category {
    category: String,
    duration_overall: u64,
    current_duration: u64,
    count: u32,
    visible: bool,
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

pub fn write_media_files_to_csv(
    media_files: &Vec<MediaFile>,
    csv_path: &str,
    category_csv_path: &str,
) -> io::Result<()> {
    let file = File::create(csv_path)?;
    let mut wtr = Writer::from_writer(file);

    // Write the header
    wtr.write_record(&["category", "path", "played"])?;

    // Count categories
    let mut category_counts: HashMap<String, usize> = HashMap::new();

    // Write each media file with an initial "played" value of 0
    for media in media_files {
        let category = media.category.clone();
        let path = (*media.path.to_string_lossy()).to_string();
        let played = media.played.to_string();

        *category_counts.entry(category.clone()).or_insert(0) += 1;

        wtr.write_record(&[category, path, played])?;
    }

    // Flush the writer to ensure all data is written
    wtr.flush()?;

    // Write category summary CSV
    let category_file = File::create(category_csv_path)?;
    let mut category_writer = Writer::from_writer(category_file);

    category_writer.write_record(&[
        "category",
        "duration_overall",
        "current_duration",
        "count",
        "visible",
    ])?;

    for (category, count) in category_counts.iter() {
        category_writer.write_record(&[category, "0", "0", &count.to_string(), "true"])?;
    }

    category_writer.flush()?;

    Ok(())
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
                        played: 0,
                    });
                }
            }
        }
    }
}

pub fn choose_media_file(
    current_media_file: Option<MediaFile>,
) -> Result<Option<MediaFile>, Box<dyn Error>> {
    let cat_file = File::open("categories.csv")?;
    let media_file = File::open("media-files.csv")?;

    let mut rdr_cat = csv::Reader::from_reader(cat_file);
    let mut rdr_media = csv::Reader::from_reader(media_file);

    let categories: Vec<Category> = rdr_cat.deserialize().collect::<Result<_, _>>()?;
    let media_files: Vec<MediaFile> = rdr_media.deserialize().collect::<Result<_, _>>()?;

    for f in &media_files {
        eprintln!("file: {} played {}", f.path.display(), f.played);
    }

    let last_choice_path = "last_choice.json";

    let mut rng = create_seeded_rng();

    let (category_to_use, times_chosen) = if Path::new(last_choice_path).exists() {
        let last_choice_data = fs::read_to_string(last_choice_path)?;
        let last_choice: LastChoice = serde_json::from_str(&last_choice_data)?;

        if last_choice.times_chosen >= 4 {
            // pick a different category
            let other_categories: Vec<&Category> = categories
                .iter()
                .filter(|cat| cat.visible && cat.category != last_choice.media_file.category)
                .collect();

            if let Some(new_cat) = other_categories.into_iter().choose(&mut rng) {
                (new_cat.category.clone(), 1)
            } else {
                eprintln!("no other category available, so take the last one anyway");
                (
                    last_choice.media_file.category,
                    last_choice.times_chosen + 1,
                )
            }
        } else {
            (
                last_choice.media_file.category.clone(),
                last_choice.times_chosen + 1,
            )
        }
    } else {
        // first time use
        if let Some(cat) = categories.iter().filter(|cat| cat.visible).choose(&mut rng) {
            (cat.category.clone(), 1)
        } else {
            eprintln!("first time use: cannot choose randomly");
            return Ok(None);
        }
    };

    eprintln!("search for next song: choose category {}", category_to_use);

    // Pick unplayed file from the chosen category
    let candidates: Vec<MediaFile> = media_files
        .clone()
        .into_iter()
        .filter(|f| match &current_media_file {
            Some(current_file) => {
                f.category == category_to_use && f.played == 0 && f.path != current_file.path
            }
            None => f.category == category_to_use && f.played == 0,
        })
        .collect();

    eprintln!("search for next song: {} candidates", candidates.len());
    for f in &candidates {
        eprintln!("candidates: {} played: {}", f.path.display(), f.played);
    }

    if let Some(selected) = candidates.into_iter().choose(&mut rng) {
        // Save the new choice
        let last_choice = LastChoice {
            media_file: selected.clone(),
            times_chosen,
        };

        let serialized = serde_json::to_string_pretty(&last_choice)?;
        fs::write(last_choice_path, serialized)?;

        eprintln!(
            "next song is one played not before: {}",
            selected.path.display()
        );

        return Ok(Some(selected));
    }

    // Now already played files ...
    let candidates: Vec<MediaFile> = media_files.into_iter().collect();
    eprintln!(
        "search under already played songs: {} candidates",
        candidates.len()
    );

    if let Some(selected) = candidates.into_iter().choose(&mut rng) {
        // Save the new choice
        let last_choice = LastChoice {
            media_file: selected.clone(),
            times_chosen,
        };

        let serialized = serde_json::to_string_pretty(&last_choice)?;
        fs::write(last_choice_path, serialized)?;

        eprintln!(
            "next song was played before since all were played already: {}",
            selected.path.display()
        );
        return Ok(Some(selected));
    }

    Ok(None)
}

pub fn update_play_info(
    media_file: &MediaFile,
    duration: u64,
    category_change: bool,
) -> Result<(), Box<dyn Error>> {
    let category_name = &media_file.category;
    let file_path = &media_file.path;

    let cat_path = "categories.csv";
    let media_path = "media-files.csv";

    let cat_file = File::open(cat_path)?;
    let media_file = File::open(media_path)?;

    let mut rdr_cat = csv::Reader::from_reader(cat_file);
    let mut rdr_media = csv::Reader::from_reader(media_file);

    let mut categories: Vec<Category> = rdr_cat.deserialize().collect::<Result<_, _>>()?;
    let mut media_files: Vec<MediaFile> = rdr_media.deserialize().collect::<Result<_, _>>()?;

    for cat in categories.iter_mut() {
        if cat.category == *category_name {
            if category_change {
                cat.current_duration = 0;
            } else {
                cat.current_duration += duration;
            }
            cat.duration_overall += duration;
            eprintln!("update CSV: current_duration: {}", cat.current_duration);
            eprintln!("update CSV: duration_overall: {}", cat.duration_overall);
            break;
        }
    }

    for media in media_files.iter_mut() {
        if media.category == *category_name && media.path == *file_path {
            media.played += 1;
            eprintln!("update CSV: played: {}", media.played);
            break;
        }
    }

    // Write updated categories
    let mut wtr_cat = csv::Writer::from_path(cat_path)?;
    for cat in &categories {
        wtr_cat.serialize(cat)?;
    }
    wtr_cat.flush()?;

    // Write updated media
    let mut wtr_media = csv::Writer::from_path(media_path)?;
    for media in &media_files {
        wtr_media.serialize(media)?;
    }
    wtr_media.flush()?;

    Ok(())
}

fn create_seeded_rng() -> StdRng {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    // Combine seconds and nanoseconds into one u64 seed
    let seed = now.as_secs() ^ now.subsec_nanos() as u64;

    StdRng::seed_from_u64(seed)
}

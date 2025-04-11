use std::path::Path;

mod commands;
mod media_files;
mod state_machine;

fn main() -> std::io::Result<()> {
    let media_files_csv = "media-files.csv";
    let categories_csv = "categories.csv";

    if !Path::new(media_files_csv).exists() || !Path::new(categories_csv).exists() {
        let media_files = media_files::load(Path::new("/home/micki/1tb/Music"));

        media_files::write_media_files_to_csv(&media_files, media_files_csv, categories_csv)?;
    }

    state_machine::play();

    eprintln!("main finished, quit now");

    Ok(())
}

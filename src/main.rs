use std::path::Path;

mod commands;
mod load_media_files;
mod state_machine;

fn main() {
    let media_files = load_media_files::load(Path::new("/home/micki/1tb/Music"));

    state_machine::play(&media_files);

    eprintln!("main finished, quit now");
}

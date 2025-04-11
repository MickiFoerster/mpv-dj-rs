use std::process::{Child, Command};

use std::{thread, time::Duration};

use serde_json::json;

use crate::commands::{
    get_duration, get_playback_time, quit, send_msg, start_video, wait_for_the_end,
};
use crate::media_files::{self, MediaFile};

pub fn play() {
    let mut from = 0;
    let mut to = 1;
    let mut children: Vec<Child> = Vec::with_capacity(2);

    let media_file_from = get_next_song();

    let mut socket_path_from = format!("/tmp/mpv{from}.socket");
    let mut socket_path_to: String;

    let _ = std::fs::remove_file(&socket_path_from);

    let mut cmd = Command::new("mpv");
    let cmd = cmd
        .arg("--idle")
        .arg("--no-terminal")
        .arg("--quiet")
        .arg("--fs")
        .arg("--fs-screen=1")
        .arg(format!("--input-ipc-server={}", socket_path_from));

    eprintln!("{cmd:#?}");
    children.insert(from, cmd.spawn().expect("Failed to spawn mpv process"));

    loop {
        if std::path::Path::new(&socket_path_from).exists() {
            break;
        } else {
            eprintln!("Cannot see IPC socket yet, waiting ...");
            thread::sleep(Duration::from_millis(100));
        }
    }

    start_video(&socket_path_from, &media_file_from.path, 100).expect("Failed to start video");

    loop {
        wait_for_the_end(&socket_path_from);

        // Now time to start next video
        let media_file_to = get_next_song();

        socket_path_to = format!("/tmp/mpv{to}.socket");
        let _ = std::fs::remove_file(&socket_path_to);

        let mut cmd = Command::new("mpv");
        let cmd = cmd
            .arg("--idle")
            .arg("--no-terminal")
            .arg("--quiet")
            .arg("--fs")
            .arg(format!("--fs-screen={}", to + 1))
            .arg(format!("--input-ipc-server={}", socket_path_to));

        eprintln!("{cmd:#?}");
        children.insert(to, cmd.spawn().expect("Failed to spawn mpv process"));

        loop {
            if std::path::Path::new(&socket_path_to).exists() {
                break;
            } else {
                eprintln!("Cannot see IPC socket yet, waiting ...");
                thread::sleep(Duration::from_millis(100));
            }
        }

        start_video(&socket_path_to, &media_file_to.path, 0).expect("Failed to start video");

        eprintln!("Begin fading out of {from} and in of {to} ...");

        if let Some(playback_time) = get_playback_time(&socket_path_from) {
            if let Some(duration) = get_duration(&socket_path_from) {
                let time_difference = duration - playback_time;

                loop {
                    // set volume of instance 0
                    if let Some(playback_time) = get_playback_time(&socket_path_from) {
                        if let Some(duration) = get_duration(&socket_path_from) {
                            let volume = (duration - playback_time) * 100. / time_difference;

                            eprintln!("set volume of instance {from}: {volume}");
                            let msg =
                                json!({ "command": ["set_property", "volume", volume.trunc()] });
                            let _ = send_msg(&socket_path_from, msg);

                            if volume < 40. {
                                break;
                            }

                            // set volume of instance 1
                            if let Some(playback_time) = get_playback_time(&socket_path_to) {
                                let volume = playback_time * 100. / 10.;
                                let volume = if volume <= 100. { volume } else { 100. };

                                eprintln!("set volume of instance {to}: {volume}");
                                let msg = json!({ "command": ["set_property", "volume", volume.trunc()] });
                                let _ = send_msg(&socket_path_to, msg);
                            } else {
                                break;
                            }

                            thread::sleep(Duration::from_millis(500));
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        let msg = json!({ "command": ["set_property", "volume", 0] });
        let _ = send_msg(&socket_path_from, msg);

        let msg = json!({ "command": ["set_property", "volume", 100] });
        let _ = send_msg(&socket_path_to, msg);

        if let Some(duration) = get_duration(&socket_path_from) {
            // update CSV files
            match media_files::update_play_info(
                &media_file_from,
                duration.round() as u64,
                media_file_from.category != media_file_to.category,
            ) {
                Ok(_) => eprintln!("CSV files updated successfully"),
                Err(e) => eprintln!("Failed to update CSV files: {e}"),
            };
        }

        loop {
            if let Some(playback_time) = get_playback_time(&socket_path_from) {
                if let Some(duration) = get_duration(&socket_path_from) {
                    if playback_time < duration {
                        eprintln!("Wait for old video to finish ...");
                        thread::sleep(Duration::from_millis(500));
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let _ = quit(&socket_path_from);
        match children[from].kill() {
            Ok(_) => eprintln!("Old process successfully killed."),
            Err(e) => eprintln!("Old process could not be killed: {e}"),
        };

        let tmp = from;
        from = to;
        to = tmp;

        socket_path_from = format!("/tmp/mpv{from}.socket");
    }
}

fn get_next_song() -> MediaFile {
    let media_file = crate::media_files::choose_media_file()
        .expect("Failed to get a media file from CSV files.")
        .expect("Failed to choose randomly a file from the list of available files.");

    media_file
}

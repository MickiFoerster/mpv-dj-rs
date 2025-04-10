use std::process::{Child, Command};

use std::{path::PathBuf, thread, time::Duration};

use serde_json::json;

use crate::{
    commands::{get_duration, get_playback_time, send_msg, start_video, wait_for_the_end},
    load_media_files::MediaFile,
};

pub struct MpvInstance {
    pub socket: String,
    pub process: Child,
}

pub fn play(media_files: &Vec<MediaFile>) {
    let path = get_next_song(&media_files);

    let mut children: Vec<MpvInstance> = vec![];

    for i in 0..2 {
        match spawn_mpv_with_ipc(i) {
            Ok(child) => {
                let ipc_path = &child.socket;

                println!("mpv launched successfully with IPC socket {ipc_path}.");

                children.push(child);
            }
            Err(e) => {
                eprintln!("Failed to start mpv: {}", e);
            }
        }
    }

    start_video(&children, 0, &path, 100).expect("Failed to start video");

    let mut from = 0;
    let mut to = 1;
    loop {
        let _ = wait_for_the_end(&children, from);

        let path = get_next_song2(media_files);
        start_video(&children, to, &path, 0).expect("Failed to start video");

        let socket_path_from: String = children
            .get(from)
            .expect("Failed to get mpv instance")
            .socket
            .clone();

        let socket_path_to: String = children
            .get(to)
            .expect("Failed to get mpv instance")
            .socket
            .clone();

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
                            send_msg(&socket_path_from, msg)
                                .expect("Failed to send volume command");

                            if volume < 1. {
                                break;
                            }

                            // set volume of instance 1
                            let playback_time = get_playback_time(&socket_path_to)
                                .expect("Failed to get playback time");
                            let volume = playback_time * 100. / time_difference;
                            let volume = if volume <= 100. { volume } else { 100. };

                            eprintln!("set volume of instance {to}: {volume}");
                            let msg =
                                json!({ "command": ["set_property", "volume", volume.trunc()] });
                            send_msg(&socket_path_to, msg).expect("Failed to send volume command");

                            thread::sleep(Duration::from_millis(1000));
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

        from = to;
        to = from;
    }
}

fn get_next_song(media_files: &Vec<MediaFile>) -> PathBuf {
    media_files.get(10).expect("non-empty vector").path.clone()
}
fn get_next_song2(media_files: &Vec<MediaFile>) -> PathBuf {
    media_files.get(20).expect("non-empty vector").path.clone()
}

fn spawn_mpv_with_ipc(i: u32) -> std::io::Result<MpvInstance> {
    let socket_path = format!("/tmp/mpv{i}.socket");

    // Clean up any potential existing socket first
    let _ = std::fs::remove_file(&socket_path);

    let child = Command::new("mpv")
        .arg("--idle")
        .arg("--no-terminal")
        .arg("--quiet")
        .arg("--fs")
        .arg(format!("--fs-screen={}", i + 1))
        .arg(format!("--input-ipc-server={}", socket_path))
        .spawn()
        .expect("Failed to spawn mpv process");

    loop {
        if std::path::Path::new(&socket_path).exists() {
            break;
        } else {
            eprintln!("Cannot see IPC socket yet, waiting ...");
            thread::sleep(Duration::from_millis(100));
        }
    }

    Ok(MpvInstance {
        socket: socket_path,
        process: child,
    })
}

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use std::{thread, time::Duration};

use serde_json::json;

fn _full_screen(socket_path: &str, screen: usize) -> Result<(), String> {
    let msg = json!({ "command": ["set_property", "fullscreen", false] });
    send_msg(&socket_path, msg)?;

    let msg = json!({ "command": ["set_property", "fs-screen", screen] });
    send_msg(&socket_path, msg)?;

    let msg = json!({ "command": ["set_property", "fullscreen", true] });
    send_msg(&socket_path, msg)?;

    Ok(())
}

pub fn quit(socket_path: &str) -> Result<(), String> {
    let msg = json!({ "command": ["quit"] });
    let result = send_msg(&socket_path, msg)?;
    eprintln!("{result:#}");

    Ok(())
}

pub fn get_video_path(socket_path: &str) -> Option<String> {
    let msg = json!({ "command": ["get_property", "path"] });
    let response = send_msg(socket_path, msg).ok()?;

    response.get("data").map(|v| v.to_string())
}

fn set_volume(socket_path: &str, volume: u8) -> Result<(), String> {
    let msg = json!({ "command": ["set_property", "volume", volume] });
    let _result = send_msg(&socket_path, msg)?;
    Ok(())
}

pub fn get_playback_time(socket_path: &str) -> Option<f64> {
    let msg = json!({
        "command": ["get_property", "time-pos"],
    });

    let response = send_msg(socket_path, msg).ok()?;

    response.get("data").map(|v| v.as_f64())?
}

pub fn get_duration(socket_path: &str) -> Option<f64> {
    let msg = json!({
        "command": ["get_property", "duration"],
    });

    let response = send_msg(socket_path, msg).ok()?;

    response.get("data").map(|v| v.as_f64())?
}

pub fn wait_for_the_end(socket_path: &str) {
    eprintln!("Wait until 30 seconds before end of the video ...");
    loop {
        if let Some(playback_time) = get_playback_time(&socket_path) {
            if let Some(duration) = get_duration(&socket_path) {
                let video_path =
                    get_video_path(socket_path).expect("Failed to get path of current video");
                let percent = playback_time * 100. / duration;
                eprintln!(
                    "instance{socket_path}: {video_path}: {playback_time:.0} / {duration:.0} ({percent:.0}%)"
                );

                if duration - playback_time <= 30.0 {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }

        thread::sleep(Duration::from_millis(500));
    }
}

pub fn start_video(socket_path: &str, path: &Path, volume: u8) -> Result<(), String> {
    let msg = json!({ "command": ["loadfile", path, "replace"], });
    send_msg(&socket_path, msg)?;

    set_volume(&socket_path, volume)?;
    //full_screen(&socket_path, instance_id + 1)?;

    // Wait until get_duration is successful
    eprintln!("Wait until get_duration is successful");
    loop {
        if let Some(_) = get_playback_time(&socket_path) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    eprintln!("Video is running and playback time can be retrieved");

    Ok(())
}

pub fn send_msg(socket_path: &str, msg: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| format!("Failed to connect to UNIX socket: {e}"))?;
    let reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| format!("Failed to create buffer reader: {e}"))?,
    );

    //eprintln!("send to {}: {}", socket_path, msg.to_string());
    writeln!(stream, "{}", msg.to_string())
        .map_err(|e| format!("Cannot write to UNIX socket: {e}"))?;

    for line in reader.lines() {
        if let Ok(text) = line {
            //eprintln!("response from mpv: {text}");
            let result: Result<serde_json::Value, _> = serde_json::from_str(&text);
            if let Ok(parsed) = result {
                if let Some(value) = parsed.get("error") {
                    if value == "success" {
                        return Ok(parsed);
                    } else {
                        return Err(value.to_string());
                    }
                }

                return Err("Unexpected structure of JSON returned by mpv".into());
            }
        }
    }

    Err("Response from mpv is not valid JSON".into())
}

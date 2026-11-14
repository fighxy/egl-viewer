//! Plays a file on the default audio device and reports what the engine does, to check the
//! player by ear and by numbers without the app:
//! `cargo run -p av --example play -- <file> [seconds]`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: play <file> [seconds]");
        std::process::exit(2);
    };
    let seconds: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let player = match av::Player::open(&path, av::PlayerOptions::default()) {
        Ok(player) => player,
        Err(error) => {
            eprintln!("cannot open {}: {error}", path.display());
            std::process::exit(1);
        }
    };
    println!(
        "{}: {:?}, sound on a device: {}",
        path.display(),
        player.info(),
        player.has_sound()
    );
    let start = Instant::now();
    let mut frames = 0;
    let mut last_report = 0.0;
    let mut seeked = false;
    while start.elapsed().as_secs_f64() < seconds && !player.ended() {
        if player.take_frame().is_some() {
            frames += 1;
        }
        let wall = start.elapsed().as_secs_f64();
        if wall - last_report >= 0.5 {
            last_report = wall;
            println!(
                "wall {wall:5.2} s  position {:6.3} s  frames shown {frames}",
                player.position()
            );
        }
        if !seeked && wall >= seconds / 2.0 {
            seeked = true;
            let target = player.duration().map_or(1.0, |d| d * 0.75);
            println!("seek to {target:.2} s");
            player.seek(target, av::SeekMode::Exact);
        }
        std::thread::sleep(
            player
                .next_frame_in()
                .unwrap_or(Duration::from_millis(10))
                .min(Duration::from_millis(20)),
        );
    }
    println!(
        "done: position {:.3} s, frames shown {frames}, ended {}",
        player.position(),
        player.ended()
    );
}

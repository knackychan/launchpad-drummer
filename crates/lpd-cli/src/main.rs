mod audio_out;
mod midi;
mod player;

use anyhow::{bail, Context, Result};
use lpd_core::audio::Command;
use lpd_core::kit::{self, Pad, Set};
use player::{Action, Player};
use std::path::{Path, PathBuf};

const USAGE: &str = "usage:
  lpd-cli new-set <set-dir> <samples-folder>
  lpd-cli play <set-dir> [--asio] [--buffer <frames>]";

/// Colours for new sets (Launchpad 0..63 RGB), one per row.
const ROW_COLOURS: [[u8; 3]; 8] = [
    [63, 0, 0],
    [63, 20, 0],
    [63, 50, 0],
    [0, 63, 0],
    [0, 63, 40],
    [0, 30, 63],
    [30, 0, 63],
    [63, 0, 40],
];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("new-set") if args.len() == 3 => new_set(Path::new(&args[1]), Path::new(&args[2])),
        Some("play") if args.len() >= 2 => {
            let asio = args.iter().any(|a| a == "--asio");
            let buffer = match args.iter().position(|a| a == "--buffer") {
                Some(i) => Some(
                    args.get(i + 1)
                        .context("--buffer needs a value")?
                        .parse::<u32>()
                        .context("--buffer must be a number")?,
                ),
                None => None,
            };
            play(Path::new(&args[1]), asio, buffer)
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn new_set(dir: &Path, samples: &Path) -> Result<()> {
    if dir.join(kit::SET_FILE).exists() {
        bail!("{} already contains a set", dir.display());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(samples)
        .with_context(|| format!("read {}", samples.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            matches!(ext.as_str(), "wav" | "mp3" | "flac" | "ogg")
        })
        .collect();
    files.sort();
    let mut set = Set::new();
    for (i, file) in files.iter().take(64).enumerate() {
        let (x, y) = ((i % 8) as u8, (i / 8) as u8);
        let rel = kit::import_sample(dir, file)?;
        set.banks[0].set_pad(Pad::new(x, y, rel, ROW_COLOURS[y as usize]));
    }
    kit::save(dir, &set)?;
    println!(
        "Created {} with {} pads in bank 1.",
        dir.display(),
        files.len().min(64)
    );
    Ok(())
}

fn play(dir: &Path, asio: bool, buffer: Option<u32>) -> Result<()> {
    let loaded = kit::load(dir)?;
    if loaded.from_backup {
        println!(
            "set.json was unreadable: loaded set.json.bak (broken file kept as set.json.broken)."
        );
    }
    let (samples, failed) = kit::load_samples(dir, &loaded.set);
    for (path, err) in &failed {
        println!("Sample not loaded, pad shows red: {path}: {err:#}");
    }
    let mut player = Player::new(loaded.set, samples);
    let audio_out::AudioOut {
        stream: _stream,
        commands: mut audio_tx,
        device_name,
        sample_rate,
        buffer,
    } = audio_out::start(asio, buffer)?;
    println!("Audio: {device_name} @ {sample_rate} Hz, buffer {buffer:?}");
    for cmd in player.mixer_commands() {
        push(&mut audio_tx, cmd);
    }
    let (lp_in, mut lp_out) = midi::connect_launchpad()?;
    lp_out.send_leds(&player.paint())?;
    println!("Playing. Side buttons = banks, Mixer = stop all, Ctrl+C = quit.");
    for ev in lp_in.events.iter() {
        for action in player.on_event(ev) {
            match action {
                Action::Audio(cmd) => push(&mut audio_tx, cmd),
                Action::Leds(leds) => lp_out.send_leds(&leds)?,
            }
        }
    }
    Ok(())
}

fn push(tx: &mut rtrb::Producer<Command>, cmd: Command) {
    if tx.push(cmd).is_err() {
        eprintln!("audio command queue full; command dropped");
    }
}

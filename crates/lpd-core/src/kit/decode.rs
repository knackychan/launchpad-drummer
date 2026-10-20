use super::Set;
use crate::audio::Sample;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode an audio file (WAV, FLAC, MP3, OGG) to stereo f32. Mono is duplicated,
/// channels beyond the first two are dropped.
pub fn decode_file(path: &Path) -> Result<Sample> {
    let file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("unrecognised audio format: {}", path.display()))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .context("no audio track")?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut data = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let spec = *decoded.spec();
        rate = spec.rate;
        let channels = spec.channels.count();
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        for frame in buf.samples().chunks(channels) {
            let l = frame[0];
            let r = if channels > 1 { frame[1] } else { l };
            data.push(l);
            data.push(r);
        }
    }
    anyhow::ensure!(!data.is_empty(), "no audio decoded from {}", path.display());
    Ok(Sample { rate, data })
}

/// The sample paths a set references, each once, in the order they are first referenced.
pub fn unique_sample_paths(set: &Set) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    set.banks
        .iter()
        .flat_map(|b| &b.pads)
        .filter(|pad| seen.insert(pad.sample.as_str()))
        .map(|pad| pad.sample.clone())
        .collect()
}

/// Decode every sample the set references. Failures are returned per path; the rest load.
pub fn load_samples(
    dir: &Path,
    set: &Set,
) -> (HashMap<String, Arc<Sample>>, Vec<(String, anyhow::Error)>) {
    load_samples_with_progress(dir, set, |_, _, _| {})
}

/// [`load_samples`], reporting `(completed, total, failed)` over the unique sample paths: once
/// before the first file (also for an empty set: 0, 0, 0) and after every file, failed ones
/// included. The callback runs on the calling thread between files; keep it cheap.
pub fn load_samples_with_progress(
    dir: &Path,
    set: &Set,
    progress: impl Fn(usize, usize, usize),
) -> (HashMap<String, Arc<Sample>>, Vec<(String, anyhow::Error)>) {
    let paths = unique_sample_paths(set);
    let total = paths.len();
    let mut ok = HashMap::new();
    let mut failed: Vec<(String, anyhow::Error)> = Vec::new();
    progress(0, total, 0);
    for (i, path) in paths.into_iter().enumerate() {
        match decode_file(&dir.join(&path)) {
            Ok(s) => {
                ok.insert(path, Arc::new(s));
            }
            Err(e) => failed.push((path, e)),
        }
        progress(i + 1, total, failed.len());
    }
    (ok, failed)
}

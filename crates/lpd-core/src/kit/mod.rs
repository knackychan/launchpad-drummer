//! Sets, banks and pads: data model and files.

mod decode;
mod history;
pub mod learn;
mod model;
pub mod ops;
mod params;
mod starter;
mod store;

pub use decode::{decode_file, load_samples};
pub use history::{restore_keeping_mixer, History, UNDO_STEPS};
pub use model::*;
pub use params::{group_params, play_params};
pub use starter::{starter_set, write_starter_samples, DEFAULT_SET_NAME};
pub use store::{
    ensure_new_set_dir, import_sample, load, save, Loaded, BACKUP_FILE, BROKEN_FILE, SAMPLES_DIR,
    SET_FILE,
};

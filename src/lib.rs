#![forbid(unsafe_code)]
#![deny(non_snake_case)]

use std::{path::{Path, PathBuf}, sync::Arc};

mod index;
mod progress;

mod crypto;
pub use crypto::{generate_key, Key};

mod pack;
pub use pack::pack;

mod archive;
pub use archive::Archive;

const BKY_HEADER: &[u8] = b"backy archive v0.2\n";

#[derive(Clone, Debug)]
struct Source {
	id: Arc<str>,
	is_file: bool,
	path: Arc<Path>,
}

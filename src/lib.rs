#![forbid(unsafe_code)]
#![deny(non_snake_case)]

use std::{path::{Path, PathBuf}, sync::Arc};

mod index;
mod group;
mod progress;

mod crypto;
pub use crypto::{generate_key, Key};

mod pack;
pub use pack::pack;

mod unpack;
pub use unpack::unpack;

const BKY_HEADER: &[u8] = b"backy archive v1\n";

#[derive(Clone, Debug)]
struct Source {
	id: Arc<str>,
	path: Arc<Path>,
}

#[derive(Debug)]
struct Entry {
	source: Source,
	path: PathBuf,
	size: u64,
}

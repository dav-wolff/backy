#![forbid(unsafe_code)]
#![deny(non_snake_case)]

use std::{path::Path, sync::Arc};

mod index;
mod progress;

mod header;
mod crypto;
pub use crypto::{generate_key, Key};

mod pack;
pub use pack::pack;

mod archive;
pub use archive::Archive;

const BKY_HEADER: &[u8] = b"backy archive v0.2\n";

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Source {
	id: Arc<str>,
	// TODO: is this really necessary?
	is_file: bool,
	path: Arc<Path>,
}

#![forbid(unsafe_code)]
#![deny(non_snake_case)]


mod index;
mod group;

mod pack;
use std::{path::{Path, PathBuf}, rc::Rc};

pub use pack::pack;

mod unpack;
pub use unpack::unpack;

const BKY_HEADER: &[u8] = b"backy archive v1\n";

#[derive(Clone, Debug)]
struct Source {
	id: Rc<str>,
	path: Rc<Path>,
}

#[derive(Debug)]
struct Entry {
	source: Source,
	path: PathBuf,
	size: u64,
}

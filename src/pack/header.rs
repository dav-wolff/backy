use std::{collections::BTreeMap, io::{self, Write}, path::{Path, PathBuf}};

use blake3::Hash;

use crate::index::Sources;

struct Key {
	source: String,
	path: PathBuf,
}

pub struct Header {
	entries: BTreeMap<Key, ()>,
}

impl Header {
	pub fn new(sources: &Sources) -> Self {
		todo!()
	}
	
	pub fn header_size(&self) -> u64 {
		todo!()
	}
	
	pub fn set_entry(&mut self, source: &str, entry: &Path, size: u64, hash: Hash) {
		todo!()
	}
	
	pub fn write_header(writer: impl Write) -> Result<(), io::Error> {
		todo!()
	}
}

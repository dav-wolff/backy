use std::{fmt::Display, path::Path};

use super::*;

// TODO: should this contain a string? Paths should be UTF-8 for compatibility
/// Relative path of an entry
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct EntryPath(String);

impl EntryPath {
	pub fn empty() -> Self {
		Self(String::new())
	}
	
	// TODO: is this a false positive?
	#[expect(private_interfaces)]
	pub fn new(source: &Source, path: &Path) -> Self {
		Self(
			path.strip_prefix(&source.path).expect("path must be inside source")
				.to_str().unwrap() // TODO: return error
				.to_owned()
		)
	}
	
	pub fn from_bytes(bytes: Vec<u8>) -> Self {
		Self(String::from_utf8(bytes).unwrap()) // TODO: return error
	}
	
	pub fn in_source(&self, source: &Source) -> PathBuf {
		source.path.join(&self.0)
	}
	
	pub fn as_bytes(&self) -> &[u8] {
		self.0.as_bytes()
	}
	
	pub fn as_path(&self) -> &Path {
		Path::new(&self.0)
	}
	
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Display for EntryPath {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}

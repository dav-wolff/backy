use std::{fmt::Display, path::Path};

use anyhow::anyhow;

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
	// EntryPath is neither re-exported from lib.rs, nor is the index module public in lib.rs
	#[expect(private_interfaces)]
	pub fn new(source: &Source, path: &Path) -> anyhow::Result<Self> {
		Ok(Self(
			path.strip_prefix(&source.path).expect("path must be inside source")
				.to_str().with_context(|| format!("path {path:?} is not representable as UTF-8"))?
				.to_owned()
		))
	}
	
	pub fn from_bytes(bytes: Vec<u8>) -> anyhow::Result<Self> {
		let string = String::from_utf8(bytes).map_err(|err| {
			let context = format!("invalid entry path: {:?}", String::from_utf8_lossy(err.as_bytes()));
			anyhow!(err).context(context)
		})?;
		Ok(Self(string))
	}
	
	// TODO: same as above, is this a false positive?
	#[expect(private_interfaces)]
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

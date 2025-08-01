use std::{
	ops::Deref,
	cmp::Ordering,
};
use blake3::{
	Hash,
};

/// Newtype for the hash of a file's contents
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FileHash(pub Hash);

/// Newtype for the hash of an archive or subarchive's header
/// and by extension its contents
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HeaderHash(pub Hash);

/// Newtype for the hash of multiple [`HeaderHash`]es
/// to identify an archive consisting of multiple subarchives
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MultiHeaderHash(pub Hash);

impl MultiHeaderHash {
	pub fn from_hashes(mut hashes: Vec<HeaderHash>) -> Self {
		// sort header hashes for a stable multi header hash that doesn't depend on the header hashes' order
		hashes.sort_by_key(|hash| *hash.as_bytes());
		
		let mut hasher = blake3::Hasher::new();
		
		for hash in hashes {
			hasher.update(hash.as_bytes());
		}
		
		Self(hasher.finalize())
	}
}

/// Hash identifying the origin of a file, the subarchive it
/// belongs to, and the overarching archive the subarchive belongs
/// to if it does
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum OriginHash {
	Single(HeaderHash),
	Multi(MultiHeaderHash, HeaderHash),
}

impl PartialOrd for OriginHash {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for OriginHash {
	fn cmp(&self, other: &Self) -> Ordering {
		use OriginHash::*;
		
		match (self, other) {
			(Multi(_, _), Single(_)) => Ordering::Greater,
			(Single(_), Multi(_, _)) => Ordering::Less,
			(Single(hash), Single(other_hash)) =>
				hash.as_bytes().cmp(other_hash.as_bytes()),
			(Multi(multi_hash, hash), Multi(other_multi_hash, other_hash)) =>
				multi_hash.as_bytes().cmp(other_multi_hash.as_bytes())
					.then_with(|| hash.as_bytes().cmp(other_hash.as_bytes())),
		}
	}
}

impl Deref for FileHash {
	type Target = Hash;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl Deref for HeaderHash {
	type Target = Hash;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl Deref for MultiHeaderHash {
	type Target = Hash;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

use std::{
	borrow::Borrow,
	io::{self, Read, Seek, SeekFrom},
};

use anyhow::{
	ensure,
	Context as _,
};

use crate::{
	BKY_HEADER
	Key,
	crypto::{DecryptReader, IV},
	hashing_reader::HashingReader,
	header::{self, Header, FileHash, HeaderHash},
	index::EntryPath,
};

pub struct SubArchive<R: Read + Seek> {
	decrypter: DecryptReader<R>,
	contents_start: u64,
	header: Header,
}

impl<R: Read + Seek> SubArchive<R> {
	pub fn new(mut reader: R, key: Key) -> anyhow::Result<Self> {
		let mut bky_header = [0u8; BKY_HEADER.len()];
		reader.read_exact(&mut bky_header).context("not a backy archive")?;
		
		ensure!(bky_header == BKY_HEADER, "not a backy archive");
		
		let mut iv = IV::default();
		reader.read_exact(&mut iv)?;
		let mut decrypter = DecryptReader::new(reader, key, iv)?;
		
		let header = Header::read_from(&mut decrypter).context("parsing header")?;
		let contents_start = decrypter.stream_position()?;
		
		// TODO: should this be a hard error? a warning might be too easy to miss
		// could potentially add a --allow-partial flag to explicitly unpack partial archives
		if header.flags().is_partial {
			eprintln!("Warning: Partial subarchive encountered, some files are missing");
		}
		
		Ok(Self {
			decrypter,
			contents_start,
			header,
		})
	}
	
	pub fn is_single_source(&self) -> bool {
		self.header.flags().is_single_source
	}
	
	pub fn contents_start(&self) -> u64 {
		self.contents_start
	}
	
	pub fn sources(&self) -> impl Iterator<Item = &str> {
		self.header.entries()
			.keys()
			.map(Borrow::borrow)
	}
	
	pub fn file_paths(&self) -> impl Iterator<Item = &str> {
		self.header.entries()
			.values()
			.flatten()
			.map(|entry| entry.path.as_str())
	}
	
	pub fn hash(&self) -> HeaderHash {
		self.header.hash()
	}
	
	pub fn file_hashes(&self) -> impl Iterator<Item = FileHash> {
		self.header.entries()
			.values()
			.flatten()
			.map(|entry| entry.hash)
	}
	
	pub fn read_file<'s>(&'s mut self, source: &str, path: &EntryPath) -> anyhow::Result<Option<ArchiveReader<impl Read + use<'s, R>>>> {
		let Some(source) = self.header.entries().get(source) else {
			return Ok(None);
		};
		let Some(entry) = source.iter().find(|entry| &entry.path == path) else {
			return Ok(None);
		};
		
		self.decrypter.seek(SeekFrom::Start(self.contents_start + entry.position)).context("seeking to file contents")?;
		
		let reader = HashingReader::new((&mut self.decrypter).take(entry.size));
		
		Ok(Some(ArchiveReader {
			reader,
			expected_hash: entry.hash,
		}))
	}
	
	pub fn for_each_file<F, E>(&mut self, mut callback: F) -> Result<(), E>
	where
		F: FnMut(&str, &header::FileInfo, ArchiveReader<io::Take<&mut DecryptReader<R>>>) -> Result<(), E>,
		E: From<anyhow::Error>,
	{
		self.decrypter.seek(SeekFrom::Start(self.contents_start)).context("seeking to start of contents")?;
		
		for (source, entries) in self.header.entries() {
			for entry in entries {
				let reader = HashingReader::new((&mut self.decrypter).take(entry.size));
				callback(source, entry, ArchiveReader {
					reader,
					expected_hash: entry.hash,
				})?;
				// TODO: ensure that reader is fully read?
			}
		}
		
		Ok(())
	}
}

pub struct ArchiveReader<R: Read> {
	reader: HashingReader<R>,
	expected_hash: FileHash,
}

impl<R: Read> Read for ArchiveReader<R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.reader.read(buf)
	}
}

impl<R: Read> ArchiveReader<R> {
	pub fn is_corrupted(&self) -> bool {
		self.reader.finalize() != *self.expected_hash
	}
	
	pub fn into_inner(self) -> R {
		self.reader.into_inner()
	}
}


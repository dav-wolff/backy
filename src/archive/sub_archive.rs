use std::{borrow::Borrow, io::{self, Read, Seek, SeekFrom}};

use anyhow::{ensure, Context};

use crate::{crypto::{DecryptReader, IV}, header::{self, Header}, index::EntryPath, Key, BKY_HEADER};

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
	
	pub fn read_file<'s>(&'s mut self, source: &str, path: &EntryPath) -> anyhow::Result<Option<impl Read + use<'s, R>>> {
		let Some(source) = self.header.entries().get(source) else {
			return Ok(None);
		};
		let Some(entry) = source.iter().find(|entry| &entry.path == path) else {
			return Ok(None);
		};
		
		self.decrypter.seek(SeekFrom::Start(self.contents_start + entry.position)).context("seeking to file contents")?;
		
		Ok(Some((&mut self.decrypter).take(entry.size)))
	}
	
	pub fn for_each_file<F, E>(&mut self, mut callback: F) -> Result<(), E>
	where
		F: FnMut(&str, &header::FileInfo, io::Take<&mut DecryptReader<R>>) -> Result<(), E>,
		E: From<anyhow::Error>,
	{
		self.decrypter.seek(SeekFrom::Start(self.contents_start)).context("seeking to start of contents")?;
		
		for (source, entries) in self.header.entries() {
			for entry in entries {
				let reader = (&mut self.decrypter).take(entry.size);
				callback(source, entry, reader)?;
				// TODO: ensure that reader is fully read?
			}
		}
		
		Ok(())
	}
}

use std::{borrow::Borrow, io::{self, Read, Seek, SeekFrom}};

use crate::{crypto::{DecryptReader, IV}, header::Header, index::EntryPath, Key, BKY_HEADER};

pub struct SubArchive<R: Read + Seek> {
	decrypter: DecryptReader<R>,
	contents_start: u64,
	header: Header,
}

impl<R: Read + Seek> SubArchive<R> {
	pub fn new(mut reader: R, key: Key) -> Result<Self, io::Error> {
		let mut bky_header = [0u8; BKY_HEADER.len()];
		reader.read_exact(&mut bky_header)?;
		
		if bky_header != BKY_HEADER {
			panic!("Not a backy archive");
		}
		
		let mut iv = IV::default();
		reader.read_exact(&mut iv)?;
		let mut decrypter = DecryptReader::new(reader, key, iv)?;
		
		let header = Header::read_from(&mut decrypter)?;
		let contents_start = decrypter.stream_position()?;
		
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
	
	pub fn read_file<'s>(&'s mut self, source: &str, path: &EntryPath) -> Option<io::Result<impl Read + use<'s, R>>> {
		let entry = self.header.entries()
			.get(source)?
			.iter()
			.find(|entry| &entry.path == path)?;
		
		if let Err(err) = self.decrypter.seek(SeekFrom::Start(self.contents_start + entry.position)) {
			return Some(Err(err));
		}
		
		Some(Ok((&mut self.decrypter).take(entry.size)))
	}
	
	pub fn for_each_file<F>(&mut self, mut callback: F) -> Result<(), io::Error>
	where
		F: FnMut(&str, &EntryPath, u64, io::Take<&mut DecryptReader<R>>) -> io::Result<()>,
	{
		self.decrypter.seek(SeekFrom::Start(self.contents_start))?;
		
		for (source, entries) in self.header.entries() {
			for entry in entries {
				let reader = (&mut self.decrypter).take(entry.size);
				callback(source, &entry.path, entry.size, reader)?;
			}
		}
		
		Ok(())
	}
}

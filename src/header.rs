use std::{collections::BTreeMap, io::{self, Read, Seek, Write}};

use blake3::Hash;

use crate::{index::{Entries, EntryPath, Sources}, Source};

#[derive(Clone, Copy, Debug)]
pub struct Flags {
	pub is_single_source: bool,
}

impl Flags {
	fn to_bytes(self) -> [u8; 4] {
		let mut flags = 0u32;
		
		if self.is_single_source {
			flags |= 1;
		}
		
		flags.to_le_bytes()
	}
	
	pub fn from_bytes(bytes: [u8; 4]) -> Self {
		let flags = u32::from_le_bytes(bytes);
		
		let is_single_source = flags & 1 != 0;
		
		Self {
			is_single_source,
		}
	}
}
// TODO use a better name
#[derive(Debug)]
struct Value {
	size: u64,
	hash: Hash,
}

#[derive(Debug)]
pub struct HeaderBuilder {
	flags: Flags,
	entries: BTreeMap<Source, BTreeMap<EntryPath, Option<Value>>>,
}

impl HeaderBuilder {
	pub fn new(sources: &Sources, flags: Flags) -> Self {
		let entries = sources.iter()
			.map(|(source, entries)| {
				let entry_map: BTreeMap<EntryPath, Option<Value>> = match entries {
					Entries::File(entry) => {
						let mut map = BTreeMap::new();
						map.insert(entry.path.clone(), None);
						map
					},
					Entries::Directory(entries) => entries.iter()
						.map(|entry| (entry.path.clone(), None))
						.collect(),
				};
				
				(source.clone(), entry_map)
			})
			.collect();
		
		Self {
			flags,
			entries,
		}
	}
	
	pub fn set_entry(&mut self, source: &Source, path: &EntryPath, size: u64, hash: Hash) {
		let entry = self.entries
			.get_mut(source).expect("source should have been included when calling Header::new")
			.get_mut(path).expect("entry should have been included when calling Header::new");
		assert!(entry.is_none(), "can't set entry twice");
		
		// TODO: save size instead of position?
		*entry = Some(Value {
			size,
			hash,
		});
	}
	
	pub fn header_size(&self) -> u64 {
		let mut size = 0u64;
		// flags
		size += self.flags.to_bytes().len() as u64;
		// source count
		size += size_of::<u32>() as u64;
		// sources
		#[allow(clippy::needless_as_bytes, reason = "be more explicit about the length being in bytes")]
		for (source, entries) in self.entries.iter() {
			// id length + id + entry count
			size += size_of::<u32>() as u64;
			size += source.id.as_bytes().len() as u64;
			size += size_of::<u32>() as u64;
			
			// entries
			for (entry_path, _) in entries.iter() {
				// hash + position + path length + path
				size += size_of::<Hash>() as u64;
				size += size_of::<u64>() as u64;
				size += size_of::<u32>() as u64;
				size += entry_path.as_bytes().len() as u64;
			}
		}
		size
	}
	
	// NOTE: if this is updated header_size might need to be updated as well
	pub fn write_header(&self, mut writer: impl Write + Seek) -> Result<(), io::Error> {
		let start_position = writer.stream_position()?;
		
		// write: flags
		writer.write_all(&self.flags.to_bytes())?;
		// write: source count
		let source_count: u32 = self.entries.len().try_into().expect("shouldn't contain that many sources");
		writer.write_all(&source_count.to_le_bytes())?;
		// sources
		for (source, entries) in self.entries.iter() {
			// no need to store source.is_file, should be unambiguous to find out from the archive
			// write: id length, id, entry count
			write_slice(&mut writer, source.id.as_bytes())?;
			let entry_count: u32 = entries.len().try_into().expect("shouldn't contain that many entries");
			writer.write_all(&entry_count.to_le_bytes())?;
			
			// entries
			// TODO: rename value?
			for (entry_path, value) in entries.iter() {
				let Some(value) = value else {
					panic!("No hash and position set for entry with path {} in source {}", entry_path, source.id);
				};
				// write: hash, size, path length, path
				writer.write_all(value.hash.as_bytes())?;
				writer.write_all(&value.size.to_le_bytes())?;
				write_slice(&mut writer, entry_path.as_bytes())?;
			}
		}
		
		let bytes_written = writer.stream_position()? - start_position;
		assert_eq!(bytes_written, self.header_size());
		
		Ok(())
	}
}

#[derive(Debug)]
pub struct Entry {
	pub path: EntryPath,
	pub hash: Hash,
	pub position: u64,
	pub size: u64,
}

#[derive(Debug)]
pub struct Header {
	flags: Flags,
	entries: BTreeMap<String, Vec<Entry>>,
}

impl Header {
	pub fn read_from(mut reader: impl Read) -> Result<Self, io::Error> {
		// read: flags
		let flags = Flags::from_bytes(read_bytes(&mut reader)?);
		
		// read: source count
		let source_count = read_u32(&mut reader)?;
		
		// sources
		// TODO: use SourceID newtype?
		let mut position = 0;
		let mut entries: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
		for _ in 0..source_count {
			// read: id length, id, entry count
			let id = String::from_utf8(read_slice(&mut reader)?).unwrap(); // TODO: return custom error
			let entry_count = read_u32(&mut reader)?;
			
			// entries
			let mut source_entries: Vec<Entry> = Vec::with_capacity(entry_count as usize);
			for _ in 0..entry_count {
				// read: hash, size, path_length, path
				let hash = Hash::from_bytes(read_bytes(&mut reader)?);
				let size = read_u64(&mut reader)?;
				let path = EntryPath::from_bytes(read_slice(&mut reader)?);
				
				source_entries.push(Entry {
					hash,
					path,
					size,
					position,
				});
				
				position += size;
			}
			
			let prev_entry = entries.insert(id, source_entries);
			assert!(prev_entry.is_none());
		}
		
		Ok(Self {
			flags,
			entries,
		})
	}
	
	pub fn flags(&self) -> Flags {
		self.flags
	}
	
	pub fn entries(&self) -> &BTreeMap<String, Vec<Entry>> {
		&self.entries
	}
}

fn write_slice(mut writer: impl Write, slice: &[u8]) -> Result<(), io::Error> {
	writer.write_all(&(slice.len() as u32).to_le_bytes())?;
	writer.write_all(slice)?;
	Ok(())
}

fn read_slice(mut reader: impl Read) -> Result<Vec<u8>, io::Error> {
	let len = read_u32(&mut reader)?;
	let mut buf = vec![0; len as usize];
	reader.read_exact(&mut buf)?;
	Ok(buf)
}

fn read_bytes<const LEN: usize>(mut reader: impl Read) -> Result<[u8; LEN], io::Error> {
	let mut bytes = [0; LEN];
	reader.read_exact(&mut bytes)?;
	Ok(bytes)
}

fn read_u32(reader: impl Read) -> Result<u32, io::Error> {
	Ok(u32::from_le_bytes(read_bytes(reader)?))
}

fn read_u64(reader: impl Read) -> Result<u64, io::Error> {
	Ok(u64::from_le_bytes(read_bytes(reader)?))
}

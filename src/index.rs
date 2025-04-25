// TODO: should this be a submodule of pack?

use std::{collections::BTreeMap, io, path::PathBuf};

use either::Either;
use walkdir::WalkDir;

use crate::Source;

mod entry_path;
pub use entry_path::EntryPath;

#[derive(Debug)]
pub struct Index {
	total_size: u64,
	entries: Contents,
}

#[derive(Debug)]
pub enum Contents {
	Simple(Sources),
	Grouped(Vec<Group>),
}

#[derive(Debug)]
pub struct Group {
	pub size: u64,
	pub sources: Sources,
}

pub type Sources = BTreeMap<Source, Entries>;

#[derive(Debug)]
pub enum Entries {
	Directory(Vec<Entry>),
	File(Entry),
}

impl<'a> IntoIterator for &'a Entries {
	type Item = &'a Entry;
	type IntoIter = Either<std::slice::Iter<'a, Entry>, std::iter::Once<&'a Entry>>;
	
	fn into_iter(self) -> Self::IntoIter {
		match self {
			Entries::Directory(entries) => Either::Left(entries.iter()),
			Entries::File(entry) => Either::Right(std::iter::once(entry)),
		}
	}
}

#[derive(Clone, Debug)]
pub struct Entry {
	pub path: EntryPath,
	pub size: u64,
}

#[derive(Debug)]
struct SourceEntry {
	source: Source,
	path: EntryPath,
	size: u64,
}

impl Index {
	pub fn from_sources(sources: Vec<Source>, max_group_size: Option<u64>) -> Result<Self, io::Error> {
		let (entries, total_size) = index_files(sources)?;
		
		let entries = match max_group_size {
			Some(max_size) => Contents::Grouped(group_files(entries, max_size)),
			None => Contents::Simple(group_by_source(entries)),
		};
		
		Ok(Self {
			total_size,
			entries,
		})
	}
	
	pub fn entries(&self) -> &Contents {
		&self.entries
	}
	
	pub fn total_size(&self) -> u64 {
		self.total_size
	}
}

fn index_files(sources: Vec<Source>) -> Result<(Vec<SourceEntry>, u64), io::Error> {
	let format = humansize::make_format(humansize::BINARY);
	let mut index = Vec::new();
	
	let mut total_size = 0;
	let mut prev_index_len = 0;
	
	for source in sources {
		if source.is_file {
			index.push(SourceEntry {
				path: EntryPath::empty(),
				size: source.path.metadata()?.len(),
				source,
			});
			continue;
		}
		
		println!("Indexing files in {}...", source.path.to_string_lossy());
		
		let mut source_size = 0;
		
		for entry in WalkDir::new(&source.path).follow_links(true) {
			let entry = entry?;
			
			if !entry.file_type().is_file() {
				continue;
			}
			
			let size = entry.metadata()?.len();
			source_size += size;
			
			index.push(SourceEntry {
				source: source.clone(),
				path: EntryPath::new(&source, entry.path()),
				size: entry.metadata()?.len(),
			});
		}
		
		total_size += source_size;
		let files_count = index.len() - prev_index_len;
		prev_index_len = index.len();
		println!("Found {files_count} files with a total size of {}.", format(source_size));
	}
	
	Ok((index, total_size))
}

struct UnsortedGroup {
	size: u64,
	entries: Vec<SourceEntry>,
}

fn group_files(mut entries: Vec<SourceEntry>, max_group_size: u64) -> Vec<Group> {
	entries.sort_by(|left, right| right.size.cmp(&left.size));
	let mut groups: Vec<UnsortedGroup> = Vec::new();
	
	for entry in entries {
		let Some(group_position) = groups.iter()
			.position(|group| group.size + entry.size <= max_group_size)
		else {
			groups.push(UnsortedGroup {
				size: entry.size,
				entries: vec![entry],
			});
			continue;
		};
		
		let group = &mut groups[group_position];
		group.size += entry.size;
		group.entries.push(entry);
		
		// sort groups
		
		if group_position == 0 || groups[group_position - 1].size >= groups[group_position].size {
			continue;
		}
		
		let group = groups.remove(group_position);
		
		let insert_position = match groups.binary_search_by(|probe| group.size.cmp(&probe.size)) {
			Ok(found_position) => found_position + 1,
			Err(insert_position) => insert_position,
		};
		
		groups.insert(insert_position, group);
	}
	
	groups.into_iter().map(Into::into).collect()
}

fn group_by_source(entries: Vec<SourceEntry>) -> Sources {
	let mut sources = Sources::new();
	
	for source_entry in entries {
		let source = source_entry.source;
		let entry = Entry {
			path: source_entry.path,
			size: source_entry.size,
		};
		
		if source.is_file {
			let prev_value = sources.insert(source, Entries::File(entry));
			assert!(prev_value.is_none(), "Sources should be unique and file sources should only have one entry");
			continue;
		}
		
		if let Some(source_entries) = sources.get_mut(&source) {
			let Entries::Directory(source_entries) = source_entries else {
				panic!("Sources should be unique and can't be a file and a directory at the same time");
			};
			
			source_entries.push(entry);
		} else {
			sources.insert(source, Entries::Directory(vec![entry]));
		}
	}
	
	sources
}

impl From<UnsortedGroup> for Group {
	fn from(unsorted: UnsortedGroup) -> Self {
		Self {
			size: unsorted.size,
			sources: group_by_source(unsorted.entries),
		}
	}
}

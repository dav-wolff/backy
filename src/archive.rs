use std::{
	fs::{self, File},
	io::{self, Read},
	path::{Path, PathBuf},
};

use anyhow::{
	bail,
	ensure,
	Context,
};
use rayon::{
	iter::{IntoParallelRefMutIterator, ParallelIterator},
};

use crate::{
	FileHash,
	HeaderHash,
	MultiHeaderHash,
	OriginHash,
	crypto::Key,
	index::EntryPath,
	progress::{NoopProgressDisplay, ProgressDisplay, TerminalProgressDisplay},
};

mod sub_archive;
use sub_archive::SubArchive;
pub use sub_archive::ArchiveReader;

pub struct Archive {
	sub_archive_data: Vec<SubArchiveData>,
	is_multi_archive: bool,
}

struct SubArchiveData {
	name: String,
	size: u64,
	sub_archive: SubArchive<File>,
	// TODO: work on terminology. is multi archive a good name to use everywhere?
}

impl Archive {
	pub fn new(path: PathBuf, key: Key) -> anyhow::Result<Self> {
		if !path.exists() {
			bail!("archive doesn't exist");
		}
		
		let is_multi_archive = path.is_dir();
		
		let sub_archives: Vec<SubArchiveData> = if is_multi_archive {
			fs::read_dir(&path).with_context(|| format!("reading archive directory at {:?}", path))?
				.map(|dir_entry| -> anyhow::Result<_> {
					let dir_entry = dir_entry.with_context(|| format!("iterating entries in {:?}", path))?;
					let entry_path = dir_entry.path();
					ensure!(entry_path.is_file(), "subarchive at {:?} is not a file", entry_path);
					let metadata = dir_entry.metadata().with_context(|| format!("querying metadata for {:?}", entry_path))?;
					let size = metadata.len();
					let file = File::open(&entry_path).with_context(|| format!("opening subarchive file at {:?}", entry_path))?;
					let sub_archive = SubArchive::new(file, key).with_context(|| format!("parsing subarchive at {:?}", entry_path))?;
					
					Ok(SubArchiveData {
						sub_archive,
						size,
						name: entry_path.to_string_lossy().into_owned(),
					})
				})
				.collect::<Result<_, _>>()?
		} else {
			let metadata = path.metadata().with_context(|| format!("querying metadata for {:?}", path))?;
			let size = metadata.len();
			let file = File::open(&path).with_context(|| format!("opening archive file at {:?}", path))?;
			let sub_archive = SubArchive::new(file, key).with_context(|| format!("parsing archive at {:?}", path))?;
			vec![SubArchiveData {
				sub_archive,
				size,
				name: path.to_string_lossy().into_owned(),
			}]
		};
		
		Ok(Self {
			sub_archive_data: sub_archives,
			is_multi_archive,
		})
	}
	
	fn total_size(&self) -> u64 {
		self.sub_archive_data.iter()
			.map(|data| data.size)
			.sum()
	}
	
	pub fn unpack(&mut self, out_dir: impl AsRef<Path>, display_progress: bool) -> anyhow::Result<()> {
		let out_dir = out_dir.as_ref();
		
		let progress_display: &dyn ProgressDisplay = if display_progress {
			&TerminalProgressDisplay::new(self.total_size())
		} else {
			&NoopProgressDisplay
		};
		
		self.sub_archive_data.par_iter_mut()
			// TODO: fail early
			.try_for_each(|SubArchiveData { name, size, sub_archive }| -> anyhow::Result<()> {
				let progress_tracker = progress_display.new_tracker(name.clone().into(), *size - sub_archive.contents_start());
				let is_single_source = sub_archive.is_single_source();
				
				sub_archive.for_each_file(|source, file_info, mut reader| -> anyhow::Result<()> {
					let path = &file_info.path;
					let dest = if path.as_str().is_empty() { // source is a file
						out_dir.join(source)
					} else if is_single_source {
						out_dir.join(path.as_path())
					} else {
						out_dir.join(source).join(path.as_path())
					};
					
					let parent = dest.parent().expect("must have a parent directory");
					fs::create_dir_all(parent).with_context(|| format!("creating out directory in {:?}", parent))?;
					let mut out = File::create(&dest).with_context(|| format!("creating file at {:?}", dest))?;
					io::copy(&mut reader, &mut out).with_context(|| format!("unpacking file to {:?}", dest))?;
					ensure!(!reader.is_corrupted(), "file {:?} in source {source} is corrupted", path.as_path());
					
					progress_tracker.advance(file_info.size);
					
					Ok(())
				})?;
				
				Ok(())
			})?;
		
		Ok(())
	}
	
	pub fn check_integrity(&mut self, display_progress: bool) -> anyhow::Result<bool> {
		enum CheckIntegrityError {
			Integrity,
			Other(anyhow::Error),
		}
		
		impl From<anyhow::Error> for CheckIntegrityError {
			fn from(value: anyhow::Error) -> Self {
				Self::Other(value)
			}
		}
		
		let progress_display: &dyn ProgressDisplay = if display_progress {
			&TerminalProgressDisplay::new(self.total_size())
		} else {
			&NoopProgressDisplay
		};
		
		let result = self.sub_archive_data.par_iter_mut()
			.try_for_each(|SubArchiveData { name, size, sub_archive }| {
				let progress_tracker = progress_display.new_tracker(name.clone().into(), *size - sub_archive.contents_start());
				
				sub_archive.for_each_file(|source, file_info, reader| -> Result<(), CheckIntegrityError> {
					let reader = reader.into_inner();
					let mut hasher = blake3::Hasher::new();
					hasher.update_reader(reader)
						.with_context(|| format!("hashing file {:?} from source {source:?}", file_info.path))
						.with_context(|| format!("reading subarchive {name:?}"))?;
					
					progress_tracker.advance(file_info.size);
					
					if FileHash(hasher.finalize()) != file_info.hash {
						Err(CheckIntegrityError::Integrity)
					} else {
						Ok(())
					}
				})
			});
		
		match result {
			Ok(()) => Ok(true),
			Err(CheckIntegrityError::Integrity) => Ok(false),
			Err(CheckIntegrityError::Other(err)) => Err(err),
		}
	}
	
	fn sub_archives(&self) -> impl Iterator<Item = &SubArchive<File>> {
		self.sub_archive_data.iter()
			.map(|data| &data.sub_archive)
	}
	
	// FIX: outputs duplicate sources
	pub fn sources(&self) -> impl Iterator<Item = &str> {
		self.sub_archives()
			.flat_map(|sub_archive| sub_archive.sources())
	}
	
	// TODO: better way to output file sources?
	pub fn file_paths(&self) -> impl Iterator<Item = &str> {
		self.sub_archives()
			.flat_map(|sub_archive| sub_archive.file_paths())
	}
	
	// TODO: should this be public?
	pub(crate) fn file_hashes(&self) -> impl Iterator<Item = (OriginHash, impl Iterator<Item = FileHash>)> {
		let multi_header_hash = self.is_multi_archive.then(|| {
			let header_hashes: Vec<_> = self.sub_archives()
				.map(|sub_archive| sub_archive.hash())
				.collect();
			
			MultiHeaderHash::from_hashes(header_hashes)
		});
		
		let origin_hash = move |header_hash: HeaderHash| match multi_header_hash {
			None => OriginHash::Single(header_hash),
			Some(multi_header_hash) => OriginHash::Multi(multi_header_hash, header_hash),
		};
		
		self.sub_archives()
			.map(move |sub_archive| (origin_hash(sub_archive.hash()), sub_archive.file_hashes()))
	}
	
	pub fn get_file(&mut self, source: Option<&str>, path: &str) -> anyhow::Result<Option<ArchiveReader<impl Read>>> {
		// TODO: is this necessary?
		let entry_path = EntryPath::from_bytes(path.as_bytes().to_owned()).unwrap();
		
		for SubArchiveData { sub_archive, .. } in &mut self.sub_archive_data {
			// TODO: don't unwrap source
			if let Some(reader) = sub_archive.read_file(source.unwrap(), &entry_path)? {
				return Ok(Some(reader));
			}
		}
		
		Ok(None)
	}
}

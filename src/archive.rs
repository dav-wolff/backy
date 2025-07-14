use std::{fs::{self, File}, io::{self, Read}, path::PathBuf};

use anyhow::{bail, ensure, Context};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};

use crate::{crypto::Key, index::EntryPath, progress::ProgressDisplay};

mod sub_archive;
use sub_archive::SubArchive;

pub struct Archive {
	sub_archives: Vec<SubArchiveData>,
}

struct SubArchiveData {
	name: String,
	size: u64,
	sub_archive: SubArchive<File>,
}

impl Archive {
	pub fn new(path: PathBuf, key: Key) -> anyhow::Result<Self> {
		if !path.exists() {
			bail!("archive doesn't exist");
		}
		
		let sub_archives: Vec<SubArchiveData> = if path.is_dir() {
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
			sub_archives,
		})
	}
	
	pub fn unpack(&mut self, out_dir: PathBuf) -> anyhow::Result<()> {
		let total_size = self.sub_archives.iter()
			.map(|data| data.size)
			.sum();
		let progress_display = ProgressDisplay::new(total_size);
		
		self.sub_archives.par_iter_mut()
			// TODO: fail early
			.try_for_each(|SubArchiveData { name, size, sub_archive }| -> anyhow::Result<()> {
				let progress_tracker = progress_display.new_tracker(name.clone(), *size - sub_archive.contents_start());
				let is_single_source = sub_archive.is_single_source();
				
				sub_archive.for_each_file(|source, path, size, mut reader| {
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
					
					progress_tracker.advance(size);
					
					Ok(())
				})?;
				
				Ok(())
			})?;
		
		Ok(())
	}
	
	// FIX: outputs duplicate sources
	pub fn sources(&self) -> impl Iterator<Item = &str> {
		self.sub_archives.iter()
			.map(|data| &data.sub_archive)
			.flat_map(|sub_archive| sub_archive.sources())
	}
	
	// TODO: better way to output file sources?
	pub fn file_paths(&self) -> impl Iterator<Item = &str> {
		self.sub_archives.iter()
			.map(|data| &data.sub_archive)
			.flat_map(|sub_archive| sub_archive.file_paths())
	}
	
	pub fn get_file(&mut self, source: Option<&str>, path: &str) -> anyhow::Result<Option<impl Read>> {
		// TODO: is this necessary?
		let entry_path = EntryPath::from_bytes(path.as_bytes().to_owned()).unwrap();
		
		for SubArchiveData { sub_archive, .. } in &mut self.sub_archives {
			// TODO: don't unwrap source
			if let Some(reader) = sub_archive.read_file(source.unwrap(), &entry_path)? {
				return Ok(Some(reader));
			}
		}
		
		Ok(None)
	}
}

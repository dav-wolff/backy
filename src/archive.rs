use std::{fs::{self, File}, io::{self, Read}, path::PathBuf};

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
	pub fn new(path: PathBuf, key: Key) -> io::Result<Self> {
		// TODO: return custom errors
		if !path.exists() {
			panic!("Archive doesn't exist");
		}
		
		let sub_archives: Vec<SubArchiveData> = if path.is_dir() {
			fs::read_dir(&path)?
				.map(|dir_entry| dir_entry.and_then(|dir_entry| {
					let entry_path = dir_entry.path();
					// TODO: return custom error
					assert!(entry_path.is_file());
					let size = dir_entry.metadata()?.len();
					let file = File::open(&entry_path)?;
					let sub_archive = SubArchive::new(file, key)?;
					
					Ok(SubArchiveData {
						sub_archive,
						size,
						name: entry_path.to_string_lossy().into_owned(),
					})
				}))
				.collect::<Result<_, _>>()?
		} else {
			let size = path.metadata()?.len();
			let file = File::open(&path)?;
			let sub_archive = SubArchive::new(file, key)?;
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
	
	pub fn unpack(&mut self, out_dir: PathBuf) -> io::Result<()> {
		let total_size = self.sub_archives.iter()
			.map(|data| data.size)
			.sum();
		let progress_display = ProgressDisplay::new(total_size);
		
		self.sub_archives.par_iter_mut()
			.map(|SubArchiveData { name, size, sub_archive }| -> io::Result<()> {
				let progress_tracker = progress_display.new_tracker(name.clone(), *size - sub_archive.contents_start());
				let is_single_source = sub_archive.is_single_source();
				
				sub_archive.for_each_file(|source, path, size, mut reader| {
					let dest = if is_single_source {
						out_dir.join(path.as_path())
					} else {
						out_dir.join(source).join(path.as_path())
					};
					
					fs::create_dir_all(dest.parent().expect("must have a parent directory"))?;
					let mut out = File::create(dest)?;
					io::copy(&mut reader, &mut out)?;
					
					progress_tracker.advance(size);
					
					Ok(())
				})?;
				
				Ok(())
			})
			.collect::<io::Result<()>>()?;
		
		Ok(())
	}
	
	pub fn sources(&self) -> impl Iterator<Item = &str> {
		self.sub_archives.iter()
			.map(|data| &data.sub_archive)
			.flat_map(|sub_archive| sub_archive.sources())
	}
	
	pub fn file_paths(&self) -> impl Iterator<Item = &str> {
		self.sub_archives.iter()
			.map(|data| &data.sub_archive)
			.flat_map(|sub_archive| sub_archive.file_paths())
	}
	
	pub fn get_file(&mut self, source: Option<&str>, path: &str) -> io::Result<Option<impl Read>> {
		// TODO: is this necessary?
		let entry_path = EntryPath::from_bytes(path.as_bytes().to_owned());
		
		for SubArchiveData { sub_archive, .. } in &mut self.sub_archives {
			if let Some(reader) = sub_archive.read_file(source.unwrap(), &entry_path).transpose()? { // TODO transpose shouldn't be necessary
				return Ok(Some(reader));
			}
		}
		
		Ok(None)
	}
}

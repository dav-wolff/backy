use std::{borrow::Cow, fs::{self, DirEntry, File}, io::{self, Read, Seek, Write}, ops::ControlFlow, path::{Path, PathBuf}};

use either::Either;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{crypto::Key, progress::{ProgressDisplay, ProgressTracker}};

mod sub_archive;
use sub_archive::SubArchive;

pub struct Archive {
	path: PathBuf,
	key: Key,
}

impl Archive {
	pub fn new(path: PathBuf, key: Key) -> Self {
		// TODO: return custom errors
		if !path.exists() {
			panic!("Archive doesn't exist");
		}
		
		Self {
			path,
			key,
		}
	}
	
	pub fn unpack(&self, out: PathBuf) -> Result<(), io::Error> {
		if self.path.is_dir() {
			let entries: Vec<DirEntry> = fs::read_dir(&self.path)?.collect::<Result<_, _>>()?;
			let total_size: u64 = entries.iter()
				.map(|entry| -> Result<_, io::Error> {
					Ok(entry.metadata()?.len())
				})
				.sum::<Result<_, _>>()?;
			
			let progress_display = ProgressDisplay::new(total_size);
			
			entries.into_par_iter()
				.map(|entry| -> Result<_, io::Error> {
					let progress_tracker = progress_display.new_tracker(entry.path().to_string_lossy().into_owned(), entry.metadata()?.len());
					self.unpack_group(&entry.path(), &out, progress_tracker)?;
					
					Ok(())
				})
				.collect::<Result<(), _>>()?;
		} else {
			let total_size = self.path.metadata()?.len();
			let progress_display = ProgressDisplay::new(total_size);
			let progress_tracker = progress_display.new_tracker("Total", total_size);
			self.unpack_group(&self.path, &out, progress_tracker)?;
		}
		
		Ok(())
	}
	
	fn unpack_group(&self, group: &Path, out: &Path, progress_tracker: ProgressTracker) -> Result<(), io::Error> {
		let file = File::open(group)?;
		let sub_archive = SubArchive::new(&file, self.key)?;
		
		progress_tracker.advance((&file).stream_position()?);
		
		let is_single_source = sub_archive.is_single_source();
		sub_archive.for_each_tar(|source_group, tar| {
			let is_file = source_group.flags & 1 != 0;
			
			let directory = if is_single_source || is_file {
				Cow::Borrowed(out)
			} else {
				Cow::Owned(out.join(&source_group.id))
			};
			
			fs::create_dir_all(&directory)?;
			tar.unpack(&directory)?;
			
			progress_tracker.advance(source_group.size);
			
			Ok(ControlFlow::Continue(()))
		})?;
		
		Ok(())
	}
	
	pub fn sources(&self) -> Result<Vec<String>, io::Error> {
		if self.path.is_dir() {
			let mut sources: Vec<String> = Vec::new();
			
			for entry in fs::read_dir(&self.path)? {
				let file = File::open(entry?.path())?;
				for source in SubArchive::new(file, self.key)?.sources() {
					if !sources.iter().any(|s| s == source) {
						sources.push(source.to_owned());
					}
				}
			}
			
			Ok(sources)
		} else {
			let file = File::open(&self.path)?;
			Ok(SubArchive::new(file, self.key)?
				.sources()
				.map(ToOwned::to_owned)
				.collect())
		}
	}
	
	pub fn for_each_file(&self, mut callback: impl FnMut(&str, &Path)) -> Result<(), io::Error> {
		for sub_archive in self.sub_archives()? {
			let sub_archive = sub_archive?;
			
			sub_archive.for_each_tar(|source_group, tar| {
				let iter = tar.entries()?
					.map(|entry| entry.map(|entry| {
						entry.path().unwrap().into_owned()
					}));
				
				for entry in iter {
					callback(&source_group.id, &entry?);
				}
				
				Ok(ControlFlow::Continue(()))
			})?;
		}
		
		Ok(())
	}
	
	pub fn get_file(&self, source: Option<&str>, path: &str, mut writer: impl Write) -> Result<(), io::Error> {
		for sub_archive in self.sub_archives()? {
			let sub_archive = sub_archive?;
			
			sub_archive.for_each_tar(|source_group, tar| {
				if source.is_some_and(|source| source_group.id == source) {
					return Ok(ControlFlow::Continue(()));
				}
				
				if let Some(entry) =
					tar.entries()?
						.find(|entry| {
							entry.as_ref().unwrap().path().unwrap().to_str().unwrap() == path
						})
				{
					let mut entry = entry?;
					io::copy(&mut entry, &mut writer)?;
					return Ok(ControlFlow::Break(()));
				}
				
				Ok(ControlFlow::Continue(()))
			})?;
		}
		
		Ok(())
	}
	
	fn sub_archives(&self) -> Result<impl Iterator<Item = Result<SubArchive<impl Read>, io::Error>>, io::Error> {
		let iter = if self.path.is_dir() {
			Either::Left(
				fs::read_dir(&self.path)?
					.map(|entry| {
						let path = entry?.path();
						let file = File::open(&path)?;
						SubArchive::new(file, self.key)
					})
			)
		} else {
			let file = File::open(&self.path)?;
			let sub_archive = SubArchive::new(file, self.key);
			Either::Right(std::iter::once(sub_archive))
		};
		
		Ok(iter)
	}
}

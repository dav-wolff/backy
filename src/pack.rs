use std::{fs::{self, File}, io::{self, Seek, Write}, mem, path::{Path, PathBuf}};

use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use xz2::write::XzEncoder;

use crate::{group::create_groups, index::create_index, progress::{ProgressDisplay, ProgressTracker}, Entry, Source, BKY_HEADER};

pub fn pack(sources: Vec<PathBuf>, out: PathBuf, max_group_size: Option<u64>, compression_level: u32) -> Result<(), io::Error> {
	if compression_level > 9 {
		panic!("compression_level must be a number between 0 and 9");
	}
	
	// TODO: get rid of unwraps
	// TODO: create separate ids for folders with same name
	let sources = sources.into_iter()
		.map(|path| path.canonicalize().unwrap())
		.map(|path| Source {
			id: path.file_name().unwrap().to_string_lossy().into(),
			path: path.into(),
		})
		.collect();
	
	let (index, total_size) = create_index(sources)?;
	
	if let Some(max_group_size) = max_group_size {
		if !out.exists() {
			fs::create_dir(&out)?;
		}
		
		let groups = create_groups(index, max_group_size);
		let progress_display = ProgressDisplay::new(total_size);
		
		groups.into_par_iter()
			.enumerate()
			.map(|(i, group)| -> Result<_, io::Error> {
				let i = i + 1;
				let path = out.join(format!("{i}.bky"));
				pack_group(&path, group.entries, compression_level, progress_display.new_tracker(path.to_string_lossy().into_owned(), group.size))?;
				
				Ok(())
			})
			.collect::<Result<(), _>>()?;
	} else {
		let progress_display = ProgressDisplay::new(total_size);
		pack_group(&out, index, compression_level, progress_display.new_tracker("Total", total_size))?;
	}
	
	Ok(())
}

fn pack_group(out: &Path, entries: Vec<Entry>, compression_level: u32, progress_tracker: ProgressTracker) -> Result<(), io::Error> {
	let mut file = File::create_new(out)?;
	
	file.write_all(BKY_HEADER)?;
	
	let mut source_groups: Vec<(Source, Vec<Entry>, u64)> = Vec::new();
	
	for entry in entries {
		match source_groups.iter_mut().find(|(source, _, _)| source.id == entry.source.id) {
			Some((_, source_entries, _)) => {
				source_entries.push(entry);
			},
			None => {
				source_groups.push((entry.source.clone(), vec![entry], 0));
			},
		}
	}
	
	// header
	let groups_len: u32 = source_groups.len() as u32;
	file.write_all(&groups_len.to_le_bytes())?;
	
	for (source, _, _) in &source_groups {
		let id_len: u32 = source.id.len() as u32;
		file.write_all(&id_len.to_le_bytes())?;
		file.write_all(source.id.as_bytes())?;
		// placeholder for archive length
		file.write_all(&0u64.to_le_bytes())?;
	}
	
	
	// tar archives
	let mut encoder = XzEncoder::new(&file, compression_level);
	let mut prev_position = 0;
	for (source, entries, source_size) in &mut source_groups {
		let mut tar_builder = tar::Builder::new(encoder);
		
		for entry in entries.iter() {
			tar_builder.append_file(
				entry.path.strip_prefix(&source.path).expect("all entries should be located below the source path"),
				&mut File::open(&entry.path)?
			)?;
			
			progress_tracker.advance(entry.size);
		}
		
		encoder = tar_builder.into_inner()?;
		
		*source_size = encoder.total_in() - prev_position;
		prev_position = encoder.total_in();
	}
	
	mem::drop(encoder);
	
	file.seek(io::SeekFrom::Start((BKY_HEADER.len() + mem::size_of::<u32>()) as u64))?;
	
	// fill header placeholders
	for (source, _, source_size) in &source_groups {
		file.seek(io::SeekFrom::Current((mem::size_of::<u32>() + source.id.len()) as i64))?;
		file.write_all(&source_size.to_le_bytes())?;
	}
	
	Ok(())
}

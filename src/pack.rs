use std::{fs::{self, File}, io::{self, Seek, Write}, mem, path::PathBuf};

use crate::{group::create_groups, index::create_index, Entry, Source, BKY_HEADER};

pub fn pack(sources: Vec<PathBuf>, out: PathBuf, max_group_size: Option<u64>) -> Result<(), io::Error> {
	let format = humansize::make_format(humansize::BINARY);
	
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
		
		for (i, group) in groups.into_iter().enumerate() {
			let i = i + 1;
			let path = out.join(format!("{i}.bky"));
			println!("Writing group {i} of size {} to {}...", format(group.size), path.to_string_lossy());
			pack_group(path, group.entries)?;
		}
	} else {
		println!("Writing backup of size {} to {}...", format(total_size), out.to_string_lossy());
		pack_group(out, index)?;
	}
	
	Ok(())
}

fn pack_group(out: PathBuf, entries: Vec<Entry>) -> Result<(), io::Error> {
	let mut file = File::create_new(out)?;
	
	file.write_all(BKY_HEADER)?;
	
	let mut source_groups: Vec<(Source, Vec<PathBuf>, u64)> = Vec::new();
	
	for entry in entries {
		let (_, source_entries, _) = match source_groups.iter_mut()
			.find(|(source, _, _)| source.id == entry.source.id)
		{
			Some(source_group) => source_group,
			None => {
				source_groups.push((entry.source, Vec::new(), 0));
				source_groups.last_mut().expect("last should exist as it was just inserted")
			},
		};
		
		source_entries.push(entry.path)
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
	let mut prev_position = file.stream_position()?;
	for (source, entries, source_size) in &mut source_groups {
		let mut tar_builder = tar::Builder::new(file);
		
		for entry in entries {
			tar_builder.append_file(
				entry.strip_prefix(&source.path).expect("all entries should be located below the source path"),
				&mut File::open(&entry)?
			)?;
		}
		
		file = tar_builder.into_inner()?;
		
		let new_position = file.stream_position()?;
		*source_size = new_position - prev_position;
		prev_position = new_position;
	}
	
	file.seek(io::SeekFrom::Start((BKY_HEADER.len() + mem::size_of::<u32>()) as u64))?;
	
	// fill header placeholders
	for (source, _, source_size) in &source_groups {
		file.seek(io::SeekFrom::Current((mem::size_of::<u32>() + source.id.len()) as i64))?;
		file.write_all(&source_size.to_le_bytes())?;
	}
	
	Ok(())
}
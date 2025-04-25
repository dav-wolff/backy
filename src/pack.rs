use std::{fs::{self, File}, io::{self, Seek, Write}, mem, path::{Path, PathBuf}};

use hashing_reader::HashingReader;
use header::Header;
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use xz2::write::XzEncoder;

use crate::{crypto::{generate_iv, EncryptWriter, Key, IV}, index::{Contents, Index, Sources}, progress::{ProgressDisplay, ProgressTracker}, Source, BKY_HEADER};

mod header;
mod hashing_reader;

pub fn pack(sources: Vec<PathBuf>, out: PathBuf, key: Key, max_group_size: Option<u64>, compression_level: u32) -> Result<(), io::Error> {
	if sources.is_empty() {
		panic!("at least one source must be provided");
	}
	
	if compression_level > 9 {
		panic!("compression_level must be a number between 0 and 9");
	}
	
	// TODO: get rid of unwraps
	// TODO: create separate ids for folders with same name
	let sources: Vec<_> = sources.into_iter()
		.map(|path| path.canonicalize().unwrap())
		.map(|path| Source {
			id: path.file_name().unwrap().to_string_lossy().into(),
			is_file: path.is_file(),
			path: path.into(),
		})
		.collect();
	
	let is_single_source = sources.len() == 1;
	
	let index = Index::from_sources(sources, max_group_size)?;
	
	let progress_display = ProgressDisplay::new(index.total_size());
	
	match index.entries() {
		Contents::Grouped(groups) => {
			if !out.exists() {
				fs::create_dir(&out)?;
			}
			
			groups.into_par_iter()
				.enumerate()
				.map(|(i, group)| -> Result<_, io::Error> {
					let i = i + 1;
					let path = out.join(format!("{i}.bky"));
					pack_group(
						&path,
						&group.sources,
						key,
						compression_level,
						is_single_source,
						progress_display.new_tracker(path.to_string_lossy().into_owned(), group.size)
					)?;
					
					Ok(())
				})
				.collect::<Result<(), _>>()?;
		},
		Contents::Simple(entries) => {
			pack_group(
				&out,
				entries,
				key,
				compression_level,
				is_single_source,
				progress_display.new_tracker("Total", index.total_size())
			)?;
		},
	}
	
	Ok(())
}

fn pack_group(
	out: &Path,
	sources: &Sources,
	key: Key,
	compression_level: u32,
	is_single_source: bool,
	progress_tracker: ProgressTracker
) -> Result<(), io::Error> {
	let mut file = File::create_new(out)?;
	
	file.write_all(BKY_HEADER)?;
	
	let iv = generate_iv();
	file.write_all(&iv)?;
	let mut encrypter = EncryptWriter::new(&mut file, key, iv);
	
	let header = Header::new(sources);
	
	// skip header
	let header_size = header.header_size();
	// let header_size = size_of::<u32>() * 2 + source_groups.iter() //                          source_groups_len(4) + flags(4)
	// 	.map(|(source, _, _)| size_of::<u32>() * 2 + size_of::<u64>() + source.id.len()) // + sum(id_len(4) + flags(4) + source_len(8) + id)
	// 	.sum::<usize>();
	
	// TODO: use random data
	let skip_buffer = vec![0; header_size.try_into().expect("header size too large")];
	encrypter.write_all(&skip_buffer)?;
	
	// write files
	for (source, entries) in sources {
		for entry in entries {
			let file = File::open(entry.path)?;
			let hashing_reader = HashingReader::new(file);
			let size = io::copy(&mut file, &mut encrypter)?;
			header.set_entry(source, &entry.path, size, hashing_reader.finalize());
			
			if size != entry.size {
				let format = humansize::make_format(humansize::BINARY);
				eprintln!(
					"Warning: Size differs between indexing and archiving for {}.\nOriginal size: {}\nCurrent size: {}",
					entry.path.to_string_lossy(),
					format(entry.size),
					format(size)
				);
			}
		}
	}
	
	// tar archives
	let mut encoder = XzEncoder::new(encrypter, compression_level);
	let mut prev_position = 0;
	for (source, entries, source_size) in &mut source_groups {
		let prefix = if source.is_file {
			source.path.parent().expect("absolute path to a file should have a parent")
		} else {
			&*source.path
		};
		
		let mut tar_builder = tar::Builder::new(encoder);
		
		for entry in entries.iter() {
			tar_builder.append_file(
				entry.path.strip_prefix(prefix).expect("all entries should be located below the source path"),
				&mut File::open(&entry.path)?
			)?;
			
			progress_tracker.advance(entry.size);
		}
		
		encoder = tar_builder.into_inner()?;
		
		*source_size = encoder.total_in() - prev_position;
		prev_position = encoder.total_in();
	}
	
	mem::drop(encoder);
	
	// reset file
	file.seek(io::SeekFrom::Start((BKY_HEADER.len() + size_of::<IV>()) as u64))?;
	// reset encrypter
	let mut encrypter = EncryptWriter::new(&mut file, key, iv);
	
	let mut flags = 0u32;
	
	if is_single_source {
		flags |= 1;
	}
	
	encrypter.write_all(&flags.to_le_bytes())?;
	
	// groups header
	let groups_len: u32 = source_groups.len() as u32;
	encrypter.write_all(&groups_len.to_le_bytes())?;
	
	for (source, _, source_size) in &source_groups {
		let id_len: u32 = source.id.len() as u32;
		encrypter.write_all(&id_len.to_le_bytes())?;
		encrypter.write_all(source.id.as_bytes())?;
		encrypter.write_all(&source_size.to_le_bytes())?;
		
		let mut flags = 0u32;
		
		if source.is_file {
			flags |= 1;
		}
		
		encrypter.write_all(&flags.to_le_bytes())?;
	}
	
	Ok(())
}

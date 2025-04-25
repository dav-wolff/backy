use std::{collections::BTreeMap, fs::{self, File}, io::{self, Seek, Write}, path::{Path, PathBuf}};

use hashing_reader::HashingReader;
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

use crate::{crypto::{generate_iv, EncryptWriter, Key, IV}, header::{Flags, HeaderBuilder}, index::{Contents, Entries, Entry, EntryPath, Index, Sources}, progress::{ProgressDisplay, ProgressTracker}, Source, BKY_HEADER};

mod hashing_reader;

pub fn pack(sources: Vec<PathBuf>, out: PathBuf, key: Key, max_group_size: Option<u64>) -> Result<(), io::Error> {
	// TODO: delete generated files when an error occurs?
	
	if sources.is_empty() {
		panic!("at least one source must be provided");
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
	is_single_source: bool,
	progress_tracker: ProgressTracker
) -> Result<(), io::Error> {
	let mut file = File::create_new(out)?;
	
	file.write_all(BKY_HEADER)?;
	
	let iv = generate_iv();
	file.write_all(&iv)?;
	let mut encrypter = EncryptWriter::new(&mut file, key, iv);
	
	let mut header = HeaderBuilder::new(sources, Flags {
		is_single_source,
	});
	
	// skip header
	let header_size = header.header_size();
	
	// TODO: use random data
	let skip_buffer = vec![0; header_size.try_into().expect("header size too large")];
	encrypter.write_all(&skip_buffer)?;
	
	// write files
	for (source, entries) in sources {
		// FIXME: just a temporary fix to use the same order as the header
		let entry_map: BTreeMap<EntryPath, Entry> = match entries {
			Entries::File(entry) => {
				let mut map = BTreeMap::new();
				map.insert(entry.path.clone(), entry.clone());
				map
			},
			Entries::Directory(entries) => entries.iter()
				.map(|entry| (entry.path.clone(), entry.clone()))
				.collect(),
		};
		
		for entry in entry_map.values() {
			let file = File::open(entry.path.in_source(source))?;
			let mut hashing_reader = HashingReader::new(file);
			let size = io::copy(&mut hashing_reader, &mut encrypter)?;
			header.set_entry(source, &entry.path, size, hashing_reader.finalize());
			
			// use entry.size, as this is the expected value necessary to add up to 100%
			progress_tracker.advance(entry.size);
			
			if size != entry.size {
				let format = humansize::make_format(humansize::BINARY);
				eprintln!(
					"Warning: Size differs between indexing and archiving for {}.\nOriginal size: {}\nCurrent size: {}",
					entry.path,
					format(entry.size),
					format(size)
				);
			}
		}
	}
	
	// reset file
	file.seek(io::SeekFrom::Start((BKY_HEADER.len() + size_of::<IV>()) as u64))?;
	// reset encrypter
	let mut encrypter = EncryptWriter::new(&mut file, key, iv);
	
	header.write_header(&mut encrypter)?;
	
	Ok(())
}

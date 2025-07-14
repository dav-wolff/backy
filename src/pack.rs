use std::{fs::{self, File}, io::{self, Seek, SeekFrom, Write}, path::{Path, PathBuf}};

use anyhow::{anyhow, bail, Context};
use hashing_reader::HashingReader;
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

use crate::{crypto::{generate_iv, EncryptWriter, Key, IV}, header::{Flags, HeaderBuilder}, index::{Contents, Index, Sources}, progress::{ProgressDisplay, ProgressTracker}, Source, BKY_HEADER};

mod hashing_reader;

pub fn pack(sources: Vec<PathBuf>, out: PathBuf, key: Key, max_group_size: Option<u64>) -> anyhow::Result<()> {
	// TODO: delete generated files when an error occurs?
	
	if sources.is_empty() {
		bail!("at least one source must be provided");
	}
	
	// TODO: create separate ids for folders with same name
	let sources: Vec<_> = sources.into_iter()
		.map(|path| path.canonicalize().with_context(|| format!("canonicalizing path {:?}", path)))
		.map(|path_result| path_result.and_then(|path| {
			let file_name = path.file_name().ok_or_else(|| anyhow!("invalid path: {:?}", path))?;
			
			Ok(Source {
				id: file_name.to_string_lossy().into(),
				is_file: path.is_file(),
				path: path.into(),
			})
		}))
		.collect::<anyhow::Result<_>>()?;
	
	let is_single_source = sources.len() == 1;
	
	let index = Index::from_sources(sources, max_group_size).context("indexing source files")?;
	
	let progress_display = ProgressDisplay::new(index.total_size());
	
	match index.entries() {
		Contents::Grouped(groups) => {
			if !out.exists() {
				fs::create_dir(&out).with_context(|| format!("creating out directory in {:?}", out))?;
			}
			
			groups.into_par_iter()
				.enumerate()
				.map(|(i, group)| -> anyhow::Result<_> {
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
) -> anyhow::Result<()> {
	let mut file = File::create_new(out).with_context(|| format!("creating archive file at {out:?}"))?;
	
	// empty header line as the file is not yet a valid backy archive
	file.write_all(&[0; BKY_HEADER.len()]).with_context(|| format!("writing to archive file at {out:?}"))?;
	
	let iv = generate_iv()?;
	file.write_all(&iv).with_context(|| format!("writing to archive file at {out:?}"))?;
	let mut encrypter = EncryptWriter::new(&mut file, key, iv);
	
	let mut header = HeaderBuilder::new(sources, Flags {
		is_single_source,
		is_partial: false,
	});
	
	// skip header
	let header_size: usize = header.header_size().try_into().expect("header size too large");
	
	let mut skip_buffer: Vec<u8> = Vec::with_capacity(header_size);
	let skip_buffer = &mut skip_buffer.spare_capacity_mut()[..header_size];
	let skip_buffer = getrandom::fill_uninit(skip_buffer).map_err(|err| anyhow!(err).context("obtaining random bytes"))?;
	encrypter.write_all(skip_buffer).with_context(|| format!("writing to archive file at {out:?}"))?;
	
	let pack_contents_result = pack_contents(progress_tracker, sources, &mut encrypter, &mut header)
		.with_context(|| format!("archiving contents to {out:?}"));
	
	// TODO: should this error still be returned?
	if let Err(err) = &pack_contents_result {
		eprintln!("An error occured: {err:?}\n");
		eprintln!("Attempting to write header for partial archive...");
	}
	
	// reset file
	file.seek(SeekFrom::Start((BKY_HEADER.len() + size_of::<IV>()) as u64)).with_context(|| format!("writing to archive file at {out:?}"))?;
	// reset encrypter
	let mut encrypter = EncryptWriter::new(&mut file, key, iv);
	
	header.write_header(pack_contents_result.is_err(), &mut encrypter).with_context(|| format!("writing to archive file at {out:?}"))?;
	
	// finally write header line to indicate a valid backy archive
	file.rewind().with_context(|| format!("writing to archive file at {out:?}"))?;
	file.write_all(BKY_HEADER).with_context(|| format!("writing to archive file at {out:?}"))?;
	
	Ok(())
}

fn pack_contents(
	progress_tracker: ProgressTracker,
	sources: &Sources,
	mut writer: impl Write,
	header: &mut HeaderBuilder
) -> anyhow::Result<()> {
	// NOTE: the order must not change, as described in HeaderBuilder::push_entry
	for (source, entries) in sources {
		for entry in entries {
			let path = entry.path.in_source(source);
			let file = File::open(&path).with_context(|| format!("opening file at {:?}", path))?;
			let mut hashing_reader = HashingReader::new(file);
			let size = io::copy(&mut hashing_reader, &mut writer).with_context(|| format!("archiving file {:?}", path))?;
			// NOTE: only push the entry once it is complete as it still gets written to the header in case of an error
			header.push_entry(source, size, hashing_reader.finalize());
			
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
	
	Ok(())
}

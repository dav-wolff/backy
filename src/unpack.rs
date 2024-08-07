use std::{borrow::Cow, fs::{self, DirEntry, File}, io::{self, Read, Seek}, mem, path::{Path, PathBuf}};

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use xz2::read::XzDecoder;

use crate::{crypto::{DecryptReader, Key, IV}, progress::{ProgressDisplay, ProgressTracker}, BKY_HEADER};

pub fn unpack(archive: PathBuf, out: PathBuf, key: Key) -> Result<(), io::Error> {
	// TODO: return custom errors
	if !archive.exists() {
		panic!("Archive doesn't exist");
	}
	
	if archive.is_dir() {
		let entries: Vec<DirEntry> = fs::read_dir(archive)?.collect::<Result<_, _>>()?;
		let total_size: u64 = entries.iter()
			.map(|entry| -> Result<_, io::Error> {
				Ok(entry.metadata()?.len())
			})
			.sum::<Result<_, _>>()?;
		
		let progress_display = ProgressDisplay::new(total_size);
		
		entries.into_par_iter()
			.map(|entry| -> Result<_, io::Error> {
				let progress_tracker = progress_display.new_tracker(entry.path().to_string_lossy().into_owned(), entry.metadata()?.len());
				unpack_group(entry.path(), &out, key, progress_tracker)?;
				
				Ok(())
			})
			.collect::<Result<(), _>>()?;
	} else {
		let total_size = archive.metadata()?.len();
		let progress_display = ProgressDisplay::new(total_size);
		let progress_tracker = progress_display.new_tracker("Total", total_size);
		unpack_group(archive, &out, key, progress_tracker)?;
	}
	
	Ok(())
}

fn unpack_group(group: PathBuf, out: &Path, key: Key, progress_tracker: ProgressTracker) -> Result<(), io::Error> {
	let mut file = File::open(group)?;
	
	let mut header = [0u8; BKY_HEADER.len()];
	file.read_exact(&mut header)?;
	
	if header != BKY_HEADER {
		panic!("Not a backy archive");
	}
	
	let mut iv = IV::default();
	file.read_exact(&mut iv)?;
	let mut decrypter = DecryptReader::new(&file, key, iv);
	
	let mut buf32 = [0u8; mem::size_of::<u32>()];
	let mut buf64 = [0u8; mem::size_of::<u64>()];
	
	decrypter.read_exact(&mut buf32)?;
	let flags = u32::from_le_bytes(buf32);
	let is_single_source = flags & 1 != 0;
	
	decrypter.read_exact(&mut buf32)?;
	let groups_len = u32::from_le_bytes(buf32);
	
	let mut source_groups: Vec<(String, u64)> = Vec::with_capacity(groups_len as usize);
	
	// header
	for _ in 0..groups_len {
		decrypter.read_exact(&mut buf32)?;
		let id_len = u32::from_le_bytes(buf32);
		let mut id_buf = vec![0; id_len as usize];
		decrypter.read_exact(&mut id_buf[..])?;
		let source_id = String::from_utf8(id_buf).unwrap(); // TODO: return custom error
		
		decrypter.read_exact(&mut buf64)?;
		let source_size = u64::from_le_bytes(buf64);
		
		source_groups.push((source_id, source_size));
	}
	
	progress_tracker.advance((&file).stream_position()?);
	
	// tar archive
	let mut decoder = XzDecoder::new(decrypter);
	let mut prev_position = 0;
	for (source_id, source_size) in source_groups {
		let directory = if is_single_source {
			Cow::Borrowed(out)
		} else {
			Cow::Owned(out.join(source_id))
		};
		
		fs::create_dir_all(&directory)?;
		
		let read = (&mut decoder).take(source_size);
		let mut archive = tar::Archive::new(read);
		archive.unpack(&directory)?;
		read_to_end(archive.into_inner())?;
		
		progress_tracker.advance(decoder.total_in() - prev_position);
		prev_position = decoder.total_in();
	}
	
	Ok(())
}

fn read_to_end(mut read: impl Read) -> Result<(), io::Error> {
	let mut buf = [0u8; 1024];
	
	loop {
		match read.read(&mut buf) {
			Ok(0) => return Ok(()),
			Ok(_) => (),
			Err(err) => return Err(err),
		}
	}
}

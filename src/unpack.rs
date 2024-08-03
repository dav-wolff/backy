use std::{fs::{self, File}, io::{self, Read}, mem, path::{Path, PathBuf}};

use xz2::read::XzDecoder;

use crate::BKY_HEADER;

pub fn unpack(archive: PathBuf, out: PathBuf) -> Result<(), io::Error> {
	// TODO: return custom errors
	if !archive.exists() {
		panic!("Archive doesn't exist");
	}
	
	if archive.is_dir() {
		for entry in fs::read_dir(archive)? {
			unpack_group(entry?.path(), &out)?;
		}
	} else {
		unpack_group(archive, &out)?;
	}
	
	Ok(())
}

fn unpack_group(group: PathBuf, out: &Path) -> Result<(), io::Error> {
	let mut file = File::open(group)?;
	
	let mut header = [0u8; BKY_HEADER.len()];
	file.read_exact(&mut header)?;
	
	if header != BKY_HEADER {
		panic!("Not a backy archive");
	}
	
	let mut buf32 = [0u8; mem::size_of::<u32>()];
	let mut buf64 = [0u8; mem::size_of::<u64>()];
	
	file.read_exact(&mut buf32)?;
	let groups_len = u32::from_le_bytes(buf32);
	
	let mut source_groups: Vec<(String, u64)> = Vec::with_capacity(groups_len as usize);
	
	// header
	for _ in 0..groups_len {
		file.read_exact(&mut buf32)?;
		let id_len = u32::from_le_bytes(buf32);
		let mut id_buf = vec![0; id_len as usize];
		file.read_exact(&mut id_buf[..])?;
		let source_id = String::from_utf8(id_buf).unwrap(); // TODO: return custom error
		
		file.read_exact(&mut buf64)?;
		let source_size = u64::from_le_bytes(buf64);
		
		source_groups.push((source_id, source_size));
	}
	
	// tar archive
	let mut decoder = XzDecoder::new(&file);
	for (source_id, source_size) in source_groups {
		let directory = out.join(source_id);
		fs::create_dir_all(&directory)?;
		
		let read = (&mut decoder).take(source_size);
		let mut archive = tar::Archive::new(read);
		archive.unpack(&directory)?;
		read_to_end(archive.into_inner())?;
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

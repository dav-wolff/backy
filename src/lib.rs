use std::{ffi::OsString, io, path::PathBuf};

mod index;
use index::create_index;

#[derive(Debug)]
struct Source {
	id: OsString,
	path: PathBuf,
}

pub fn pack(sources: Vec<PathBuf>, out: PathBuf) -> Result<(), io::Error> {
	// TODO: get rid of unwraps
	// TODO: create separate ids for folders with same name
	let sources = sources.into_iter()
		.map(|path| path.canonicalize().unwrap())
		.map(|path| Source {
			id: path.file_name().unwrap().to_owned(),
			path,
		})
		.collect();
	
	let index = create_index(sources)?;
	
	println!("Largest file: {:?}", index[0].path);
	
	Ok(())
}

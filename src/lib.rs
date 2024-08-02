use std::{ffi::OsStr, io, path::PathBuf, rc::Rc};

mod index;
use index::create_index;

mod group;
use group::create_groups;

#[derive(Clone, Debug)]
struct SourceId(Rc<OsStr>);

#[derive(Debug)]
struct Source {
	id: SourceId,
	path: PathBuf,
}

pub fn pack(sources: Vec<PathBuf>, out: PathBuf, max_group_size: Option<u64>) -> Result<(), io::Error> {
	// TODO: get rid of unwraps
	// TODO: create separate ids for folders with same name
	let sources = sources.into_iter()
		.map(|path| path.canonicalize().unwrap())
		.map(|path| Source {
			id: SourceId(path.file_name().unwrap().into()),
			path,
		})
		.collect();
	
	let index = create_index(sources)?;
	
	if let Some(max_group_size) = max_group_size {
		let groups = create_groups(index, max_group_size);
		
		for group in &groups {
			println!("Group of size {} (error {}) with {} elements", humansize::format_size(group.size, humansize::BINARY), max_group_size - group.size, group.entries.len());
		}
	}
	
	Ok(())
}

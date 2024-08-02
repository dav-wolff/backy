use std::{io, path::PathBuf};

use walkdir::WalkDir;

use crate::{Source, SourceId};

#[derive(Debug)]
pub struct IndexEntry {
	pub source: SourceId,
	pub path: PathBuf,
	pub size: u64,
}

pub fn create_index(sources: Vec<Source>) -> Result<Vec<IndexEntry>, io::Error> {
	let format = humansize::make_format(humansize::BINARY);
	let mut index = Vec::new();
	
	let mut prev_index_len = 0;
	
	for source in sources {
		println!("Indexing files in {}...", source.path.to_string_lossy());
		
		let mut total_size = 0;
		
		for entry in WalkDir::new(source.path).follow_links(true) {
			let entry = entry?;
			
			if !entry.file_type().is_file() {
				continue;
			}
			
			let size = entry.metadata()?.len();
			total_size += size;
			
			index.push(IndexEntry {
				source: source.id.clone(),
				path: entry.path().to_owned(),
				size: entry.metadata()?.len(),
			});
		}
		
		let files_count = index.len() - prev_index_len;
		prev_index_len = index.len();
		println!("Found {files_count} files with a total size of {}.", format(total_size));
	}
	
	Ok(index)
}

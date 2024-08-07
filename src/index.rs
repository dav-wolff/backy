use std::io;

use walkdir::WalkDir;

use crate::{Entry, Source};

pub fn create_index(sources: Vec<Source>) -> Result<(Vec<Entry>, u64), io::Error> {
	let format = humansize::make_format(humansize::BINARY);
	let mut index = Vec::new();
	
	let mut total_size = 0;
	let mut prev_index_len = 0;
	
	for source in sources {
		if source.is_file {
			index.push(Entry {
				path: source.path.to_path_buf(),
				size: source.path.metadata()?.len(),
				source,
			});
			continue;
		}
		
		println!("Indexing files in {}...", source.path.to_string_lossy());
		
		let mut source_size = 0;
		
		for entry in WalkDir::new(&source.path).follow_links(true) {
			let entry = entry?;
			
			if !entry.file_type().is_file() {
				continue;
			}
			
			let size = entry.metadata()?.len();
			source_size += size;
			
			index.push(Entry {
				source: source.clone(),
				path: entry.path().to_owned(),
				size: entry.metadata()?.len(),
			});
		}
		
		total_size += source_size;
		let files_count = index.len() - prev_index_len;
		prev_index_len = index.len();
		println!("Found {files_count} files with a total size of {}.", format(source_size));
	}
	
	Ok((index, total_size))
}

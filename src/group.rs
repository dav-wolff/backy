use std::path::PathBuf;

use crate::{index::IndexEntry, SourceId};

#[derive(Debug)]
pub struct GroupEntry {
	pub source: SourceId,
	pub path: PathBuf,
}

#[derive(Debug)]
pub struct Group {
	pub size: u64,
	pub entries: Vec<GroupEntry>,
}

pub fn create_groups(mut index: Vec<IndexEntry>, max_group_size: u64) -> Vec<Group> {
	index.sort_by(|left, right| right.size.cmp(&left.size));
	let mut groups: Vec<Group> = Vec::new();
	
	for index_entry in index {
		let group_entry = GroupEntry {
			source: index_entry.source,
			path: index_entry.path,
		};
		
		let Some(group_position) = groups.iter()
			.position(|group| group.size + index_entry.size <= max_group_size)
		else {
			groups.push(Group {
				size: index_entry.size,
				entries: vec![group_entry],
			});
			continue;
		};
		
		let group = &mut groups[group_position];
		group.size += index_entry.size;
		group.entries.push(group_entry);
		
		if group_position == 0 || groups[group_position - 1].size >= groups[group_position].size {
			continue;
		}
		
		let group = groups.remove(group_position);
		
		let insert_position = match groups.binary_search_by(|probe| group.size.cmp(&probe.size)) {
			Ok(found_position) => found_position + 1,
			Err(insert_position) => insert_position,
		};
		
		groups.insert(insert_position, group);
	}
	
	groups
}

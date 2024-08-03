use crate::Entry;

#[derive(Debug)]
pub struct Group {
	pub size: u64,
	pub entries: Vec<Entry>,
}

pub fn create_groups(mut index: Vec<Entry>, max_group_size: u64) -> Vec<Group> {
	index.sort_by(|left, right| right.size.cmp(&left.size));
	let mut groups: Vec<Group> = Vec::new();
	
	for entry in index {
		let Some(group_position) = groups.iter()
			.position(|group| group.size + entry.size <= max_group_size)
		else {
			groups.push(Group {
				size: entry.size,
				entries: vec![entry],
			});
			continue;
		};
		
		let group = &mut groups[group_position];
		group.size += entry.size;
		group.entries.push(entry);
		
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

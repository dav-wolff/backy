// TODO: pack archives with multiple subarchives, run tests on entire archives or individual subarchives

use std::{
	fs::{self, OpenOptions},
	io::{self, Read, Seek, Write},
	sync::LazyLock,
};
use base64::{
	prelude::BASE64_STANDARD,
	Engine as _,
};

use backy::{
	Key,
};

mod common;
use common::archive_description::*;

const KEY_TEXT: &str = "d6k//cJHeIXNlYn8ip1no0MDVWYBxZCEU/RwIzR5cMY=";
static KEY: LazyLock<Key> = LazyLock::new(|| {
	let mut key = Key::default();
	let decoded_len = BASE64_STANDARD.decode_slice(KEY_TEXT, &mut key).unwrap();
	assert!(decoded_len == 32, "key has wrong size");
	key
});


struct Archive {
	description: ArchiveDescription,
	archive: backy::Archive,
	corrupt_archive: Option<backy::Archive>,
}

impl Archive {
	fn load(description: ArchiveDescription) -> Self {
		let archive = backy::Archive::new(description.archive_path(), *KEY).unwrap();
		let corrupt_archive = if description.has_corrupt_archive() {
			Some(backy::Archive::new(description.corrupt_archive_path(), *KEY).unwrap())
		} else {
			None
		};
		
		Self {
			description,
			archive,
			corrupt_archive,
		}
	}
}

const TESTS: &[&dyn TestArchive] = &[
	&ListTest,
	&ListSourcesTest,
	&UnpackTest,
	&GetTest,
	&CheckTest,
	&FailedCheckTest,
	&FailedGetTest,
	&FailedUnpackTest,
];

#[test]
fn test() {
	for &archive_description in ARCHIVES {
		println!("archive: {}", archive_description.name);
		pack_archive(archive_description);
		let mut archive = Archive::load(archive_description);
		
		for test in TESTS {
			test.test_archive(&mut archive);
		}
	}
}

fn pack_archive(archive_description: ArchiveDescription) {
	let sources = archive_description.source_names.into_iter()
		.map(|source_name| SOURCES_DIR.join(source_name))
		.collect();
	
	backy::pack(sources, archive_description.archive_path(), *KEY, None, false).unwrap();
	
	// corrupted copy
	let corrupt_archive_path = archive_description.corrupt_archive_path();
	fs::copy(archive_description.archive_path(), &corrupt_archive_path).unwrap();
	
	let mut file = OpenOptions::new()
		.read(true)
		.write(true)
		.open(&corrupt_archive_path).unwrap();
	// seek to the middle of the content
	file.seek(io::SeekFrom::End(-(archive_description.content_size() as i64 / 2 + 1))).unwrap();
	// change one byte
	let mut buf = [0];
	file.read_exact(&mut buf).unwrap();
	buf[0] = buf[0].wrapping_add(1);
	file.seek(io::SeekFrom::Current(-1)).unwrap();
	file.write_all(&buf).unwrap();
}

trait TestArchive {
	fn test_archive(&self, archive: &mut Archive);
}

struct ListTest;

impl TestArchive for ListTest {
	fn test_archive(&self, archive: &mut Archive) {
		let mut expected_paths: Vec<_> = archive.description.files().map(|file| file.path).collect();
		let mut actual_paths: Vec<_> = archive.archive.file_paths().collect();
		expected_paths.sort_unstable();
		actual_paths.sort_unstable();
		assert_eq!(expected_paths, actual_paths);
	}
}

struct ListSourcesTest;

impl TestArchive for ListSourcesTest {
	fn test_archive(&self, archive: &mut Archive) {
		let mut expected_sources: Vec<_> = archive.description.source_names.into_iter().collect();
		let mut actual_sources: Vec<_> = archive.archive.sources().collect();
		expected_sources.sort_unstable();
		actual_sources.sort_unstable();
		assert_eq!(expected_sources, actual_sources);
	}
}

struct UnpackTest;

impl TestArchive for UnpackTest {
	fn test_archive(&self, archive: &mut Archive) {
		let destination = archive.description.unpack_path();
		archive.archive.unpack(&destination, false).unwrap();
		
		for source in archive.description.sources() {
			let src = SOURCES_DIR.join(source.name);
			let dest = if archive.description.is_single_source() && !source.is_file() {
				&destination
			} else {
				&destination.join(source.name)
			};
			println!("comparing {src:?} and {dest:?}");
			
			if source.is_file() {
				assert!(fs::read(src).unwrap() == fs::read(dest).unwrap());
			} else {
				assert!(!dir_diff::is_different(src, dest).unwrap());
			}
		}
	}
}

struct GetTest;

impl TestArchive for GetTest {
	fn test_archive(&self, archive: &mut Archive) {
		for source in archive.description.sources() {
			for file in source.files() {
				println!("checking content of file {}", file.path);
				let mut reader = archive.archive.get_file(Some(source.name), file.path).unwrap().unwrap();
				let mut content = Vec::new();
				reader.read_to_end(&mut content).unwrap();
				assert_eq!(content, file.content);
			}
		}
	}
}

struct CheckTest;

impl TestArchive for CheckTest {
	fn test_archive(&self, archive: &mut Archive) {
		assert!(archive.archive.check_integrity(false).unwrap())
	}
}

struct FailedCheckTest;

impl TestArchive for FailedCheckTest {
	fn test_archive(&self, archive: &mut Archive) {
		let Some(corrupt_archive) = &mut archive.corrupt_archive else {
			return;
		};
		
		assert!(!corrupt_archive.check_integrity(false).unwrap());
	}
}

struct FailedGetTest;

impl TestArchive for FailedGetTest {
	fn test_archive(&self, archive: &mut Archive) {
		let Some(corrupt_archive) = &mut archive.corrupt_archive else {
			return;
		};
		
		let mut buf = Vec::new();
		let mut corrupt_files = 0;
		
		for source in archive.description.sources() {
			for file in source.files() {
				let mut reader = corrupt_archive.get_file(Some(source.name), file.path).unwrap().unwrap();
				reader.read_to_end(&mut buf).unwrap();
				if reader.is_corrupted() {
					corrupt_files += 1;
				}
			}
		}
		
		// one byte of the archive was changed, so exactly one file should be affected
		assert_eq!(corrupt_files, 1);
	}
}

struct FailedUnpackTest;

impl TestArchive for FailedUnpackTest {
	fn test_archive(&self, archive: &mut Archive) {
		let Some(corrupt_archive) = &mut archive.corrupt_archive else {
			return;
		};
		
		let result = corrupt_archive.unpack(archive.description.corrupt_unpack_path(), false);
		
		// TODO: check specific type of error
		assert!(result.is_err());
	}
}

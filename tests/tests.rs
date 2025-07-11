// TODO: pack archives with multiple subarchives, run tests on entire archives or individual subarchives

use std::{fs, io::{self, Read}, path::{Path, PathBuf}, sync::LazyLock};

use backy::Key;
use base64::{prelude::BASE64_STANDARD, Engine as _};
use either::Either;

#[derive(Clone, Copy, Debug)]
enum OneOrMany<T>
where
	T: 'static + Copy,
{
	One(T),
	Many(&'static [T]),
}

impl<T> OneOrMany<T>
where
	T: 'static + Copy,
{
	fn is_one(self) -> bool {
		matches!(self, Self::One(_))
	}
}

impl<T> IntoIterator for OneOrMany<T>
where
	T: Copy,
{
	type Item = T;
	type IntoIter = Either<std::iter::Once<T>, std::iter::Copied<std::slice::Iter<'static, T>>>;
	
	fn into_iter(self) -> Self::IntoIter {
		match self {
			Self::One(item) => Either::Left(std::iter::once(item)),
			Self::Many(items) => Either::Right(items.into_iter().copied()),
		}
	}
}

#[derive(Clone, Copy, Debug)]
struct FileDescription {
	path: &'static str,
	content: &'static [u8],
}

impl FileDescription {
	const fn new(path: &'static str, content: &'static [u8]) -> Self {
		Self {
			path,
			content,
		}
	}
}

#[derive(Clone, Copy, Debug)]
struct SourceDescription {
	name: &'static str,
	entries: OneOrMany<FileDescription>,
}

impl SourceDescription {
	const fn new_single_file(name: &'static str, content: &'static [u8]) -> Self {
		Self {
			name,
			entries: OneOrMany::One(FileDescription::new("", content)),
		}
	}
	
	const fn new(name: &'static str, entries: &'static [FileDescription]) -> Self {
		Self {
			name,
			entries: OneOrMany::Many(entries),
		}
	}
	
	fn find(name: &str) -> Self {
		*SOURCES.iter().find(|source| source.name == name).unwrap()
	}
	
	fn files(self) -> impl Iterator<Item = FileDescription> {
		self.entries.into_iter()
	}
	
	fn is_file(self) -> bool {
		self.entries.is_one()
	}
}

const CIRCLE_PNG: &[u8] = include_bytes!("../test_sources/circle.png");
const HELLO_WORLD: &[u8] = b"Hello world!\n";

#[derive(Clone, Copy, Debug)]
struct ArchiveDescription {
	name: &'static str,
	source_names: OneOrMany<&'static str>,
}

impl ArchiveDescription {
	const fn new_single_source(name: &'static str) -> Self {
		Self {
			name,
			source_names: OneOrMany::One(name),
		}
	}
	
	const fn new(name: &'static str, sources: &'static [&'static str]) -> Self {
		Self {
			name,
			source_names: OneOrMany::Many(sources),
		}
	}
	
	fn sources(self) -> impl Iterator<Item = SourceDescription> {
		self.source_names.into_iter()
			.map(SourceDescription::find)
	}
	
	fn files(self) -> impl Iterator<Item = FileDescription> {
		self.sources()
			.flat_map(SourceDescription::files)
	}
	
	fn is_single_source(self) -> bool {
		self.source_names.is_one()
	}
}

const KEY_TEXT: &str = "d6k//cJHeIXNlYn8ip1no0MDVWYBxZCEU/RwIzR5cMY=";
static KEY: LazyLock<Key> = LazyLock::new(|| {
	let mut key = Key::default();
	let decoded_len = BASE64_STANDARD.decode_slice(KEY_TEXT, &mut key).unwrap();
	assert!(decoded_len == 32, "key has wrong size");
	key
});

static SOURCES_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("test_sources")
});
static ARCHIVES_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
	let archives_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("archives");
	if let Err(err) = fs::remove_dir_all(&archives_dir) {
		assert!(err.kind() == io::ErrorKind::NotFound);
	};
	fs::create_dir(&archives_dir).unwrap();
	archives_dir
});
static UNPACK_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
	let unpack_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("unpacked_archives");
	if let Err(err) = fs::remove_dir_all(&unpack_dir) {
		assert!(err.kind() == io::ErrorKind::NotFound);
	};
	fs::create_dir(&unpack_dir).unwrap();
	unpack_dir
});

// TODO: empty directory + other generated?
const SOURCES: &[SourceDescription] = &[
	SourceDescription::new_single_file("empty_file", &[]),
	SourceDescription::new_single_file("hello_world", HELLO_WORLD),
	SourceDescription::new_single_file("circle.png", CIRCLE_PNG),
	SourceDescription::new("single_empty_file", &[
		FileDescription::new("empty_file", &[]),
	]),
	SourceDescription::new("single_file", &[
		FileDescription::new("hello_world", HELLO_WORLD),
	]),
	SourceDescription::new("multiple_files", &[
		FileDescription::new("empty", &[]),
		FileDescription::new("hello_world", HELLO_WORLD),
		FileDescription::new("circle.png", CIRCLE_PNG),
	]),
	SourceDescription::new("nested_files", &[
		FileDescription::new("empty", &[]),
		FileDescription::new("greetings/hello_world", HELLO_WORLD),
		FileDescription::new("greetings/hello_there", b"Hello there!\n"),
		FileDescription::new("greetings/hi", b"Hi"),
		FileDescription::new("pictures/shapes/circle.png", CIRCLE_PNG),
	]),
	SourceDescription::new("deeply_nested", &[
		FileDescription::new("very/deeply/nested/directory/structure/hello_world", HELLO_WORLD),
	]),
];

const ARCHIVES: &[ArchiveDescription] = &[
	ArchiveDescription::new_single_source("empty_file"),
	ArchiveDescription::new_single_source("hello_world"),
	ArchiveDescription::new_single_source("circle.png"),
	ArchiveDescription::new_single_source("single_empty_file"),
	ArchiveDescription::new_single_source("single_file"),
	ArchiveDescription::new_single_source("multiple_files"),
	ArchiveDescription::new_single_source("nested_files"),
	ArchiveDescription::new_single_source("deeply_nested"),
	ArchiveDescription::new("single_files", &["empty_file", "hello_world", "circle.png"]),
	ArchiveDescription::new("directories", &["single_empty_file", "single_file", "multiple_files", "nested_files", "deeply_nested"]),
	ArchiveDescription::new("mixed", &["empty_file", "multiple_files", "nested_files"]),
	ArchiveDescription::new("all", &["empty_file", "hello_world", "circle.png", "single_empty_file", "single_file", "multiple_files", "nested_files", "deeply_nested"]),
];

struct Archive {
	description: ArchiveDescription,
	archive: backy::Archive,
}

impl Archive {
	fn load(description: ArchiveDescription) -> Self {
		let archive = backy::Archive::new(ARCHIVES_DIR.join(format!("{}.bky", description.name)), *KEY).unwrap();
		Self {
			description,
			archive,
		}
	}
}

const TESTS: &[&dyn TestArchive] = &[
	&ListTest,
	&ListSourcesTest,
	&UnpackTest,
	&GetTest,
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
	
	println!("{SOURCES:?}")
}

fn pack_archive(archive_description: ArchiveDescription) {
	let sources = archive_description.source_names.into_iter()
		.map(|source_name| SOURCES_DIR.join(source_name))
		.collect();
	let out = ARCHIVES_DIR.join(format!("{}.bky", archive_description.name));
	
	backy::pack(sources, out, *KEY, None).unwrap();
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
		let destination = UNPACK_DIR.join(archive.description.name);
		archive.archive.unpack(&destination).unwrap();
		
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

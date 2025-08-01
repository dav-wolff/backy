use std::{
	fs,
	io,
	path::{Path, PathBuf},
	sync::LazyLock,
};

use super::util::OneOrMany;

#[derive(Clone, Copy, Debug)]
pub struct FileDescription {
	pub path: &'static str,
	pub content: &'static [u8],
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
pub struct SourceDescription {
	pub name: &'static str,
	pub entries: OneOrMany<FileDescription>,
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
	
	pub fn find(name: &str) -> Self {
		*SOURCES.iter().find(|source| source.name == name).unwrap()
	}
	
	pub fn files(self) -> impl Iterator<Item = FileDescription> {
		self.entries.into_iter()
	}
	
	pub fn is_file(self) -> bool {
		self.entries.is_one()
	}
}

#[derive(Clone, Copy, Debug)]
pub struct ArchiveDescription {
	pub name: &'static str,
	pub source_names: OneOrMany<&'static str>,
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
	
	pub fn sources(self) -> impl Iterator<Item = SourceDescription> {
		self.source_names.into_iter()
			.map(SourceDescription::find)
	}
	
	pub fn files(self) -> impl Iterator<Item = FileDescription> {
		self.sources()
			.flat_map(SourceDescription::files)
	}
	
	pub fn is_single_source(self) -> bool {
		self.source_names.is_one()
	}
	
	pub fn archive_path(self) -> PathBuf {
		ARCHIVES_DIR.join(format!("{}.bky", self.name))
	}
	
	pub fn corrupt_archive_path(self) -> PathBuf {
		CORRUPT_ARCHIVES_DIR.join(format!("{}.bky", self.name))
	}
	
	pub fn unpack_path(self) -> PathBuf {
		UNPACK_DIR.join(self.name)
	}
	
	pub fn corrupt_unpack_path(self) -> PathBuf {
		CORRUPT_UNPACK_DIR.join(self.name)
	}
	
	pub fn content_size(self) -> u64 {
		self.files()
			.map(|file| file.content.len() as u64)
			.sum()
	}
	
	pub fn has_corrupt_archive(self) -> bool {
		self.content_size() != 0
	}
}

pub static SOURCES_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("test_sources")
});

fn cleaned_dir(name: &'static str) -> PathBuf {
	let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
	if let Err(err) = fs::remove_dir_all(&dir) {
		assert!(err.kind() == io::ErrorKind::NotFound);
	}
	fs::create_dir(&dir).unwrap();
	dir
}

pub static ARCHIVES_DIR: LazyLock<PathBuf> = LazyLock::new(|| cleaned_dir("archives"));
pub static UNPACK_DIR: LazyLock<PathBuf> = LazyLock::new(|| cleaned_dir("unpacked_archives"));
pub static CORRUPT_ARCHIVES_DIR: LazyLock<PathBuf> = LazyLock::new(|| cleaned_dir("corrupted_archives"));
pub static CORRUPT_UNPACK_DIR: LazyLock<PathBuf> = LazyLock::new(|| cleaned_dir("unpacked_corrupted_archives"));

const CIRCLE_PNG: &[u8] = include_bytes!("../../test_sources/circle.png");
const HELLO_WORLD: &[u8] = b"Hello world!\n";

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

pub const ARCHIVES: &[ArchiveDescription] = &[
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

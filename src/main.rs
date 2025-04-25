#![forbid(unsafe_code)]
#![deny(non_snake_case)]

use std::{fs, io::{self, Write}, path::PathBuf};

use backy::Key;
use base64::{prelude::BASE64_STANDARD, Engine};
use clap::{command, Args, Parser, Subcommand};

fn parse_size(arg: &str) -> Result<u64, parse_size::Error> {
	parse_size::Config::new()
		.with_binary()
		.with_default_factor(1024 * 1024 * 1024)
		.parse_size(arg)
}

#[derive(Parser, Debug)]
#[command(version, propagate_version = true, author, about)]
struct BackyArgs {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
enum Commands {
	/// Generate a key for encrypting and decrypting backy archives
	GenerateKey,
	/// Create a new backy archive from the given sources
	Pack(PackArgs),
	/// Unpacks a backy archive into its sources
	Unpack(UnpackArgs),
	/// Lists all sources contained in a backy archive
	ListSources(ListSourcesArgs),
	/// Lists all files contained in a backy archive
	List(ListArgs),
	/// Extracts a single file from the backy archive
	Get(GetArgs),
}

#[derive(Args, Clone, Debug)]
struct PackArgs {
	/// All directories / files to include in the backup
	#[arg(required = true)]
	sources: Vec<PathBuf>,
	/// File to write backup data to, or directory to write files to if --size is specified
	#[arg(short, long, default_value = "backup.bky")]
	out: PathBuf,
	/// Maximum size of files in the out directory, defaults to GiB if no unit is given
	#[arg(short, long, value_parser = parse_size)]
	size: Option<u64>,
	// TODO: add compression again?
	// /// Level of compression to use
	// #[arg(short = 'l', long, value_parser = parse_compression_level, default_value = "9")]
	// compression_level: u32,
	/// Key to use for encryption
	#[arg(short, long, conflicts_with = "key_file")]
	key: Option<String>,
	/// File containing the key to use for encryption
	#[arg(short = 'f', long, conflicts_with = "key")]
	key_file: Option<PathBuf>,
}

#[derive(Args, Clone, Debug)]
struct UnpackArgs {
	/// The backy archive to unpack (can be a file or directory)
	archive: PathBuf,
	/// Directory to unpack the sources into
	#[arg(short, long, default_value = ".")]
	out: PathBuf,
	/// Key to use for decryption
	#[arg(short, long, conflicts_with = "key_file")]
	key: Option<String>,
	/// File containing the key to use for decryption
	#[arg(short = 'f', long, conflicts_with = "key")]
	key_file: Option<PathBuf>,
}

#[derive(Args, Clone, Debug)]
struct ListSourcesArgs {
	/// The backy archive to list sources of (can be a file or directory)
	archive: PathBuf,
	/// Key to use for decryption
	#[arg(short, long, conflicts_with = "key_file")]
	key: Option<String>,
	/// File containing the key to use for decryption
	#[arg(short = 'f', long, conflicts_with = "key")]
	key_file: Option<PathBuf>,
}

#[derive(Args, Clone, Debug)]
struct ListArgs {
	/// The backy archive to list files of (can be a file or directory)
	archive: PathBuf,
	/// The source containing the files to be listed
	#[arg(short, long)]
	source: Option<String>,
	/// Key to use for decryption
	#[arg(short, long, conflicts_with = "key_file")]
	key: Option<String>,
	/// File containing the key to use for decryption
	#[arg(short = 'f', long, conflicts_with = "key")]
	key_file: Option<PathBuf>,
}

#[derive(Args, Clone, Debug)]
struct GetArgs {
	/// The backy archive to extract the file from (can be a file or directory)
	archive: PathBuf,
	/// The path of the file to extract
	path: String,
	/// The source to look for the file in
	#[arg(short, long)]
	source: Option<String>,
	/// Key to use for decryption
	#[arg(short, long, conflicts_with = "key_file")]
	key: Option<String>,
	/// File containing the key to use for decryption
	#[arg(short = 'f', long, conflicts_with = "key")]
	key_file: Option<PathBuf>,
}

fn main() {
	let args = BackyArgs::parse();
	
	match args.command {
		Commands::GenerateKey => {
			let key = backy::generate_key();
			let base64_key = BASE64_STANDARD.encode(key);
			println!("{base64_key}");
		},
		Commands::Pack(pack_args) => {
			let key = get_key(pack_args.key, pack_args.key_file);
			// TODO handle file already exists
			backy::pack(pack_args.sources, pack_args.out, key, pack_args.size).unwrap();
		},
		Commands::Unpack(unpack_args) => {
			let key = get_key(unpack_args.key, unpack_args.key_file);
			backy::Archive::new(unpack_args.archive, key).unwrap()
				.unpack(unpack_args.out).unwrap();
		},
		Commands::ListSources(list_sources_args) => {
			let key = get_key(list_sources_args.key, list_sources_args.key_file);
			let archive = backy::Archive::new(list_sources_args.archive, key).unwrap();
			for source in archive.sources() {
				println!("{source}");
			}
		},
		Commands::List(list_args) => {
			let key = get_key(list_args.key, list_args.key_file);
			let archive = backy::Archive::new(list_args.archive, key).unwrap();
			
			if let Some(source) = &list_args.source {
				if !archive.sources().any(|s| s == source) {
					panic!("source {source} is not contained in this archive");
				}
			}
			
			if list_args.source.is_some() {
				todo!("filter paths by source");
			}
			
			let mut stdout = std::io::stdout();
			let mut writer = stdout.lock();
			for path in archive.file_paths() {
				// TODO: include source in output?
				writer.write_all(path.as_bytes()).unwrap();
				writer.write_all(b"\n").unwrap();
			}
			stdout.flush().unwrap();
		},
		Commands::Get(get_args) => {
			let key = get_key(get_args.key, get_args.key_file);
			let mut archive = backy::Archive::new(get_args.archive, key).unwrap();
			
			let mut stdout = io::stdout().lock();
			let mut reader = archive.get_file(get_args.source.as_ref().map(AsRef::as_ref), &get_args.path).unwrap().unwrap();
			io::copy(&mut reader, &mut stdout).unwrap();
		},
	}
}

fn get_key(key_string: Option<String>, key_file: Option<PathBuf>) -> Key {
	// TODO: handle errors
	let base64_key = match (key_string, key_file) {
		(Some(_), Some(_)) => unreachable!("clap ensures key and key_file are mutually exclusive"),
		(Some(base64_key), None) => base64_key,
		(None, Some(key_file)) => {
			let string = fs::read_to_string(key_file).unwrap();
			string.trim().to_owned()
		},
		(None, None) => {
			rpassword::prompt_password("Enter key: ").unwrap()
		},
	};
	
	let mut key = Key::default();
	
	if BASE64_STANDARD.decode_slice(base64_key, &mut key).unwrap() != 32 {
		panic!("key has wrong size");
	}
	
	key
}

#![forbid(unsafe_code)]
#![deny(non_snake_case)]

use std::{fs, path::PathBuf};

use backy::Key;
use base64::{prelude::BASE64_STANDARD, Engine};
use clap::{command, Args, Parser, Subcommand};

fn parse_size(arg: &str) -> Result<u64, parse_size::Error> {
	parse_size::Config::new()
		.with_binary()
		.with_default_factor(1024 * 1024 * 1024)
		.parse_size(arg)
}

fn parse_compression_level(arg: &str) -> Result<u32, String> {
	let level: i32 = arg.parse().map_err(|err| format!("{err}"))?;
	match level {
		0..=9 => Ok(level as u32),
		_ => Err("compression-level must be a number from 0 to 9".to_owned()),
	}
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
	/// Lists the sources a backy archive contains
	ListSources(ListSourcesArgs),
}

#[derive(Args, Clone, Debug)]
struct PackArgs {
	/// All directories / files to include in the backup
	#[arg(required = true)]
	sources: Vec<PathBuf>,
	/// File to write backup data to, or directory to write files to if --size is specified, defaults to GiB if no unit is given
	#[arg(short, long, default_value = "backup.bky")]
	out: PathBuf,
	/// Maximum size of files in the out directory
	#[arg(short, long, value_parser = parse_size)]
	size: Option<u64>,
	/// Level of compression to use
	#[arg(short = 'l', long, value_parser = parse_compression_level, default_value = "9")]
	compression_level: u32,
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
	/// The backy archive to unpack (can be a file or directory)
	archive: PathBuf,
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
			backy::pack(pack_args.sources, pack_args.out, key, pack_args.size, pack_args.compression_level).unwrap();
		},
		Commands::Unpack(unpack_args) => {
			let key = get_key(unpack_args.key, unpack_args.key_file);
			backy::Archive::new(unpack_args.archive, key).unpack(unpack_args.out).unwrap();
		},
		Commands::ListSources(list_sources_args) => {
			let key = get_key(list_sources_args.key, list_sources_args.key_file);
			let archive = backy::Archive::new(list_sources_args.archive, key);
			for source in archive.sources().unwrap() {
				println!("{source}");
			}
		}
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

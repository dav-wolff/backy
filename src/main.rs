#![forbid(unsafe_code)]
#![deny(non_snake_case)]

use std::{fs, io::{self, Write}, path::PathBuf};

use anyhow::{ensure, Context};
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
	/// Key to use for decryption
	#[arg(short, long, global = true, conflicts_with = "key_file")]
	key: Option<String>,
	/// File containing the key to use for decryption
	#[arg(short = 'f', long, global = true, conflicts_with = "key")]
	key_file: Option<PathBuf>,
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
}

#[derive(Args, Clone, Debug)]
struct UnpackArgs {
	/// The backy archive to unpack (can be a file or directory)
	archive: PathBuf,
	/// Directory to unpack the sources into
	#[arg(short, long, default_value = ".")]
	out: PathBuf,
}

#[derive(Args, Clone, Debug)]
struct ListSourcesArgs {
	/// The backy archive to list sources of (can be a file or directory)
	archive: PathBuf,
}

#[derive(Args, Clone, Debug)]
struct ListArgs {
	/// The backy archive to list files of (can be a file or directory)
	archive: PathBuf,
	/// The source containing the files to be listed
	#[arg(short, long)]
	source: Option<String>,
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
}

fn main() -> anyhow::Result<()> {
	let args = BackyArgs::parse();
	
	if matches!(args.command, Commands::GenerateKey) {
		let key = backy::generate_key()?;
		let base64_key = BASE64_STANDARD.encode(key);
		println!("{base64_key}");
		return Ok(());
	}
	
	let key = get_key(args.key, args.key_file)?;
	
	match args.command {
		Commands::GenerateKey => unreachable!("handled with early return"),
		Commands::Pack(pack_args) => {
			// TODO handle file already exists
			backy::pack(pack_args.sources, pack_args.out, key, pack_args.size)?;
		},
		Commands::Unpack(unpack_args) => {
			backy::Archive::new(unpack_args.archive, key)?
				.unpack(unpack_args.out)?;
		},
		Commands::ListSources(list_sources_args) => {
			let archive = backy::Archive::new(list_sources_args.archive, key)?;
			for source in archive.sources() {
				println!("{source}");
			}
		},
		Commands::List(list_args) => {
			let archive = backy::Archive::new(list_args.archive, key)?;
			
			if let Some(source) = &list_args.source {
				ensure!(archive.sources().any(|s| s == source), "source {source} is not contained in this archive");
			}
			
			if list_args.source.is_some() {
				todo!("filter paths by source");
			}
			
			let mut stdout = std::io::stdout();
			let mut writer = stdout.lock();
			for path in archive.file_paths() {
				// TODO: include source in output?
				writer.write_all(path.as_bytes()).context("writing to stdout")?;
				writer.write_all(b"\n").context("writing to stdout")?;
			}
			stdout.flush().context("writing to stdout")?;
		},
		Commands::Get(get_args) => {
			let mut archive = backy::Archive::new(get_args.archive, key)?;
			
			let mut stdout = io::stdout().lock();
			let mut reader = archive.get_file(get_args.source.as_ref().map(AsRef::as_ref), &get_args.path)?.unwrap();
			io::copy(&mut reader, &mut stdout).context("reading file contents")?;
		},
	}
	
	Ok(())
}

fn get_key(key_string: Option<String>, key_file: Option<PathBuf>) -> anyhow::Result<Key> {
	let base64_key = match (key_string, key_file) {
		(Some(_), Some(_)) => unreachable!("clap ensures key and key_file are mutually exclusive"),
		(Some(base64_key), None) => base64_key,
		(None, Some(key_file)) => {
			let string = fs::read_to_string(key_file).context("reading key from file")?;
			string.trim().to_owned()
		},
		(None, None) => {
			rpassword::prompt_password("Enter key: ").context("reading key from stdin")?
		},
	};
	
	let mut key = Key::default();
	
	let decoded_len = BASE64_STANDARD.decode_slice(base64_key, &mut key).context("base64 decoding key")?;
	ensure!(decoded_len == 32, "key has wrong size");
	
	Ok(key)
}

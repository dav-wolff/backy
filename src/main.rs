#![forbid(unsafe_code)]
#![deny(non_snake_case)]

use std::path::PathBuf;

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
	Pack(PackArgs),
	Unpack(UnpackArgs),
}

/// Create a new backy archive from the given sources
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
}

/// Unpacks a backy archive into its sources
#[derive(Args, Clone, Debug)]
struct UnpackArgs {
	/// The backy archive to unpack (can be a file or directory)
	archive: PathBuf,
	// TODO: come up with a description
	#[arg(short, long, default_value = "backy")]
	out: PathBuf,
}

fn main() {
	let args = BackyArgs::parse();
	
	match args.command {
		Commands::Pack(pack_args) => {
			backy::pack(pack_args.sources, pack_args.out, pack_args.size, pack_args.compression_level).unwrap();
		},
		Commands::Unpack(unpack_args) => {
			backy::unpack(unpack_args.archive, unpack_args.out).unwrap();
		},
	}
}

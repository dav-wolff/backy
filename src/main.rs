use std::path::PathBuf;

use clap::{command, Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, propagate_version = true, author, about)]
struct BackyArgs {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
enum Commands {
	Pack(PackArgs),
}

/// Create a new backup from the given sources
#[derive(Args, Clone, Debug)]
struct PackArgs {
	/// All directories / files to include in the backup
	#[arg(required = true)]
	sources: Vec<PathBuf>,
	#[arg(short, long, default_value = "backy")]
	out: PathBuf,
}

fn main() {
	let args = BackyArgs::parse();
	
	match args.command {
		Commands::Pack(pack_args) => {
			backy::pack(pack_args.sources, pack_args.out);
		},
	}
}

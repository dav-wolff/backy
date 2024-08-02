use std::path::PathBuf;

pub fn pack(sources: Vec<PathBuf>, out: PathBuf) {
	println!("out: {out:?}");
	for source in sources {
		println!("src: {source:?}");
	}
}

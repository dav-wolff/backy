use std::io::{self, Read};

pub struct HashingReader<R: Read> {
	inner: R,
	hasher: blake3::Hasher,
}

impl<R: Read> HashingReader<R> {
	pub fn new(inner: R) -> Self {
		Self {
			inner,
			hasher: blake3::Hasher::new(),
		}
	}
	
	pub fn finalize(&self) -> blake3::Hash {
		self.hasher.finalize()
	}
}

impl<R: Read> Read for HashingReader<R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let bytes_read = self.inner.read(buf)?;
		self.hasher.update(&buf[..bytes_read]);
		
		Ok(bytes_read)
	}
}

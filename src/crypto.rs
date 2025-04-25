use std::io::{self, Read, Seek, SeekFrom, Write};

use chacha20::{cipher::{consts::{U24, U32}, generic_array::GenericArray, KeyIvInit, StreamCipher, StreamCipherSeek}, XChaCha20};

pub type Key = GenericArray<u8, U32>;
pub type IV = GenericArray<u8, U24>;

pub fn generate_key() -> Key {
	let mut key = Key::default();
	getrandom::fill(&mut key).expect("random data should be available");
	key
}

pub fn generate_iv() -> IV {
	let mut iv = IV::default();
	getrandom::fill(&mut iv).expect("random data should be available");
	iv
}

pub struct EncryptWriter<W: Write> {
	inner: W,
	cipher: XChaCha20,
	buffer: Vec<u8>,
}

impl<W: Write + Seek> EncryptWriter<W> {
	pub fn new(inner: W, key: Key, iv: IV) -> Self {
		let cipher = XChaCha20::new(&key, &iv);
		
		Self {
			inner,
			cipher,
			buffer: Vec::new(),
		}
	}
}

impl<W: Write + Seek> Write for EncryptWriter<W> {
	fn write(&mut self, in_buf: &[u8]) -> io::Result<usize> {
		self.buffer.resize(std::cmp::max(in_buf.len(), self.buffer.len()), 0);
		let out_buf = &mut self.buffer[..in_buf.len()];
		
		// TODO handle error
		self.cipher.apply_keystream_b2b(in_buf, out_buf).unwrap();
		self.inner.write_all(out_buf)?;
		Ok(in_buf.len())
	}
	
	fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
		let _ = self.write(buf)?;
		Ok(())
	}
	
	fn flush(&mut self) -> io::Result<()> {
		self.inner.flush()
	}
}

impl<W: Write + Seek> Seek for EncryptWriter<W> {
	fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
		unimplemented!("Seek is just implemented to use stream_position")
	}
	
	fn stream_position(&mut self) -> io::Result<u64> {
		self.inner.stream_position()
	}
}

pub struct DecryptReader<R: Read + Seek> {
	inner: R,
	inner_start_pos: u64,
	cipher: XChaCha20,
}

impl<R: Read + Seek> DecryptReader<R> {
	pub fn new(mut inner: R, key: Key, iv: IV) -> io::Result<Self> {
		let inner_start_pos = inner.stream_position()?;
		let cipher = XChaCha20::new(&key, &iv);
		
		Ok(Self {
			inner,
			inner_start_pos,
			cipher,
		})
	}
}

impl<R: Read + Seek> Read for DecryptReader<R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let bytes_read = self.inner.read(buf)?;
		self.cipher.apply_keystream(&mut buf[..bytes_read]);
		Ok(bytes_read)
	}
	
	fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
		self.inner.read_exact(buf)?;
		self.cipher.apply_keystream(buf);
		Ok(())
	}
}

impl<R: Read + Seek> Seek for DecryptReader<R> {
	fn seek(&mut self, seek_from: SeekFrom) -> io::Result<u64> {
		let mut new_pos = match seek_from {
			SeekFrom::Start(pos) => self.inner.seek(SeekFrom::Start(self.inner_start_pos + pos))?,
			seek_from => self.inner.seek(seek_from)?,
		};
		
		if new_pos < self.inner_start_pos {
			new_pos = self.inner.seek(SeekFrom::Start(self.inner_start_pos))?;
			assert!(new_pos >= self.inner_start_pos);
		}
		
		self.cipher.try_seek(new_pos - self.inner_start_pos)
			.map_err(io::Error::other)?;
		
		Ok(new_pos - self.inner_start_pos)
	}
	
	fn stream_position(&mut self) -> io::Result<u64> {
		Ok(self.inner.stream_position()? - self.inner_start_pos)
	}
}

use std::io::{Read, Write};

use chacha20::{cipher::{consts::{U24, U32}, generic_array::GenericArray, KeyIvInit, StreamCipher}, XChaCha20};
use getrandom::getrandom;

pub type Key = GenericArray<u8, U32>;
pub type IV = GenericArray<u8, U24>;

pub fn generate_key() -> Key {
	let mut key = Key::default();
	getrandom(&mut key).expect("random data should be available");
	key
}

pub fn generate_iv() -> IV {
	let mut iv = IV::default();
	getrandom(&mut iv).expect("random data should be available");
	iv
}

pub struct EncryptWriter<W: Write> {
	inner: W,
	cipher: XChaCha20,
	buffer: Vec<u8>,
}

impl<W: Write> EncryptWriter<W> {
	pub fn new(inner: W, key: Key, iv: IV) -> Self {
		let cipher = XChaCha20::new(&key, &iv);
		
		Self {
			inner,
			cipher,
			buffer: Vec::new(),
		}
	}
}

impl<W: Write> Write for EncryptWriter<W> {
	fn write(&mut self, in_buf: &[u8]) -> std::io::Result<usize> {
		self.buffer.resize(std::cmp::max(in_buf.len(), self.buffer.len()), 0);
		let out_buf = &mut self.buffer[..in_buf.len()];
		
		// TODO handle error
		self.cipher.apply_keystream_b2b(&in_buf, out_buf).unwrap();
		self.inner.write_all(out_buf)?;
		Ok(in_buf.len())
	}
	
	fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
		#[allow(clippy::unused_io_amount)]
		self.write(buf)?;
		Ok(())
	}
	
	fn flush(&mut self) -> std::io::Result<()> {
		self.inner.flush()
	}
}

pub struct DecryptReader<R: Read> {
	inner: R,
	cipher: XChaCha20,
}

impl<R: Read> DecryptReader<R> {
	pub fn new(inner: R, key: Key, iv: IV) -> Self {
		let cipher = XChaCha20::new(&key, &iv);
		
		Self {
			inner,
			cipher,
		}
	}
}

impl<R: Read> Read for DecryptReader<R> {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		let bytes_read = self.inner.read(buf)?;
		self.cipher.apply_keystream(&mut buf[..bytes_read]);
		Ok(bytes_read)
	}
}

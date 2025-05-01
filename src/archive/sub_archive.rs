use std::{io::{self, Read}, ops::ControlFlow};

use xz2::read::XzDecoder;

use crate::{crypto::{DecryptReader, IV}, Key, BKY_HEADER};

pub struct SubArchive<R: Read> {
	decrypter: DecryptReader<R>,
	source_groups: Vec<SourceGroup>,
	is_single_source: bool,
}

pub struct SourceGroup {
	pub id: String,
	pub size: u64,
	pub flags: u32,
}

impl<R: Read> SubArchive<R> {
	pub fn new(mut reader: R, key: Key) -> Result<Self, io::Error> {
		let mut header = [0u8; BKY_HEADER.len()];
		reader.read_exact(&mut header)?;
		
		if header != BKY_HEADER {
			panic!("Not a backy archive");
		}
		
		let mut iv = IV::default();
		reader.read_exact(&mut iv)?;
		let mut decrypter = DecryptReader::new(reader, key, iv);
		
		let mut buf32 = [0u8; size_of::<u32>()];
		let mut buf64 = [0u8; size_of::<u64>()];
		
		decrypter.read_exact(&mut buf32)?;
		let flags = u32::from_le_bytes(buf32);
		let is_single_source = flags & 1 != 0;
		
		decrypter.read_exact(&mut buf32)?;
		let groups_len = u32::from_le_bytes(buf32);
		
		let mut source_groups = Vec::with_capacity(groups_len as usize);
		
		// header
		for _ in 0..groups_len {
			decrypter.read_exact(&mut buf32)?;
			let id_len = u32::from_le_bytes(buf32);
			let mut id_buf = vec![0; id_len as usize];
			decrypter.read_exact(&mut id_buf[..])?;
			let id = String::from_utf8(id_buf).unwrap(); // TODO: return custom error
			
			decrypter.read_exact(&mut buf64)?;
			let size = u64::from_le_bytes(buf64);
			
			decrypter.read_exact(&mut buf32)?;
			let flags = u32::from_le_bytes(buf32);
			
			source_groups.push(SourceGroup {
				id,
				size,
				flags,
			});
		}
		
		Ok(Self {
			decrypter,
			source_groups,
			is_single_source,
		})
	}
	
	pub fn is_single_source(&self) -> bool {
		self.is_single_source
	}
	
	pub fn sources(&self) -> impl Iterator<Item = &str> {
		self.source_groups.iter().map(|source_group| source_group.id.as_str())
	}
	
	pub fn for_each_tar<F>(self, mut callback: F) -> Result<(), io::Error>
	where
		F: FnMut(&SourceGroup, &mut tar::Archive<io::Take<&mut XzDecoder<DecryptReader<R>>>>) -> Result<ControlFlow<()>, io::Error>,
	{
		let mut decoder = XzDecoder::new(self.decrypter);
		for source_group in self.source_groups {
			let read = (&mut decoder).take(source_group.size);
			let mut archive = tar::Archive::new(read);
			
			if callback(&source_group, &mut archive)?.is_break() {
				break;
			};
			
			read_to_end(archive.into_inner())?;
		}
		
		Ok(())
	}
}

fn read_to_end(mut read: impl Read) -> Result<(), io::Error> {
	let mut buf = [0u8; 1024];
	
	loop {
		match read.read(&mut buf) {
			Ok(0) => return Ok(()),
			Ok(_) => (),
			Err(err) => return Err(err),
		}
	}
}

use crate::BitStream;


#[derive(Clone)]
pub struct JpegFormat {
	pub sizex: u16,
	pub sizey: u16,
	pub huff_deep_ac: Vec<[u8;16]>,
	pub huff_symbol_ac : Vec<[Vec<u8>;16]>,
	pub huff_deep_dc: Vec<[u8;16]>,
	pub huff_symbol_dc : Vec<[Vec<u8>;16]>,
	pub data: BitStream,
	pub comp: u8,
	pub x_sampling: u8,
	pub y_sampling: u8,
	pub quant_table: Vec<Vec<u8>>,
	pub bsizex: u16,
	pub bsizey: u16,
}

fn marking(stream: &mut Vec<u8>, mark: u8) {
	stream.push(0xFF);
	stream.push(mark);
}

fn mark_size(stream: &mut Vec<u8>, size: u16) {
	stream.push((size/256) as u8);
	stream.push((size%256) as u8);
}

impl JpegFormat {
	pub fn new(sizex:u16, sizey: u16, comp: u8, x_sampling: u8, y_sampling: u8) -> Self {
		Self {  sizex,
				sizey,
				x_sampling,
				y_sampling,
				comp,
				huff_deep_ac: vec![],
				huff_symbol_ac: vec![],
				huff_deep_dc: vec![],
				huff_symbol_dc: vec![],
				data: BitStream { stream:vec![] },
				quant_table: vec![],
				bsizex: (sizex+7)/8,
				bsizey: (sizey+7)/8,
			}
	}

	fn jfif_bytestream(&self, stream: &mut Vec<u8>) {
		marking(stream, 0xE0);
		let size: u16 = 16;
		mark_size(stream, size);

		"JFIF".as_bytes().iter().for_each(|c| stream.push(*c));
		stream.push(0x00);

		// lazy inmplementation for now, to improve
		stream.push(0x01);
		stream.push(0x01);
		stream.push(0x01);
		stream.push(0x00);
		stream.push(0x48);
		stream.push(0x00);
		stream.push(0x48);
		stream.push(0x00);
		stream.push(0x00);
	}

	fn sof0_bytestream(&self, stream: &mut Vec<u8>) {
		marking(stream, 0xC0);
		mark_size(stream, 8 + 3*(self.comp as u16));

		stream.push(8);
		mark_size(stream, self.sizey);
		mark_size(stream, self.sizex);
		stream.push(self.comp);

		for i in 0..self.comp {
			stream.push(i);
			stream.push((self.y_sampling<<4) + self.x_sampling);
			if i == 0 {
				stream.push(0);
			} else {
				stream.push(1);
			}
		}
	}

	fn dqt_bytestream(&self, stream: &mut Vec<u8>) {
		self.quant_table.iter().enumerate().for_each(|(i,t)|{
			marking(stream, 0xDB);
			mark_size(stream, 67);
			stream.push(i as u8);
			t.iter().for_each(|b| stream.push(*b));
		});
	}

	fn dht_bytestream(&self, stream: &mut Vec<u8>) {
		self.huff_deep_dc.iter().enumerate().for_each(|(i,t)|{
			marking(stream, 0xC4);
			mark_size(stream, 19 + self.huff_symbol_dc[i].iter().fold(0, |acc,v|acc+v.iter().count()) as u16);
			stream.push(i as u8);
			t.iter().for_each(|b| stream.push(*b));
			self.huff_symbol_dc[i].iter().for_each(|v|v.iter().for_each(|b| stream.push(*b)));
		});
		self.huff_deep_ac.iter().enumerate().for_each(|(i,t)|{
			marking(stream, 0xC4);
			mark_size(stream, 19 + self.huff_symbol_ac[i].iter().fold(0, |acc,v|acc+v.iter().count()) as u16);
			stream.push((1<<4)+i as u8);
			t.iter().for_each(|b| stream.push(*b));
			self.huff_symbol_ac[i].iter().for_each(|v|v.iter().for_each(|b| stream.push(*b)));
		});
	}

	fn sos_bytestream(&self, stream: &mut Vec<u8>) {
		marking(stream, 0xDA);
		mark_size(stream, (6 + 2*self.comp) as u16);
		stream.push(self.comp);

		for i in 0..self.comp {
			stream.push(i);
			stream.push((i<<4) + i);
		}
		stream.push(0x00);
		stream.push(0x3F);
		stream.push(0x00);

		self.data.to_byte(true).iter().for_each(|b| stream.push(*b));
	}

	pub fn create_jpeg_bytestream(self) -> Vec<u8> {
		let mut stream: Vec<u8> = vec![];
		marking(&mut stream, 0xD8);

		self.jfif_bytestream(&mut stream);
		self.sof0_bytestream(&mut stream);
		self.dqt_bytestream(&mut stream);
		self.dht_bytestream(&mut stream);
		self.sos_bytestream(&mut stream);

		marking(&mut stream, 0xD9);
		stream
	}
}
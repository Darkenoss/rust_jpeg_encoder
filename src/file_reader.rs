use std::{fs::File, io::Bytes, str::from_utf8};

use crate::{ImageFormat::{self, PixMap}, JpegError, jpeg_format::JpegFormat};



const LF: u8 = 0x0A;
const TAB: u8 = 0x09;
const SP: u8 = 0x20;

pub struct FileReader {
	file: Bytes<File>,
}

impl FileReader {
	pub fn new(file: Bytes<File>) -> Self {
		FileReader { file }
	}

	fn next_line_as_bytes(&mut self) -> Result<Vec<u8>,JpegError> {
		let line: Result<Vec<u8>, std::io::Error> = self.file.by_ref()
			.take_while(|b| {
				if let Ok(c) = b {
					if *c == LF {
						false
					} else {
						true
					}
				} else {
					false
				}
			}).collect();
		Ok(line?)
	}

	fn next_line_as_str(&mut self) -> Result<String,JpegError> {
		let byte_line = self.next_line_as_bytes()?;
		let line = from_utf8(&byte_line)?;
		Ok(line.to_string())
	}

	fn next_separator_as_str(&mut self) -> Result<(String,u8),JpegError> {
		let mut sep = 0;
		let part_byte: Result<Vec<u8>, std::io::Error> = self.file.by_ref()
			.take_while(|b| {
				if let Ok(c) = b {
					if *c == LF || *c == TAB || *c == SP {
						sep = *c;
						false
					} else {
						true
					}
				} else {
					false
				}
			}).collect();
		let part_byte = part_byte?;
		let part = from_utf8(&part_byte)?;
		Ok((part.to_string(),sep))
	}

	fn remaining_data(&mut self) -> Result<Vec<u8>, JpegError> {
		let data: Result<Vec<u8>, std::io::Error> = self.file.by_ref().collect();
		Ok(data?)
	}

}

fn parse_pixmap_comment(lines: &mut FileReader) -> Result<String, JpegError> {
	let (mut data,mut sep) = lines.next_separator_as_str()?;

	while data[0..1] == *"#" {
		if sep != LF {
			lines.next_line_as_str()?;
		}
		(data, sep) = lines.next_separator_as_str()?;
	}
	Ok(data)
}

fn parse_pixmap_comment_line(lines: &mut FileReader) -> Result<String, JpegError> {
	let mut data = lines.next_line_as_str()?;

	while data.len()!=0 && data[0..1] == *"#" {
		data = lines.next_line_as_str()?;
	}
	Ok(data)
}

fn parse_pixmap_comment_byte(lines: &mut FileReader) -> Result<Vec<u8>, JpegError> {
	let mut data = lines.next_line_as_bytes()?;

	while data.iter().count()!= 0 && data[0] == 0x23 {
		data = lines.next_line_as_bytes()?;
	}
	Ok(data)
}

fn parse_pixmap(lines: &mut FileReader) -> Result<(JpegFormat,Vec<u8>),JpegError> {
	let mut comp = 1;
	let pix_type = parse_pixmap_comment(lines)?;
	let mut max: u8 = 0;
	let mut isbyte = false;

	match pix_type.as_str() {
		"P1" => max = 1,
		"P4" => {
			max = 1;
			isbyte = true;
		},
		"P2" => isbyte = false,
		"P5" => isbyte = true,
		"P3" => comp = 3,
		"P6" => {
			isbyte = true;
			comp = 3;
		},
		_ => return Err(JpegError::FormatError),
	}

	let sizex_str = parse_pixmap_comment(lines)?;
	let sizex = sizex_str.parse::<u16>()?;

	let sizey_str = parse_pixmap_comment(lines)?;
	let sizey = sizey_str.parse::<u16>()?;

	if max != 1 {
		let max_str= parse_pixmap_comment(lines)?;
		max = max_str.parse::<u8>()?;
	}

	let mut data: Vec<u8> = vec![];

	if isbyte {
		let mut new_line = parse_pixmap_comment_byte(lines)?;
		new_line = new_line.iter().map(|b|(*b)*(255/max)).collect();
		data.append(&mut new_line);
		data.push(LF);
		let mut remain = lines.remaining_data()?;
		data.append(&mut remain);
	} else {
		let mut new_line = parse_pixmap_comment_line(lines)?;
		let mut error = false;
		while new_line.len() != 0 {
			new_line
				.split(" ")
				.map(|s|s.parse::<u8>())
				.filter_map(|r| r.map_err(|_|error=true).ok())
				.for_each(|b|data.push(b*(255/max)));
			if error {
				return Err(JpegError::FormatError);
			}
			new_line = parse_pixmap_comment_line(lines)?;
		}
	}

	let file_format = JpegFormat::new(sizex, sizey, comp, 1, 1);

	Ok((file_format,data))
}

pub fn parse_file(img_format: ImageFormat, lines: &mut FileReader) -> Result<(JpegFormat,Vec<u8>),JpegError> {
	match img_format {
		PixMap => parse_pixmap(lines)
	}
}
use std::env;
use std::f64::consts::PI;
use std::fmt::Display;
use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::num::ParseIntError;
use num_traits::Num;
use num_traits::ToPrimitive;
use std::str::Utf8Error;
use crate::file_reader::FileReader;
use crate::file_reader::parse_file;
use crate::huffman::HuffmanTree;
use crate::huffman::encode_single;
use crate::huffman::perform_huffman_no_encoding;
use crate::jpeg_format::JpegFormat;

mod huffman;
mod jpeg_format;
mod file_reader;

#[derive(Debug)]
enum JpegError {
	FileError,
	FormatError,
	HelpError,
	ReadError,
	OutOfRange,
	HuffmanError,
	ParseError,
}

impl From<std::io::Error> for JpegError {
	fn from(_: std::io::Error) -> Self {
		JpegError::FileError
	}
}

impl From<ParseIntError> for JpegError {
	fn from(_: ParseIntError) -> Self {
		JpegError::ParseError
	}
}

impl From<Utf8Error> for JpegError {
	fn from(_: Utf8Error) -> Self {
		JpegError::ParseError
	}
}

enum ImageFormat {
	PixMap,
}

fn c_function(u: u8) -> f64 {
	if u == 0 {
		1.0/((2.0_f64).sqrt())
	} else {
		1.0
	}
}

fn dct_cos(count: u8, i: u8) -> f64 {
	f64::cos((2*(count/8)+1) as f64 *(i/8) as f64 *PI/16.0)
	* f64::cos((2*(count%8)+1) as f64 *(i%8) as f64 *PI/16.0)
}


#[derive(Clone)]
struct BitStream {
	stream: Vec<bool>,
}

struct Bloc<T: num_traits::NumCast + Copy + Display + Num> {
	data: [T; 64],
}

impl<T: num_traits::NumCast + Copy + Display + Num> Bloc<T> {
	fn do_dct(&mut self) -> Bloc<i16>{
		let mut res:Bloc<i16>= Bloc { data: [0;64]};
		for i in 0..64 {
			let mut count = 0;
			res.data[i] = (self.data
				.into_iter()
				.fold(0.0_f64, |acc, x|{
					let res = acc + (x.to_f64().unwrap()-128.0)*dct_cos(count,i as u8);
					count+=1;
					res})
				* 0.25 * c_function(i as u8/8) * c_function(i as u8%8)) as i16;
		};
		res
	}

	fn do_quant(&mut self, quant: &Bloc<u8>) -> Bloc<i16>{
		let mut count = 0;
		let mut res: Bloc<i16> = Bloc { data: [0;64] };

		self.data
			.iter()
			.for_each(|x |{
				count +=1;
				res.data[count-1] = x.to_i16().unwrap()/quant.data[count-1].to_i16().unwrap()});
		res
	}

	fn do_zigzag(&self) -> Vec<T>{
		let zig = [0,1,8,16,9,2,3,10,17,24,32,25,18,11,4,5,12,19,26,33,40,48,41,34,27,20,13,6,7,14,21,28,35,42,49,56,57,50,43,36,29,22,15,23,30,37,44,51,58,59,52,45,38,31,39,46,53,60,61,54,47,55,62,63];
		let mut res: Vec<T> = vec![];
		let mut temp:Vec<T> = vec![];
		for i in 0..64 {
			let val = self.data[zig[i]];
			if val.to_i16().unwrap() == 0 {
				temp.push(val);
			} else if temp.is_empty() {
				res.push(val);
			} else {
				temp.push(val);
				res = [res, temp].concat();
				temp = vec![];
			};
		};
		res
	}

	fn display(&self) {
		for i in 0..8 {
			for u in 0..8 {
				print!("{} ",self.data[i*8+u])
			}
			println!();
		}
	}
}

impl BitStream {
	fn new(mut val: u8, size: usize) -> Self {
		let mut stream = vec![];
		let mut count = 0;
		while val!=0 && count<size {
			stream.push((val&1) != 0);
			val >>=1;
			count += 1;
		}
		for _ in 0..(size-count) {stream.push(false);}
		stream.reverse();
		BitStream {stream}
	}

	fn _add(&mut self, mut nb: u16) {
		while nb != 0 {
			self.stream.push((nb & 1) != 0);
			nb >>= 1;
		}
	}

	fn push_single(&mut self, val: bool){
		self.stream.push(val);
	}

	fn add_stream(&mut self, stream: &BitStream) {
		stream.stream.iter().for_each(|b| self.stream.push(*b));
	}

	fn display(&self) {
		self.stream.iter().for_each(|b|print!("{} ",b));
		println!("");
	}

	fn to_byte(&self, stuffing: bool) -> Vec<u8> {
		let mut res = vec![];
		let mut nb: u8 = 0;
		let mut count:u8 = 0;
		self.stream.iter().for_each(|b|{
			nb<<=1;
			if *b {
				nb+=1;
			}
			count+=1;
			if count==8 {
				res.push(nb);
				if stuffing && nb==0xFF {
					res.push(0x00);
				}
				count = 0;
				nb = 0;
			}
		});

		if count!=0 {
			nb <<= 8-count;
			res.push(nb);
		}
		res
	}
}

fn help(cmd: &String) -> JpegError {
	println!("usage: {cmd} -f 'path_to_image'");
	JpegError::HelpError
}

fn parse_format(name: &String) -> Result<ImageFormat,JpegError> {
	let form = &name[name.len()-3..];
	match form {
		"pbm" => Ok(ImageFormat::PixMap),
		"pgm" => Ok(ImageFormat::PixMap),
		"ppm" => Ok(ImageFormat::PixMap),
		_ => Err(JpegError::FormatError),
	}
}

fn parse_cmd(args: &Vec<String>) -> Result<(FileReader,ImageFormat), JpegError> {
	match args.len() {
		3 => {
			if args[1].eq("-f") {
				let img_format = parse_format(&args[2])?;
				let file= fs::File::open(&args[2])?.bytes();
				Ok((FileReader::new(file),img_format))
			} else {
				Err(help(&args[0]))
			}
		},
		_ => {
			Err(help(&args[0]))
		},
	}
}

fn parse_bloc_line(lines: Vec<u8>, file_format: &JpegFormat) -> Result<Vec<Bloc<u8>>, JpegError> {
	let mut blocs: Vec<Bloc<u8>> = Vec::new();
	for _ in  0..(file_format.bsizex*file_format.bsizey) {
		blocs.push(Bloc { data: [0;64] });
	};

	let mut count = 0;
	for y in 0..file_format.sizey {
		for x in 0..file_format.sizex {
			blocs[(x/8 +(y/8)*file_format.bsizex) as usize].data[(x%8 + (y%8)*8) as usize] = lines[count];
			count+=1;
		}
	}

	Ok(blocs)
}

fn read_quant(s: String) -> Result<Bloc<u8>,JpegError> {
	let Ok(file) = fs::File::open(s) else {
		return Err(JpegError::FileError);
	};
	let mut lines = BufReader::new(file).lines();
	let mut quant:Bloc<u8> = Bloc { data: [0;64] };
	for l in 0..8 {
		let Some(Ok(line)) = lines.next() else {
			return Err(JpegError::ReadError);
		};
		let mut count = 0;
		let mut errors = false;
		line
			.split(" ")
			.map(|s| s.parse::<u8>())
			.filter_map(|r| r.map_err(|_|errors=true).ok())
			.for_each(|x| {
				quant.data[8*l + count%8] = x;
				count+=1;});
		if errors {
			return Err(JpegError::ReadError);
		}
	};

	Ok(quant)
}

fn magnitude_code(mut nb: i16, max: i16) -> Result<(u8, BitStream),JpegError> {
	let mut temp = 0;
	let mut stream = BitStream {stream: vec![]};
	if nb > max || nb < -max {
		return Err(JpegError::OutOfRange);
	}
	while nb.abs() >= 1<<temp {
		temp+=1;
	};
	if nb<0 {
		nb += (1<<temp) -1;
	};
	for i in 0..temp {
		stream.stream.push((nb>>(temp-i-1))&1 != 0);
	}
	Ok((temp,stream))

}

fn single_mcu_encoding(mcu: &Vec<i16>, last_dc:i16) -> Result<Vec<(u8,BitStream)>,JpegError> {
	let mut output:Vec<(u8,BitStream)> = vec![];
	let mut count =0;
	let mut last = 0;
	for byte in mcu {
		if count == 0 {
			output.push(magnitude_code(byte-last_dc, 2047)?);
			count+=1;
			continue;
		};
		if *byte == 0 {
			last+=1;
			if last == 16 {
				output.push((0xF0,BitStream{stream: vec![]}));
				last = 0;
			}
		} else {
			let (mag,stream) = magnitude_code(*byte, 1023)?;
			output.push((((last<<4)+mag),stream));
			last = 0;
		};
		count+=1;
	};
	if count < 64 {
		output.push((0x00,BitStream{stream: vec![]}));
	}
	Ok(output)
}

fn mcu_encoding(data: Vec<Vec<i16>>) -> Result<Vec<Vec<(u8,BitStream)>>,JpegError> {
	let mut last_dc = 0;
	data.iter()
		.map(|mcu| {
			let res = single_mcu_encoding(mcu, last_dc);
			last_dc = mcu[0];
			res
		}).collect()
}

fn huffman_separation(datas: &Vec<Vec<(u8,BitStream)>>) -> (Vec<u8>,Vec<u8>) {
	let mut dc = vec![];
	let mut ac = vec![];
	datas.iter().for_each(|v|{
		v.iter().enumerate().for_each(|(c,d)|{
			if c == 0 {
				dc.push(d.0);
			} else {
				ac.push(d.0);
			}
		});
	});
	(dc,ac)
}

fn encode_data(data: &Vec<Vec<(u8, BitStream)>>,huff_dc: Vec<HuffmanTree>, huff_ac: Vec<HuffmanTree>) -> Result<BitStream,JpegError> {
	let mut res = BitStream{stream: vec![]};
	let err = false;
	data
		.iter()
		.for_each(|vec| {
			vec
				.iter()
				.enumerate()
				.for_each(|(c,val)| {
					let temp;
					if c == 0 {
						temp = encode_single(val.0, &huff_dc).unwrap();
					} else {
						temp = encode_single(val.0, &huff_ac).unwrap();
					};
					res.add_stream(&temp);
					res.add_stream(&val.1);
				})
		});
	if err {
		Err(JpegError::HuffmanError)
	} else {
		Ok(res)
	}
}

fn main() -> Result<(), JpegError>{

	let args: Vec<String> = env::args().collect();
	let (mut lines, img_format) = parse_cmd(&args)?;
	let (mut jpeg_info,data) = parse_file(img_format, &mut lines)?;

	println!("{:?}",data);
	let mut blocs = parse_bloc_line(data, &jpeg_info)?;

	println!("{:x?}",blocs[0].data);
	let mut blocs:Vec<Bloc<i16>> = blocs.iter_mut().map(|b| b.do_dct()).collect();
	println!("{:x?}",blocs[0].data);
	blocs[0].display();
	let quant = read_quant("quant/table".to_string())?;
	quant.display();
	let mut blocs:Vec<Bloc<i16>> = blocs.iter_mut().map(|b| b.do_quant(&quant)).collect();
	blocs[0].display();
	let blocs:Vec<Vec<i16>> = blocs.iter_mut().map(|b| b.do_zigzag()).collect();
	blocs[0].iter().for_each(|x|print!("{x} "));

	let (mag, val) = magnitude_code(78,2047)?;
	println!("{}",mag);
	val.display();

	let datas = mcu_encoding(blocs)?;
	let (dc,ac) = huffman_separation(&datas);

	println!("{:x?}\n{:x?}",dc,ac);

	let (huff_dc,deep_dc,symbol_dc) = perform_huffman_no_encoding(dc)?;
	let (huff_ac,deep_ac,symbol_ac) = perform_huffman_no_encoding(ac)?;

	println!("{:x?}",symbol_ac);

	let temp = huff_ac.clone();

	for tree in temp {
		println!("{:x},{},{:?}",tree.val, tree.freq, tree.stream.unwrap().stream);
	}

	let coded_data = encode_data(&datas, huff_dc, huff_ac)?;

	jpeg_info.data = coded_data;
	jpeg_info.huff_deep_dc.push(deep_dc);
	jpeg_info.huff_deep_ac.push(deep_ac);
	jpeg_info.huff_symbol_dc.push(symbol_dc);
	jpeg_info.huff_symbol_ac.push(symbol_ac);
	jpeg_info.quant_table.push(quant.do_zigzag());

	let bytestream = jpeg_info.create_jpeg_bytestream();

	println!("{:x?}",bytestream);

	let mut output = args[2][..&args[2].len()-3].to_string();
	output.push_str("jpg");

	let mut f = File::create(output)?;

	f.write_all(&bytestream.to_vec())?;

	Ok(())
}
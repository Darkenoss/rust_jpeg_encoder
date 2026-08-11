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
use crate::ImageFormat::PixMap;
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

struct CmdArgs {
	format: ImageFormat,
	debug: bool,
	quant: String,
}

impl CmdArgs {
	fn new() -> Self {
		CmdArgs { format:PixMap, debug: false, quant: "quant/one".to_string() }
	}
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

#[derive(Clone)]
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
		if res.len() == 0 {
			res.push(self.data[0]);
		}
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

fn parse_cmd(args: &Vec<String>) -> Result<(FileReader,CmdArgs), JpegError> {
	let mut args = args.iter();
	let Some(command) = args.next() else {
		return Err(help(&"".to_string()));
	};

	let mut cmd_res = CmdArgs::new();
	let mut file = String::from("");

	while let Some(cmd)= args.next() {
		match cmd.as_str() {
			"-f" => {
				if let Some(path) = args.next() {
					cmd_res.format = parse_format(path)?;
					file = path.clone();
				} else {
					return Err(help(&command))
				}
			}
			"-d" => {
				cmd_res.debug = true;
			}
			"-q" => {
				if let Some(quant) = args.next() {
					cmd_res.quant = quant.clone();
				} else {
					return Err(help(&command))
				}
			}
			_ => return Err(help(&command))
		};
	};

	if file == "" {
		return Err(help(&command));
	} else {
		let file= fs::File::open(file)?.bytes();
		Ok((FileReader::new(file),cmd_res))
	}
}

fn parse_bloc_line(lines: Vec<u8>, file_format: &JpegFormat) -> Result<Vec<Bloc<u8>>, JpegError> {
	let mut blocs: Vec<Bloc<u8>> = Vec::new();
	for _ in  0..(file_format.bsizex*file_format.bsizey*(file_format.comp as u16)) {
		blocs.push(Bloc { data: [0;64] });
	};

	let mut count = 0;
	for y in 0..file_format.sizey {
		for x in 0..file_format.sizex {
			for c in 0..file_format.comp as u16 {
				blocs[((x/8 +(y/8)*file_format.bsizex)*file_format.comp as u16 + c) as usize]
					.data[(x%8 + (y%8)*8) as usize] = lines[count];
				count+=1;
			}
		}
	}

	Ok(blocs)
}

fn rgb_to_ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = r as f64;
    let g = g as f64;
    let b = b as f64;

    (
        (0.229 * r + 0.587 * g + 0.114 * b).round().clamp(0.0, 255.0) as u8,
        (-0.1687 * r - 0.3313 * g + 0.5 * b + 128.0).round().clamp(0.0, 255.0) as u8,
        (0.5 * r - 0.4187 * g - 0.0813 * b + 128.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn to_luminance(blocs: &mut Vec<Bloc<u8>>) {
	for bloc in (0..blocs.len()).step_by(3) {
		let r = blocs[bloc].clone();
		let g = blocs[bloc + 1].clone();
		let b = blocs[bloc + 2].clone();

		let mut y = Bloc { data: [0; 64] };
		let mut cb = Bloc { data: [0; 64] };
		let mut cr = Bloc { data: [0; 64] };

		for i in 0..64 {
			(y.data[i], cb.data[i], cr.data[i]) =
				rgb_to_ycbcr(r.data[i], g.data[i], b.data[i]);
		}

		blocs[bloc] = y;
		blocs[bloc + 1] = cb;
		blocs[bloc + 2] = cr;
		}
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

fn mcu_encoding(data: Vec<Vec<i16>>, jpeg_format: &JpegFormat) -> Result<Vec<Vec<(u8,BitStream)>>,JpegError> {
	let mut last_dc = [0;3];
	data.iter()
		.enumerate()
		.map(|(c,mcu)| {
			let res = single_mcu_encoding(mcu, last_dc[c%jpeg_format.comp as usize]);
			last_dc[c%jpeg_format.comp as usize] = mcu[0];
			res
		}).collect()
}

fn huffman_separation(datas: &Vec<Vec<(u8,BitStream)>>, jpeg_format: &JpegFormat) -> ([Vec<u8>;3],[Vec<u8>;3]) {
	let mut dc: [Vec<u8>; 3] = std::array::from_fn(|_| Vec::new());
	let mut ac: [Vec<u8>; 3] = std::array::from_fn(|_| Vec::new());
	datas.iter()
		.enumerate()
		.for_each(|(b,v)|{
			v.iter().enumerate().for_each(|(c,d)|{
				if c == 0 {
					dc[b%jpeg_format.comp as usize].push(d.0);
				} else {
					ac[b%jpeg_format.comp as usize].push(d.0);
				}
			});
		});
	(dc,ac)
}

fn encode_data(data: &Vec<Vec<(u8, BitStream)>>,huff_dc: [Vec<HuffmanTree>;3], huff_ac: [Vec<HuffmanTree>;3], jpeg_format: &JpegFormat) -> Result<BitStream,JpegError> {
	let mut res = BitStream{stream: vec![]};
	let err = false;
	data
		.iter()
		.enumerate()
		.for_each(|(b,vec)| {
			vec
				.iter()
				.enumerate()
				.for_each(|(c,val)| {
					let temp;
					if c == 0 {
						temp = encode_single(val.0, &huff_dc[b%jpeg_format.comp as usize]).unwrap();
					} else {
						temp = encode_single(val.0, &huff_ac[b%jpeg_format.comp as usize]).unwrap();
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
	let (mut lines, cmd_res) = parse_cmd(&args)?;
	let (mut jpeg_info,data) = parse_file(cmd_res.format, &mut lines)?;

	if cmd_res.debug {
		println!("{:?}",data);
	}


	let mut blocs = parse_bloc_line(data, &jpeg_info)?;

	if cmd_res.debug {
		println!("{:x?}",blocs[3].data);
	}

	if jpeg_info.comp == 3 {
		to_luminance(&mut blocs);
		if cmd_res.debug {
			println!("{:x?}",blocs[3].data);
		}
	}

	let mut blocs:Vec<Bloc<i16>> = blocs.iter_mut().map(|b| b.do_dct()).collect();

	if cmd_res.debug {
		println!("{:x?}",blocs[3].data);
		blocs[5].display();
		println!("{} blocs",blocs.len());
	}

	let quant = read_quant(cmd_res.quant.to_string())?;
	if cmd_res.debug {
		quant.display();
	}

	let mut blocs:Vec<Bloc<i16>> = blocs.iter_mut().map(|b| b.do_quant(&quant)).collect();
	if cmd_res.debug {
		blocs[3].display();
	}
	let blocs:Vec<Vec<i16>> = blocs.iter_mut().map(|b| b.do_zigzag()).collect();
	if cmd_res.debug {
		blocs[3].iter().for_each(|x|print!("{x} "));
		println!();
	}

	let datas = mcu_encoding(blocs, &jpeg_info)?;
	let (dc,ac) = huffman_separation(&datas,&jpeg_info);

	if cmd_res.debug {
		println!("{:x?}\n{:x?}",dc,ac);
	}
	let mut huff_dc: [Vec<HuffmanTree>;3] = Default::default();
	let mut huff_ac: [Vec<HuffmanTree>;3] = Default::default();
	let mut deep_dc: [[u8;16];3] = Default::default();
	let mut deep_ac: [[u8;16];3] = Default::default();
	let mut symbol_dc: [[Vec<u8>;16];3] = Default::default();
	let mut symbol_ac: [[Vec<u8>;16];3] = Default::default();

	for i in 0..(jpeg_info.comp as usize) {
		(huff_dc[i],deep_dc[i],symbol_dc[i]) = perform_huffman_no_encoding(&dc[i])?;
		(huff_ac[i],deep_ac[i],symbol_ac[i]) = perform_huffman_no_encoding(&ac[i])?;
	}

	if cmd_res.debug {
		println!("{:x?}",symbol_ac[0]);
		let temp = huff_dc[0].clone();
		for tree in temp {
			println!("{:x},{},{:?}",tree.val, tree.freq, tree.stream.unwrap().stream);
		}
	}

	let coded_data = encode_data(&datas, huff_dc, huff_ac, &jpeg_info)?;

	jpeg_info.data = coded_data;
	for i in 0..jpeg_info.comp as usize {
		jpeg_info.huff_deep_dc.push(deep_dc[i]);
		jpeg_info.huff_deep_ac.push(deep_ac[i]);
		jpeg_info.huff_symbol_dc.push(symbol_dc[i].clone());
		jpeg_info.huff_symbol_ac.push(symbol_ac[i].clone());
	}
	jpeg_info.quant_table.push(quant.do_zigzag());

	let bytestream = jpeg_info.create_jpeg_bytestream();

	if cmd_res.debug {
		println!("{:x?}",bytestream);
	}

	let mut output = args[2][..&args[2].len()-3].to_string();
	output.push_str("jpg");
	let print_output = output.clone();

	let mut f = File::create(output)?;

	f.write_all(&bytestream.to_vec())?;
	println!("Jpeg image sucessfully created : {}",print_output);

	Ok(())
}
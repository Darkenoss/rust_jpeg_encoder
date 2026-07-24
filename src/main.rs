use std::env;
use std::f64::consts::PI;
use std::fmt::Display;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Lines;
use num_traits::Num;
use num_traits::ToPrimitive;


#[derive(Debug)]
enum JpegError {
    FileError,
    HelpError,
    ReadError,
    OutOfRange,
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
                    let res = acc + (x.to_f64().unwrap())*dct_cos(count,i as u8);
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
    fn add(&mut self, mut nb: u16) {
        while nb != 0 {
            self.stream.push((nb & 1) != 0);
            nb >>= 1;
        }
    }

    fn add_stream(&mut self, stream: BitStream) {
        stream.stream.iter().for_each(|b| self.stream.push(*b));
    }

    fn display(&self) {
        self.stream.iter().for_each(|b|print!("{} ",b));
        println!("");
    }

    fn to_byte(&self) -> Vec<u8> {
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

fn parse_cmd(args: Vec<String>) -> Result<Lines<BufReader<fs::File>>, JpegError> {
    match args.len() {
        3 => {
            if args[1].eq("-f") {
                let Ok(file) = fs::File::open(&args[2]) else {
                    return Err(JpegError::FileError);
                };
                Ok(BufReader::new(file).lines())
            } else {
                Err(help(&args[0]))
            }
        },
        _ => {
            Err(help(&args[0]))
        },
    }
}

fn parse_bloc_line(mut lines: Lines<BufReader<fs::File>>, sizex: u32) -> Result<Vec<Bloc<u8>>, JpegError> {
    let mut blocs: Vec<Bloc<u8>> = Vec::new();
    for _ in  0..sizex { blocs.push(Bloc { data: [0;64] }); };

    for bline in 0..8 {
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
                blocs[count/8].data[8*bline + count%8] = x;
                count+=1;});
        if errors {
            return Err(JpegError::ReadError);
        }
    };

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

fn mcu_encoding(mcu: &Vec<i16>, last_dc:i16) -> Result<Vec<(u8,BitStream)>,JpegError> {
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

fn main() -> Result<(), JpegError>{

    let args: Vec<String> = env::args().collect();
    let lines = parse_cmd(args)?;
    let mut blocs = parse_bloc_line(lines, 1)?;
    let mut blocs:Vec<Bloc<i16>> = blocs.iter_mut().map(|b| b.do_dct()).collect();
    blocs[0].display();
    let quant = read_quant("quant/table".to_string())?;
    quant.display();
    let mut blocs:Vec<Bloc<i16>> = blocs.iter_mut().map(|b| b.do_quant(&quant)).collect();
    blocs[0].display();
    let blocs:Vec<Vec<i16>> = blocs.iter_mut().map(|b| b.do_zigzag()).collect();
    blocs[0].iter().for_each(|x|print!("{x} "));

    let mut stream = BitStream {stream : vec![]};
    stream.add(9);
    stream.add(1);
    stream.add(1);
    stream.display();

    let test = stream.to_byte();
    println!("{:08b}",test[0]);

    let (mag, val) = magnitude_code(-6,2047)?;
    println!("{}",mag);
    val.display();
    stream.add_stream(val);
    stream.display();

    let enc = mcu_encoding(&blocs[0], 0)?;
    for vec in enc {
        print!("{:02x} ",vec.0);
        vec.1.display();
    }

    Ok(())
}
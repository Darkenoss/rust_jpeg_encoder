use std::env;
use std::f64::consts::PI;
use std::fmt::Display;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Lines;
use std::ptr::read;

use num_traits::ToPrimitive;


#[derive(Debug)]
enum JpegError {
    FileError,
    HelpError,
    ReadError,
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

struct Bloc<T: num_traits::NumCast + Copy + Display> {
    data: [T; 64],
}

impl<T: num_traits::NumCast + Copy + Display> Bloc<T> {
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

    fn display(&self) {
        for i in 0..8 {
            for u in 0..8 {
                print!("{} ",self.data[i*8+u])
            }
            println!();
        }
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

fn main() -> Result<(), JpegError>{

    let args: Vec<String> = env::args().collect();
    let lines = parse_cmd(args)?;
    let mut blocs = parse_bloc_line(lines, 1)?;
    let mut blocs:Vec<Bloc<i16>> = blocs.iter_mut().map(|b| b.do_dct()).collect();
    blocs[0].display();
    let quant = read_quant("quant/table".to_string())?;
    quant.display();
    let blocs:Vec<Bloc<i16>> = blocs.iter_mut().map(|b| b.do_quant(&quant)).collect();
    blocs[0].display();

    Ok(())
}
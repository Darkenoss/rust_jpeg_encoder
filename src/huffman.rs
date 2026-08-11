use std::{collections::HashMap};

use crate::{BitStream, JpegError::{self, HuffmanError}};

#[derive(Clone)]
pub struct HuffmanTree {
	zero: Option<Box<HuffmanTree>>,
	one: Option<Box<HuffmanTree>>,
	isleaf: bool,
	pub val: u8,
	pub freq: u64,
	pub stream: Option<BitStream>,
	weight: u16,
}

fn create_leaf(val: u8, freq: u64) -> HuffmanTree {
	HuffmanTree { zero:None, one: None, isleaf: true, freq, val , stream: None, weight: 1}
}

fn generate_freq_leafs(val: &Vec<u8>) -> Vec<HuffmanTree> {
	let mut freqs:HashMap<u8, u64> = HashMap::new();
	val.iter().for_each(|v|{
		let freq = freqs.entry(*v).or_insert(0);
		*freq +=1;
	});

	let mut res:Vec<HuffmanTree> = freqs
		.iter()
		.map(|(&value,&frequence)| create_leaf(value, frequence))
		.collect();
	res.sort_by_key(|h| -(h.freq as i64));
	res.push(HuffmanTree { zero: None, one: None, isleaf: true, val: 0, freq: 0, stream: None, weight: 257 });
	res
}

fn insert_sorted_huffman(nodes: &mut Vec<HuffmanTree>, elem: HuffmanTree) {
	let mut index = 0;
	let size = nodes.iter().count();
	while index < size && elem.freq < nodes[index].freq{
		index+=1;
	};
	nodes.insert(index, elem);
}

fn construct_huff_table(mut nodes: Vec<HuffmanTree>) -> Result<HuffmanTree,JpegError> {
	while nodes.iter().count() != 1 {
		let Some(mut left) = nodes.pop() else {
			return Err(JpegError::HuffmanError)
		};
		let Some(mut right) = nodes.pop() else {
			return Err(JpegError::HuffmanError);
		};
		if left.weight > right.weight {
			let temp = left;
			left = right;
			right = temp;
		}
		let weight = left.weight+ right.weight;
		let freq = left.freq+right.freq;
		insert_sorted_huffman(&mut nodes, HuffmanTree { zero:Some(Box::new(left)),
														one: Some(Box::new(right)),
														isleaf: false,
														val: 0,
														freq,
														stream: None,
														weight});
	};
	Ok(nodes[0].clone())
}

fn prepare_jpeg_encoding(table: &mut HuffmanTree, deep: usize, leafs: &mut [Vec<HuffmanTree>;16], jpeg_deep: &mut [u8;16], jpeg_symbol: &mut [Vec<u8>;16]){
	if table.isleaf {
		if table.freq == 0 {
			return;
		}
		leafs[deep-1].push(table.clone());
		jpeg_deep[deep-1]+= 1;
		jpeg_symbol[deep-1].push(table.val);
	} else {
		if let Some(ref mut zero) = table.zero {
			prepare_jpeg_encoding(zero, deep +1, leafs, jpeg_deep, jpeg_symbol);
		}
		if let Some(ref mut one) = table.one {
			prepare_jpeg_encoding(one, deep +1, leafs, jpeg_deep,jpeg_symbol);
		}
	}
}

fn _encode_to_bistream(values: Vec<u8>, leafs: Vec<HuffmanTree>) -> Vec<BitStream>{
	let mut stream = vec![];
	values.iter().for_each(|s|{
		if let Some(code) = leafs.iter().find(|t| t.val==*s) {
			if let Some(code_stream) = &code.stream {
				stream.push(code_stream.clone());
			};
		};
	});
	stream
}

pub fn encode_single(val: u8, leafs: &Vec<HuffmanTree>) -> Result<BitStream,JpegError> {
	let Some(code) = leafs.iter().find(|t| t.val==val) else {
		return Err(HuffmanError);
	};
	let Some(code_stream) = &code.stream else {
		return Err(HuffmanError);
	};
	Ok(code_stream.clone())
}

fn generate_bitstreams(sleaf: &mut [Vec<HuffmanTree>;16], jpeg_deep: [u8;16]) {
	let mut code = 0;
	let mut last_size = 0;
	for i in 0..16 {
		for sym in 0..jpeg_deep[i] {
			if i!=last_size {
				code <<= i-last_size;
				last_size = i;
			}
			sleaf[i][sym as usize].stream = Some(BitStream::new(code, i+1));
			code+=1;
		}
	}
}

pub fn _perform_huffman(values: &Vec<u8>) -> Result<(Vec<BitStream>,[u8;16],[Vec<u8>;16]),JpegError>{
	let leafs = generate_freq_leafs(&values);
	let mut table = construct_huff_table(leafs)?;
	let mut sleaf: [Vec<HuffmanTree>; 16] = Default::default();
	let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
	let mut jpeg_deep = [0;16];
	prepare_jpeg_encoding(&mut table, 0, &mut sleaf, &mut jpeg_deep, &mut jpeg_symbol);
	generate_bitstreams(&mut sleaf, jpeg_deep);
	let eleaf = sleaf.into_iter().flatten().collect();
	let stream = _encode_to_bistream(values.clone(), eleaf);
	Ok((stream,jpeg_deep,jpeg_symbol))
}

pub fn perform_huffman_no_encoding(values: &Vec<u8>) -> Result<(Vec<HuffmanTree>,[u8;16],[Vec<u8>;16]),JpegError>{
	let leafs = generate_freq_leafs(&values);
	let mut table = construct_huff_table(leafs)?;
	let mut sleaf: [Vec<HuffmanTree>; 16] = Default::default();
	let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
	let mut jpeg_deep = [0;16];
	prepare_jpeg_encoding(&mut table, 0, &mut sleaf, &mut jpeg_deep, &mut jpeg_symbol);
	generate_bitstreams(&mut sleaf, jpeg_deep);
	Ok((sleaf.into_iter().flatten().collect(),jpeg_deep,jpeg_symbol))
}


#[cfg(test)]
mod tests {

use super::*;


	#[test]
	fn test_create_leaf() {
		assert!(matches!(create_leaf(8, 11),HuffmanTree {zero:None, one: None, isleaf: true, freq: 11, val: 8, stream: None, weight: 1}))
	}

	#[test]
	fn test_generate_leaf() {
		let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
		assert_eq!(leafs[2].val,1);
		assert_eq!(leafs[2].freq,2);
		assert_eq!(leafs[1].val,2);
		assert_eq!(leafs[0].val,3);
		assert_eq!(leafs[0].freq,4);
	}

	#[test]
	fn insert_sort() {
		let mut leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
		insert_sorted_huffman(&mut leafs, HuffmanTree { zero:None, one: None, isleaf: true, val: 5, freq: 10, stream: None, weight:1});
		assert_eq!(leafs[0].val,5);
		insert_sorted_huffman(&mut leafs, HuffmanTree { zero:None, one: None, isleaf: true, val: 6, freq: 0, stream: None, weight:1});
		assert_eq!(leafs[4].val,6);
	}

	#[test]
	fn create_table() {
		let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
		let head = construct_huff_table(leafs).unwrap();
		assert!(!head.isleaf);
		let leaf0 = head.zero.unwrap();
		assert!(leaf0.isleaf);
		assert_eq!(leaf0.val,3);
		let node1 = head.one.unwrap();
		assert!(!node1.isleaf);
		let leaf10 = node1.zero.unwrap();
		assert_eq!(leaf10.val,2);
		let node11 = node1.one.unwrap();
		let leaf110 = node11.zero.unwrap();
		assert_eq!(leaf110.val,1);
		let leaf_bad = node11.one.unwrap();
		assert_eq!(leaf_bad.freq,0);
	}

	#[test]
	fn streams() {
		let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
		let mut head = construct_huff_table(leafs).unwrap();
		let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
		let mut sleaf: [Vec<HuffmanTree>; 16] = Default::default();
		prepare_jpeg_encoding(&mut head, 0, &mut sleaf, &mut[0;16], &mut jpeg_symbol);
		let leaf3 = head.zero.unwrap().stream.unwrap();
		assert_eq!(leaf3.stream.iter().count(),1);
		assert!(!leaf3.stream[0]);
		let node1 = head.one.unwrap();
		let leaf10 = node1.zero.unwrap().stream.unwrap();
		assert_eq!(leaf10.stream.iter().count(),2);
		assert!(leaf10.stream[0]);
		assert!(!leaf10.stream[1]);
		let node11 = node1.one.unwrap();
		let leaf110 = node11.zero.unwrap().stream.unwrap();
		assert_eq!(leaf110.stream.iter().count(),3);
		assert!(leaf110.stream[0]);
		assert!(leaf110.stream[1]);
		assert!(!leaf110.stream[2]);
	}

	#[test]
	fn streams_leaf() {
		let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
		let mut head = construct_huff_table(leafs).unwrap();
		let mut sleaf: [Vec<HuffmanTree>; 16] = Default::default();
		let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
		prepare_jpeg_encoding(&mut head, 0, &mut sleaf, &mut [0;16], &mut jpeg_symbol);
		if let Some(leaf3) = &sleaf[1][0].stream {
			assert_eq!(leaf3.stream.iter().count(),1);
			assert!(!leaf3.stream[0]);
		} else {
			assert!(false);
		}
		if let Some(leaf1) = &sleaf[2][0].stream {
			assert_eq!(leaf1.stream.iter().count(),2);
			assert!(leaf1.stream[0]);
			assert!(!leaf1.stream[1]);
		} else {
			assert!(false);
		}
		if let Some(leaf2) = &sleaf[3][0].stream {
			assert_eq!(leaf2.stream.iter().count(),3);
			assert!(leaf2.stream[0]);
			assert!(leaf2.stream[1]);
			assert!(!leaf2.stream[2]);
		} else {
			assert!(false);
		}
	}

	#[test]
	fn jpeg_info() {
		let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
		let mut head = construct_huff_table(leafs).unwrap();
		let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
		let mut jpeg_deep = [0;16];
		let mut sleaf: [Vec<HuffmanTree>; 16] = Default::default();
		prepare_jpeg_encoding(&mut head, 0, &mut sleaf, &mut jpeg_deep, &mut jpeg_symbol);
		assert_eq!(jpeg_deep[0],1);
		assert_eq!(jpeg_deep[1],1);
		assert_eq!(jpeg_deep[2],1);
		assert_eq!(jpeg_symbol[0][0],3);
		assert_eq!(jpeg_symbol[1][0],2);
		assert_eq!(jpeg_symbol[2][0],1);
	}

	#[test]
	fn encoding() {
		let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
		let mut head = construct_huff_table(leafs).unwrap();
		let mut sleaf: [Vec<HuffmanTree>; 16] = Default::default();
		let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
		prepare_jpeg_encoding(&mut head, 0, &mut sleaf, &mut [0;16], &mut jpeg_symbol);
		let res = _encode_to_bistream(vec![1,3,3], sleaf.into_iter().flatten().collect());
		assert_eq!(res[0].stream.iter().count(),3);
		assert!(res[0].stream[0]);
		assert!(res[0].stream[1]);
		assert!(!res[0].stream[2]);
		assert_eq!(res[1].stream.iter().count(),1);
		assert!(!res[1].stream[0]);
		assert_eq!(res[2].stream.iter().count(),1);
		assert!(!res[2].stream[0]);
	}

	#[test]
	fn total() {
		let (stream,deep,symbol) =  _perform_huffman(&vec![2,2,3,1,1,3,2,3,3]).unwrap();
		assert_eq!(stream[0].stream.iter().count(),2);
		assert!(stream[0].stream[0]);
		assert!(!stream[0].stream[1]);
		assert_eq!(deep[0],1);
		assert_eq!(deep[1],1);
		assert_eq!(deep[2],1);
		assert_eq!(symbol[0][0],3);
		assert_eq!(symbol[1][0],2);
		assert_eq!(symbol[2][0],1);
	}
}
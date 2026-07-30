use std::{collections::HashMap};

use crate::{BitStream, JpegError::{self, HuffmanError}};

#[derive(Clone)]
pub struct HuffmanTree {
    zero: Option<Box<HuffmanTree>>,
    one: Option<Box<HuffmanTree>>,
    isleaf: bool,
    val: u8,
    freq: u64,
    stream: Option<BitStream>,
}

fn create_leaf(val: u8, freq: u64) -> HuffmanTree {
    HuffmanTree { zero:None, one: None, isleaf: true, freq, val , stream: None}
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
        let Some(left) = nodes.pop() else {
            return Err(JpegError::HuffmanError)
        };
        let Some(right) = nodes.pop() else {
            return Err(JpegError::HuffmanError);
        };
        let freq = left.freq+right.freq;
        insert_sorted_huffman(&mut nodes, HuffmanTree { zero:Some(Box::new(left)),
                                                        one: Some(Box::new(right)),
                                                        isleaf: false,
                                                        val: 0,
                                                        freq,
                                                        stream: None});
    };
    Ok(nodes[0].clone())
}

fn prepare_jpeg_encoding(table: &mut HuffmanTree, stream: &mut BitStream, leafs: &mut Vec<HuffmanTree>, jpeg_deep: &mut [u8;16], jpeg_symbol: &mut [Vec<u8>;16]){
    if table.isleaf {
        let mut count = stream.stream.iter().count();
        if count > 0 {
            count-=1;
            table.stream = Some(stream.clone());
        } else {
            table.stream = Some(BitStream { stream: vec![false] })
        }
        leafs.push(table.clone());
        jpeg_deep[count]+= 1;
        jpeg_symbol[count].push(table.val);
    } else {
        if let Some(ref mut zero) = table.zero {
            stream.push_single(false);
            prepare_jpeg_encoding(zero, stream, leafs, jpeg_deep, jpeg_symbol);
            stream.stream.pop();
        }
        if let Some(ref mut one) = table.one {
            stream.push_single(true);
            prepare_jpeg_encoding(one, stream, leafs, jpeg_deep,jpeg_symbol);
            stream.stream.pop();
        }
    }
}

fn encode_to_bistream(values: Vec<u8>, leafs: Vec<HuffmanTree>) -> Vec<BitStream>{
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

pub fn perform_huffman(values: Vec<u8>) -> Result<(Vec<BitStream>,[u8;16],[Vec<u8>;16]),JpegError>{
    let leafs = generate_freq_leafs(&values);
    let mut table = construct_huff_table(leafs)?;
    let mut sleaf = vec![];
    let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
    let mut jpeg_deep = [0;16];
    prepare_jpeg_encoding(&mut table, &mut BitStream { stream:vec![] }, &mut sleaf, &mut jpeg_deep, &mut jpeg_symbol);
    let stream = encode_to_bistream(values, sleaf);
    Ok((stream,jpeg_deep,jpeg_symbol))
}

pub fn perform_huffman_no_encoding(values: Vec<u8>) -> Result<(Vec<HuffmanTree>,[u8;16],[Vec<u8>;16]),JpegError>{
    let leafs = generate_freq_leafs(&values);
    let mut table = construct_huff_table(leafs)?;
    let mut sleaf = vec![];
    let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
    let mut jpeg_deep = [0;16];
    prepare_jpeg_encoding(&mut table, &mut BitStream { stream:vec![] }, &mut sleaf, &mut jpeg_deep, &mut jpeg_symbol);
    Ok((sleaf,jpeg_deep,jpeg_symbol))
}


#[cfg(test)]
mod tests {

use super::*;


    #[test]
    fn test_create_leaf() {
        assert!(matches!(create_leaf(8, 11),HuffmanTree {zero:None, one: None, isleaf: true, freq: 11, val: 8, stream: None}))
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
        insert_sorted_huffman(&mut leafs, HuffmanTree { zero:None, one: None, isleaf: true, val: 5, freq: 10, stream: None});
        assert_eq!(leafs[0].val,5);
        insert_sorted_huffman(&mut leafs, HuffmanTree { zero:None, one: None, isleaf: true, val: 6, freq: 0, stream: None });
        assert_eq!(leafs[4].val,6);
    }

    #[test]
    fn create_table() {
        let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
        let head = construct_huff_table(leafs).unwrap();
        assert!(!head.isleaf);
        let node1 = head.one.unwrap();
        assert!(!node1.isleaf);
        let leaf3 = head.zero.unwrap();
        assert!(leaf3.isleaf);
        assert_eq!(leaf3.val,3);
        let leaf1 = node1.zero.unwrap();
        assert_eq!(leaf1.val,1);
        let leaf2 = node1.one.unwrap();
        assert_eq!(leaf2.val,2);
    }

    #[test]
    fn streams() {
        let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
        let mut head = construct_huff_table(leafs).unwrap();
        let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
        prepare_jpeg_encoding(&mut head, &mut BitStream { stream:vec![] }, &mut vec![], &mut[0;16], &mut jpeg_symbol);
        let leaf3 = head.zero.unwrap().stream.unwrap();
        assert_eq!(leaf3.stream.iter().count(),1);
        assert!(!leaf3.stream[0]);
        let node1 = head.one.unwrap();
        let leaf1 = node1.zero.unwrap().stream.unwrap();
        assert_eq!(leaf1.stream.iter().count(),2);
        assert!(leaf1.stream[0]);
        assert!(!leaf1.stream[1]);
        let leaf2 = node1.one.unwrap().stream.unwrap();
        assert_eq!(leaf2.stream.iter().count(),2);
        assert!(leaf2.stream[0]);
        assert!(leaf2.stream[1]);
    }

    #[test]
    fn streams_leaf() {
        let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
        let mut head = construct_huff_table(leafs).unwrap();
        let mut sleaf = vec![];
        let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
        prepare_jpeg_encoding(&mut head, &mut BitStream { stream:vec![] }, &mut sleaf, &mut [0;16], &mut jpeg_symbol);
        if let Some(leaf3) = &sleaf[0].stream {
            assert_eq!(leaf3.stream.iter().count(),1);
            assert!(!leaf3.stream[0]);
        } else {
            assert!(false);
        }
        if let Some(leaf1) = &sleaf[1].stream {
            assert_eq!(leaf1.stream.iter().count(),2);
            assert!(leaf1.stream[0]);
            assert!(!leaf1.stream[1]);
        } else {
            assert!(false);
        }
        if let Some(leaf2) = &sleaf[2].stream {
            assert_eq!(leaf2.stream.iter().count(),2);
            assert!(leaf2.stream[0]);
            assert!(leaf2.stream[1]);
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
        prepare_jpeg_encoding(&mut head, &mut BitStream { stream:vec![] }, &mut vec![], &mut jpeg_deep, &mut jpeg_symbol);
        assert_eq!(jpeg_deep[0],1);
        assert_eq!(jpeg_deep[1],2);
        assert_eq!(jpeg_symbol[0][0],3);
        assert_eq!(jpeg_symbol[1][0],1);
        assert_eq!(jpeg_symbol[1][1],2);
    }

    #[test]
    fn encoding() {
        let leafs = generate_freq_leafs(&vec![2,2,3,1,1,3,2,3,3]);
        let mut head = construct_huff_table(leafs).unwrap();
        let mut sleaf = vec![];
        let mut jpeg_symbol: [Vec<u8>; 16] = Default::default();
        prepare_jpeg_encoding(&mut head, &mut BitStream { stream:vec![] }, &mut sleaf, &mut [0;16], &mut jpeg_symbol);
        let res = encode_to_bistream(vec![1,3,3], sleaf);
        assert_eq!(res[0].stream.iter().count(),2);
        assert!(res[0].stream[0]);
        assert!(!res[0].stream[1]);
        assert_eq!(res[1].stream.iter().count(),1);
        assert!(!res[1].stream[0]);
        assert_eq!(res[2].stream.iter().count(),1);
        assert!(!res[2].stream[0]);
    }

    #[test]
    fn total() {
        let (stream,deep,symbol) =  perform_huffman(vec![2,2,3,1,1,3,2,3,3]).unwrap();
        assert_eq!(stream[0].stream.iter().count(),2);
        assert!(stream[0].stream[0]);
        assert!(stream[0].stream[1]);
        assert_eq!(deep[0],1);
        assert_eq!(deep[1],2);
        assert_eq!(symbol[0][0],3);
        assert_eq!(symbol[1][0],1);
        assert_eq!(symbol[1][1],2);
    }
}
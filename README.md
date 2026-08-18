# rust_jpeg_encoder
A rust project about making a jpeg encoder

## Getting started
To start using it you first need to have cargo and rust installed.

Then clone the repository and use cargo

```bash
git clone https://github.com/Darkenoss/rust_jpeg_encoder.git
cd rust_jpeg_encoder
cargo build --release
/target/release/rust_jpeg_encoder -f images/flower.ppm
```

## Specifications
This implementation only encode for baseline DCT with huffman coding. Aka SOF0.

## Quantization table
The quantization table are in the quant directory of the repository.
If no quantization table are specified, no quantization is applied during the compression.
Currently the 3 quantization table available are :
- one (a table that doesn't apply any quantization)
- regular_Y (the recommend table for luminance)
- regular_Cr (the recommend table for chrominance)

See option -h for details about how to use them at runtime

## Input file
The input file format currently supported is only the portable pixmap format. The specification for portable pixmap is strictly followed and the encoder should accept all pixmap files.

## Usage
```bash
/rust_jpeg_encoder -f [target source] [option]

Options :

-d			Shows debug info
-q PATH		Use table at PATH for luminance quantization, default is one
-c PATH		Use table at PATH for chrominance, default is using the same as luminance
```

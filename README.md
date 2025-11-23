# Rust PDF Merger
This program takes a number of pdf files and merges them into one and
then numbers each of them A1 ... A5, B1 ... B3, etc.

## To build

### To build a mac version
cargo build

The executable will be in 
target/debug/rpdf_merge

### To build a windows version
cargo build --target=x86_64-pc-windows-gnu

The executable will be in 
target/debug/x86_64-pc-windows-gnu/debug/rpdf_merge.exe

## To run
cargo run -- combined_file.pdf file1.pdf file2.pdf ...  filen.pdf

or 

./target/debug/rpdf_merge combined_file.pdf file1.pdf file2.pdf ...  filen.pdf

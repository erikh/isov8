use isov8::*;
use std::io::Read;

pub fn main() {
    let mut iso = IsoV8::new();
    let mut io = std::io::stdin().lock();
    let mut input = Vec::new();
    io.read_to_end(&mut input).expect("Can't read I/O");
    let result = iso
        .eval(String::from_utf8(input).expect("Invalid UTF-8"))
        .unwrap();
    println!("Result: {:?}", result);
}

//! Small independent BLAKE3 inspector for files and length-framed body bytes.
use std::io::{BufRead, Read, Write};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--frames"] {
        let mut input = std::io::BufReader::new(std::io::stdin().lock());
        let mut output = std::io::BufWriter::new(std::io::stdout().lock());
        loop {
            let mut line = String::new();
            if input.read_line(&mut line).unwrap() == 0 { break }
            let mut bytes = vec![0u8; line.trim().parse::<usize>().unwrap()];
            input.read_exact(&mut bytes).unwrap();
            writeln!(output, "{}", blake3::hash(&bytes)).unwrap();
        }
    } else {
        for path in args {
            let bytes = std::fs::read(&path).unwrap();
            println!("{}\t{path}", blake3::hash(&bytes));
        }
    }
}

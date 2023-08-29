#![allow(clippy::explicit_auto_deref)]

use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, BufWriter};
use std::marker::PhantomData;
use std::mem;
use std::process::exit;
use std::sync::Arc;

use async_std::task;
use reverie::proof::Proof;
use reverie::Operation;
use reverie::CombineOperation;
use reverie::{largest_wires};

mod witness;

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

pub trait Parser<E>: Sized {
    fn new(reader: BufReader<File>) -> io::Result<Self>;

    fn next(&mut self) -> io::Result<Option<E>>;
}

enum FileStreamer<E, P: Parser<E>> {
    Memory(Arc<Vec<E>>, PhantomData<P>),
}

impl<E, P: Parser<E>> FileStreamer<E, P> {
    fn new(path: &str) -> io::Result<Self> {
        let file = File::open(path)?;
        let meta = file.metadata()?;

        // parse once and load into memory
        let reader = BufReader::new(file);
        let mut contents: Vec<E> = Vec::with_capacity(meta.len() as usize / mem::size_of::<E>());
        let mut parser = P::new(reader)?;
        while let Some(elem) = parser.next()? {
            contents.push(elem)
        }
        Ok(FileStreamer::Memory(Arc::new(contents), PhantomData))
    }

    fn rewind(&self) -> Arc<Vec<E>> {
        match self {
            FileStreamer::Memory(vec, PhantomData) => vec.clone(),
        }
    }
}

async fn oneshot_zk<WP: Parser<bool> + Send + 'static>(
    program_path: &str,
    witness_path: &str,
) -> io::Result<Result<(), String>> {
    // open and parse program
    let file = File::open(program_path)?;
    let mut reader = BufReader::new(file);

    // first read the number of gates and wires of the circuit
    let mut first_line = String::new();
    reader.read_line(&mut first_line).unwrap();
    let numbers: Vec<&str> = first_line.split_whitespace().collect();
    let num_gates: usize = numbers[0].parse().unwrap();
    let num_wires: usize = numbers[1].parse().unwrap();
    println!("#gates: {}, #wires: {}", num_gates, num_wires);

    // second read the number of input wires
    let mut second_line = String::new();
    reader.read_line(&mut second_line).unwrap();
    let numbers: Vec<&str> = second_line.split_whitespace().collect();
    let num_parties: usize = numbers[0].parse().unwrap();
    let num_input_alice: usize = numbers[1].parse().unwrap();
    let num_input_bob: usize = numbers[2].parse().unwrap();
    println!("#parties: {}, #alice input: {}, #bob input: {}", num_parties, num_input_alice, num_input_bob);

    // third read the number of output wires
    let mut third_line = String::new();
    reader.read_line(&mut third_line).unwrap();
    let numbers: Vec<&str> = third_line.split_whitespace().collect();
    let num_output : usize = numbers[0].parse().unwrap();
    println!("#output: {}", num_output);

    // read the gates
    let mut program: Vec<CombineOperation> = Vec::new();
    for _ in 0..num_gates {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).unwrap();
        if bytes_read == 0 {
            println!("empty line");
            break;
        }
        //println!("{}", line);
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let num_inputs: usize = tokens[0].parse().unwrap();
        let num_outputs: usize = tokens[1].parse().unwrap();
        let mut input_indices: Vec<usize> = Vec::new();
        let mut output_indices: Vec<usize> = Vec::new();
        for i in 2..(2 + num_inputs) {
            input_indices.push(tokens[i].parse().unwrap());
        }
        for i in (2 + num_inputs)..(2 + num_inputs + num_outputs) {
            output_indices.push(tokens[i].parse().unwrap());
        }
        let gate_type: &str = tokens[2+num_inputs+num_outputs];

        match gate_type {
            "INPUT" => program.push(
                reverie::CombineOperation::GF2(
                    Operation::Input(output_indices[0])
                    )
                ),
            "XOR" => program.push(
                reverie::CombineOperation::GF2(
                    Operation::Add(
                        output_indices[0],
                        input_indices[0],
                        input_indices[1]
                        )
                    )
                ),
            "AND" => program.push(
                reverie::CombineOperation::GF2(
                    Operation::Mul(
                        output_indices[0],
                        input_indices[0],
                        input_indices[1]
                        )
                    )
                ),
            "INV" => program.push(
                reverie::CombineOperation::GF2(
                    Operation::AddConst(
                        output_indices[0],
                        input_indices[0],
                        true
                        )
                    )
                ),
            _ => unimplemented!("Unsupported gate type: {}", gate_type),
        }
    }

    //let program: Vec<CombineOperation> = bincode::deserialize_from(reader).unwrap();

    // open and parse witness
    let witness: FileStreamer<_, WP> = FileStreamer::new(witness_path)?;

    println!("Evaluating program in ~zero knowledge~");
    let wire_counts = largest_wires(program.as_slice());

    let program_arc = Arc::new(program);

    // timer start
    use std::time::Instant;
    let now = Instant::now();

    // Create the proof
    let proof = Proof::new(
        program_arc.clone(),
        witness.rewind(),
        Arc::new(vec![]),
        wire_counts,
    );

    // timer ends
    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);

    // Write proof to file
    let proof_file = File::create("./proof/proof.bin")?;
    let proof_writer = BufWriter::new(proof_file);
    if bincode::serialize_into(proof_writer, &proof).is_ok() {
        println!("write proof to file");
    } else {
        println!("could not write proof to file");
    }

    // Verify the proof
    if proof.verify(program_arc, wire_counts) {
        Ok(Ok(()))
    } else {
        Ok(Err("Unverifiable Proof".to_string()))
    }
}

async fn async_main() {
    let res = oneshot_zk::<witness::WitParser>(
        "./program_file.txt",
        "./witness_file.txt",
    )
    .await;
    match res {
        Err(e) => {
            eprintln!("Invalid proof: {}", e);
            exit(-1)
        }
        Ok(output) => println!("{:?}", output),
    }
}

fn main() {
    task::block_on(async_main());
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn test_app() {
        app().debug_assert();
    }
}

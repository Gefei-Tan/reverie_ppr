use crate::algebra::*;
use crate::online;
use crate::preprocessing;
use crate::Instruction;

use rand::rngs::OsRng;
use rand_core::RngCore;

use async_channel::bounded;
use async_std::task;

use serde::{Deserialize, Serialize};

const CHANNEL_CAPACITY: usize = 100;
const CHUNK_SIZE: usize = 10_000_000;

/// # Example
/// 
/// Proving that you know bits a, b st. a * b = 1
/// 
/// ```
/// use reverie::Instruction;
/// use reverie::ProofGF2P8;
/// use reverie::algebra::gf2::*;
/// 
/// // result of satisfying witness
/// let result = vec![BIT1];
/// 
/// // inputs (the witness)
/// let witness = vec![BIT1, BIT1];
///
/// // the program (circuit description)
/// let program: Vec<Instruction<BitScalar>> = vec![
///     Instruction::Input(0),     // w[0] <- 0: assign w[0] the next bit from the witness
///     Instruction::Input(1),     // w[1] <- 1: assign w[1] the next bit from the witness
///     Instruction::Mul(2, 0, 1), // w[2] <- w[0] * w[1]
///     Instruction::Output(2)     // output the value of w[2]
/// ];
/// 
/// // generate a proof (proof implements serde Serialize / Deserialize)
/// let proof = ProofGF2P8::new(&program[..], &witness[..]);
/// 
/// // verify the proof (in production you should check the result)
/// let output = proof.verify(&program[..]).unwrap(); 
/// 
/// assert_eq!(&output[..], &result[..]);
/// ```
pub type ProofGF2P8 = Proof<gf2::GF2P8, 8, 8, 252, 256, 44>;

/// # Example
/// 
/// Proving that you know bits a, b st. a * b = 1
/// 
/// ```
/// use reverie::Instruction;
/// use reverie::ProofGF2P64;
/// use reverie::algebra::gf2::*;
/// 
/// // result of satisfying witness
/// let result = vec![BIT1];
/// 
/// // inputs (the witness)
/// let witness = vec![BIT1, BIT1];
///
/// // the program (circuit description)
/// let program: Vec<Instruction<BitScalar>> = vec![
///     Instruction::Input(0),     // w[0] <- 0: assign w[0] the next bit from the witness
///     Instruction::Input(1),     // w[1] <- 1: assign w[1] the next bit from the witness
///     Instruction::Mul(2, 0, 1), // w[2] <- w[0] * w[1]
///     Instruction::Output(2)     // output the value of w[2]
/// ];
/// 
/// // generate a proof (proof implements serde Serialize / Deserialize)
/// let proof = ProofGF2P64::new(&program[..], &witness[..]);
/// 
/// // verify the proof (in production you should check the result)
/// let output = proof.verify(&program[..]).unwrap(); 
/// 
/// assert_eq!(&output[..], &result[..]);
/// ```
pub type ProofGF2P64 = Proof<gf2::GF2P64, 64, 64, 631, 1024, 2>;

/// Simplified interface for in-memory proofs
/// with pre-processing verified simultaneously with online execution.
/// 
///
#[derive(Deserialize, Serialize)]
pub struct Proof<
    D: Domain,
    const P: usize,
    const PT: usize,
    const R: usize,
    const RT: usize,
    const H: usize,
> {
    preprocessing: preprocessing::Proof<D, P, PT, R, RT, H>,
    online: online::Proof<D, H, P, PT>,
    chunks: Vec<Vec<u8>>,
}

impl<
        D: Domain,
        const P: usize,
        const PT: usize,
        const R: usize,
        const RT: usize,
        const H: usize,
    > Proof<D, P, PT, R, RT, H>
{
    async fn new_async(program: Vec<Instruction<D::Scalar>>, witness: Vec<D::Scalar>) -> Self {
        // prove preprocessing
        let mut seed: [u8; 16] = [0; 16];

        OsRng.fill_bytes(&mut seed);
        let (preprocessing, pp_output) =
            preprocessing::Proof::new(seed, program.iter().cloned(), CHUNK_SIZE);



        // create prover for online phase
        let (online, prover) = online::StreamingProver::new(
            pp_output,
            program.iter().cloned(),
            witness.iter().cloned(),
        );

        let (send, recv) = bounded(CHANNEL_CAPACITY);
        let prover_task =
            task::spawn(prover.stream(send, program.into_iter(), witness.into_iter()));

        // read all chunks from online execution
        let mut chunks = vec![];
        while let Ok(chunk) = recv.recv().await {
            chunks.push(chunk)
        }

        // should never fail
        prover_task.await.unwrap();
        Proof {
            preprocessing,
            online,
            chunks,
        }
    }

    async fn verify_async(&self, program: Vec<Instruction<D::Scalar>>) -> Option<Vec<D::Scalar>> {
        // verify pre-processing
        let preprocessing = self.preprocessing.verify(program.clone().into_iter())?;

        // verify the online execution
        let verifier =
            online::StreamingVerifier::new(program.clone().into_iter(), self.online.clone());
        let (send, recv) = bounded(CHANNEL_CAPACITY);
        let task_online = task::spawn(verifier.verify(recv));

        // send proof to the verifier
        for chunk in self.chunks.clone().into_iter() {
            send.send(chunk).await.ok()?;
        }

        // check that online execution matches preprocessing
        task_online.await?.check(&preprocessing)
    }

    pub fn new(program: &[Instruction<D::Scalar>], witness: &[D::Scalar]) -> Self {
        task::block_on(Self::new_async(program.to_owned(), witness.to_owned()))
    }

    pub fn verify(&self, program: &[Instruction<D::Scalar>]) -> Option<Vec<D::Scalar>> {
        task::block_on(self.verify_async(program.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::algebra::gf2::BitScalar;
    use crate::algebra::RingElement;
    use crate::Instruction;

    #[test]
    fn test_gf2p64_simplified() {
        let mut result = vec![
            <BitScalar as RingElement>::ONE,
            <BitScalar as RingElement>::ONE,
            <BitScalar as RingElement>::ZERO,
            <BitScalar as RingElement>::ONE,
        ];
        let mut witness = vec![
            <BitScalar as RingElement>::ZERO,
            <BitScalar as RingElement>::ONE,
            <BitScalar as RingElement>::ONE,
        ];
        let mut program: Vec<Instruction<BitScalar>> = vec![
            Instruction::Input(0),     // v[0] <- 0
            Instruction::Input(1),     // v[1] <- 1
            Instruction::Input(2),     // v[2] <- 1
            Instruction::Output(2),    // <- v[2]
            Instruction::Mul(3, 2, 1), // v[3] <- v[2] * v[1]
            Instruction::Output(3),    // <- v[3]
            Instruction::Add(0, 1, 1), // v[0] <- v[1] + v[1] = 0
            Instruction::Output(0),    // <- v[0]
            Instruction::Add(0, 0, 2), // v[0] <- v[0] + v[2] = 1
            Instruction::Output(0),    // <- v[0]
        ];
        let proof = ProofGF2P64::new(&program[..], &witness[..]);

        let output = proof.verify(&program[..]).unwrap();

        assert_eq!(&output[..], &result[..]);
    }

    #[test]
    fn test_gf2p8_simplified() {
        let mut result = vec![
            <BitScalar as RingElement>::ONE,
            <BitScalar as RingElement>::ONE,
            <BitScalar as RingElement>::ZERO,
            <BitScalar as RingElement>::ONE,
        ];
        let mut witness = vec![
            <BitScalar as RingElement>::ZERO,
            <BitScalar as RingElement>::ONE,
            <BitScalar as RingElement>::ONE,
        ];
        let mut program: Vec<Instruction<BitScalar>> = vec![
            Instruction::Input(0),     // v[0] <- 0
            Instruction::Input(1),     // v[1] <- 1
            Instruction::Input(2),     // v[2] <- 1
            Instruction::Output(2),    // <- v[2]
            Instruction::Mul(3, 2, 1), // v[3] <- v[2] * v[1]
            Instruction::Output(3),    // <- v[3]
            Instruction::Add(0, 1, 1), // v[0] <- v[1] + v[1] = 0
            Instruction::Output(0),    // <- v[0]
            Instruction::Add(0, 0, 2), // v[0] <- v[0] + v[2] = 1
            Instruction::Output(0),    // <- v[0]
        ];
        let proof = ProofGF2P8::new(&program[..], &witness[..]);

        let output = proof.verify(&program[..]).unwrap();

        assert_eq!(&output[..], &result[..]);
    }
}

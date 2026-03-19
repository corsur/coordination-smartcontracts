use anchor_lang::prelude::*;
use crate::errors::CoordinationError;

/// Verifies a Groth16 range proof that the committed guess is ∈ {0, 1}.
///
/// The commitment is SHA-256(guess_byte || salt_bytes) and is the public input
/// to the circuit. The proof points use BN254 (altbn128) encoding:
///   proof_a: G1 point (64 bytes)
///   proof_b: G2 point (128 bytes)
///   proof_c: G1 point (64 bytes)
///
/// TODO: This is a stub. Replace the body with a real Groth16 verification call
/// once the circuit has been compiled and the trusted setup ceremony is complete.
///
/// Steps to enable:
///   1. cd circuits/bool_range
///   2. Compile circuit: circom bool_range.circom --r1cs --wasm
///   3. Run trusted setup: snarkjs groth16 setup ...
///   4. Export verifying key: snarkjs zkey export verificationkey
///   5. Convert to Groth16Verifyingkey struct format expected by groth16-solana
///   6. Replace this function body with the actual verification call
pub fn verify_bool_range_proof(
    _proof_a: &[u8; 64],
    _proof_b: &[u8; 128],
    _proof_c: &[u8; 64],
    _commitment: &[u8; 32],
) -> Result<()> {
    // ZK verification is not yet enabled — circuit has not been compiled.
    // Once the verifying key is available, replace this with a Groth16Verifier call.
    err!(CoordinationError::InvalidRangeProof)
}

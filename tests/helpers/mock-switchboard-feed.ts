/**
 * Mock Switchboard PullFeed account for bankrun tests.
 *
 * Creates a buffer matching PullFeedAccountData layout (3208 bytes)
 * with a valid discriminator and a single oracle submission at the
 * specified score value.
 */

import { PublicKey } from "@solana/web3.js";

const DISCRIMINATOR = Buffer.from([196, 27, 108, 196, 10, 215, 219, 40]);
const ACCOUNT_SIZE = 3208; // 8 (discriminator) + 3200 (struct)

// Switchboard V3 PRECISION = 18 decimals
const PRECISION = 18n;

/**
 * Build a mock PullFeedAccountData buffer.
 *
 * @param score - The composite score (e.g., 500_000)
 * @param slot - The slot number for the submission (must be within max_staleness of current slot)
 * @returns Buffer of ACCOUNT_SIZE bytes
 */
export function buildMockFeedBuffer(score: number, slot: number): Buffer {
  const buf = Buffer.alloc(ACCOUNT_SIZE);
  let offset = 0;

  // Discriminator (8 bytes)
  DISCRIMINATOR.copy(buf, offset);
  offset += 8;

  // submissions[32] — each 64 bytes (pubkey:32 + slot:8 + landed_at:8 + value:16)
  // Write one valid submission at index 0
  const submissionOffset = offset;

  // submission[0].oracle = zero pubkey (32 bytes)
  offset += 32;
  // submission[0].slot = slot (u64 LE)
  buf.writeBigUInt64LE(BigInt(slot), offset);
  offset += 8;
  // submission[0].landed_at = slot (u64 LE)
  buf.writeBigUInt64LE(BigInt(slot), offset);
  offset += 8;
  // submission[0].value = score * 10^18 (i128 LE)
  // Switchboard stores i128 scaled by 10^18
  const scaledValue = BigInt(score) * 10n ** PRECISION;
  // Write as two 64-bit parts (little-endian i128)
  const low = scaledValue & ((1n << 64n) - 1n);
  const high = scaledValue >> 64n;
  buf.writeBigUInt64LE(low, offset);
  offset += 8;
  buf.writeBigInt64LE(high, offset);
  offset += 8;

  // Skip remaining 31 submissions (already zeroed)
  offset = submissionOffset + 32 * 64;

  // authority (32 bytes) — zeroed
  offset += 32;
  // queue (32 bytes) — zeroed
  offset += 32;
  // feed_hash (32 bytes) — zeroed
  offset += 32;
  // initialized_at (i64)
  offset += 8;
  // permissions (u64)
  offset += 8;
  // max_variance (u64)
  offset += 8;
  // min_responses (u32) — set to 1
  buf.writeUInt32LE(1, offset);
  offset += 4;
  // name (32 bytes)
  offset += 32;
  // padding1 (1 byte)
  offset += 1;
  // permit_write_by_authority (u8)
  offset += 1;
  // historical_result_idx (u8)
  offset += 1;
  // min_sample_size (u8) — set to 1
  buf.writeUInt8(1, offset);
  offset += 1;
  // last_update_timestamp (i64)
  offset += 8;
  // lut_slot (u64)
  offset += 8;
  // _reserved1 (32 bytes)
  offset += 32;

  // CurrentResult:
  // value (i128) — same scaled value as the submission
  const resultValueOffset = offset;
  buf.writeBigUInt64LE(low, offset);
  offset += 8;
  buf.writeBigInt64LE(high, offset);
  offset += 8;
  // std_dev (i128) — zero
  offset += 16;
  // mean (i128) — same as value
  buf.writeBigUInt64LE(low, offset);
  offset += 8;
  buf.writeBigInt64LE(high, offset);
  offset += 8;
  // range (i128) — zero
  offset += 16;
  // min_value (i128) — same as value
  buf.writeBigUInt64LE(low, offset);
  offset += 8;
  buf.writeBigInt64LE(high, offset);
  offset += 8;
  // max_value (i128) — same as value
  buf.writeBigUInt64LE(low, offset);
  offset += 8;
  buf.writeBigInt64LE(high, offset);
  offset += 8;
  // num_samples (u8) — 1
  buf.writeUInt8(1, offset);
  offset += 1;
  // submission_idx (u8) — 0
  buf.writeUInt8(0, offset);
  offset += 1;
  // padding1 (6 bytes)
  offset += 6;
  // slot (u64) — current slot
  buf.writeBigUInt64LE(BigInt(slot), offset);
  offset += 8;
  // min_slot (u64)
  buf.writeBigUInt64LE(BigInt(slot), offset);
  offset += 8;
  // max_slot (u64)
  buf.writeBigUInt64LE(BigInt(slot), offset);
  offset += 8;

  // max_staleness (u32) — set to same as slot to avoid underflow in get_value()
  // get_value() does: clock_slot - max_staleness which panics if max_staleness > clock_slot
  buf.writeUInt32LE(slot, offset);
  offset += 4;
  // padding2 (12 bytes)
  offset += 12;

  // historical_results[32] — each 16 bytes (f32 + f32 + u64), zeroed
  offset += 32 * 16;

  // _ebuf4 (8 bytes)
  offset += 8;
  // _ebuf3 (24 bytes)
  offset += 24;

  // submission_timestamps[32] — i64 each
  // Set timestamp[0] to a reasonable value
  buf.writeBigInt64LE(BigInt(Math.floor(Date.now() / 1000)), submissionTimestampOffset(0));

  return buf;
}

function submissionTimestampOffset(index: number): number {
  // 8 (disc) + 2048 (submissions) + 32+32+32+8+8+8+4+32+1+1+1+1+8+8+32 (fields before result)
  // + 16*6+1+1+6+8+8+8 (CurrentResult) + 4+12 (max_staleness+pad) + 32*16 (_history) + 8+24 (_ebufs)
  const baseOffset =
    8 + 2048 + 32 + 32 + 32 + 8 + 8 + 8 + 4 + 32 + 1 + 1 + 1 + 1 + 8 + 8 + 32 +
    96 + 1 + 1 + 6 + 8 + 8 + 8 + 4 + 12 + 512 + 8 + 24;
  return baseOffset + index * 8;
}

/**
 * The Switchboard On-Demand program ID on mainnet/devnet.
 * Used as the owner of mock feed accounts.
 */
export const SWITCHBOARD_PROGRAM_ID = new PublicKey(
  "SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv"
);

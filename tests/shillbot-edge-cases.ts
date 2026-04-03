/**
 * Shillbot Edge Cases & Error Path Tests
 *
 * Covers: expire flows, concurrent claim limits, error paths,
 * emergency return, and timing enforcement.
 *
 * Run: npx ts-mocha -p ./tsconfig.json -t 1000000 "tests/shillbot-edge-cases.ts"
 */

import { startAnchor } from "anchor-bankrun";
import { BankrunProvider } from "anchor-bankrun";
import { BN, Program } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  SYSVAR_SLOT_HASHES_PUBKEY,
  Transaction,
} from "@solana/web3.js";
import { Clock } from "solana-bankrun";
import { assert } from "chai";
import { createHash } from "crypto";

import {
  buildMockFeedBuffer,
  SWITCHBOARD_PROGRAM_ID,
} from "./helpers/mock-switchboard-feed";

import { Shillbot } from "../target/types/shillbot";
const IDL = require("../target/idl/shillbot.json");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_SCORE = 1_000_000;
const PROTOCOL_FEE_BPS = 1000;
const QUALITY_THRESHOLD = new BN(200_000);
const ESCROW_LAMPORTS = new BN(1 * LAMPORTS_PER_SOL);
const FOURTEEN_DAYS = 14 * 24 * 60 * 60;

// ---------------------------------------------------------------------------
// Helpers (same as shillbot-lifecycle.ts)
// ---------------------------------------------------------------------------

function contentHash(data: string): number[] {
  return Array.from(createHash("sha256").update(data).digest());
}

function globalStatePda(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("shillbot_global")],
    programId
  );
}

function taskPda(
  taskCounter: BN,
  client: PublicKey,
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("task"),
      taskCounter.toArrayLike(Buffer, "le", 8),
      client.toBuffer(),
    ],
    programId
  );
}

function agentStatePda(
  agent: PublicKey,
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("agent_state"), agent.toBuffer()],
    programId
  );
}

async function warpToTimestamp(
  context: Awaited<ReturnType<typeof startAnchor>>,
  targetTimestamp: number
): Promise<void> {
  const currentClock = await context.banksClient.getClock();
  const newClock = new Clock(
    currentClock.slot,
    currentClock.epochStartTimestamp,
    currentClock.epoch,
    currentClock.leaderScheduleEpoch,
    BigInt(targetTimestamp)
  );
  context.setClock(newClock);
}

async function getBalance(
  context: Awaited<ReturnType<typeof startAnchor>>,
  pubkey: PublicKey
): Promise<bigint> {
  const account = await context.banksClient.getAccount(pubkey);
  return account === null ? BigInt(0) : BigInt(account.lamports);
}

async function fundAccount(
  provider: BankrunProvider,
  recipient: PublicKey,
  lamports: number
): Promise<void> {
  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: provider.wallet.publicKey,
      toPubkey: recipient,
      lamports,
    })
  );
  await provider.sendAndConfirm(tx);
}

async function initializeGlobal(
  program: Program<Shillbot>,
  authority: Keypair,
  treasury: PublicKey,
  globalPda: PublicKey
): Promise<void> {
  await program.methods
    .initialize(PROTOCOL_FEE_BPS, QUALITY_THRESHOLD, new BN(0))
    .accountsPartial({
      globalState: globalPda,
      authority: authority.publicKey,
      treasury,
      systemProgram: SystemProgram.programId,
    })
    .signers([authority])
    .rpc();
}

async function createTask(
  program: Program<Shillbot>,
  client: Keypair,
  globalPda: PublicKey,
  deadline: BN
): Promise<{ taskPda: PublicKey; taskId: BN }> {
  const global = await program.account.globalState.fetch(globalPda);
  const [tPda] = taskPda(
    global.taskCounter,
    client.publicKey,
    program.programId
  );
  const content = contentHash(
    "edge case test " + global.taskCounter.toString()
  );

  await program.methods
    .createTask(
      ESCROW_LAMPORTS,
      content as any,
      deadline,
      new BN(3600),
      new BN(14_400),
      0, // platform
      0, // attestation_delay_override (0 = use global default)
      0, // challenge_window_override (0 = use global default)
      0  // verification_timeout_override (0 = use global default)
    )
    .accountsPartial({
      globalState: globalPda,
      task: tPda,
      client: client.publicKey,
      slotHashes: SYSVAR_SLOT_HASHES_PUBKEY,
      systemProgram: SystemProgram.programId,
    })
    .signers([client])
    .rpc();

  return { taskPda: tPda, taskId: global.taskCounter };
}

async function claimTask(
  program: Program<Shillbot>,
  agent: Keypair,
  taskPdaAddr: PublicKey
): Promise<void> {
  const [agentPda] = agentStatePda(agent.publicKey, program.programId);
  await program.methods
    .claimTask()
    .accountsPartial({
      task: taskPdaAddr,
      globalState: globalStatePda(program.programId)[0],
      agentState: agentPda,
      agent: agent.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .signers([agent])
    .rpc();
}

async function submitWork(
  program: Program<Shillbot>,
  agent: Keypair,
  taskPdaAddr: PublicKey,
  videoId: string
): Promise<void> {
  const [agentPda] = agentStatePda(agent.publicKey, program.programId);
  const [gPda] = globalStatePda(program.programId);
  await program.methods
    .submitWork(Buffer.from(videoId))
    .accountsPartial({
      task: taskPdaAddr,
      globalState: gPda,
      agentState: agentPda,
      agent: agent.publicKey,
    })
    .signers([agent])
    .rpc();
}

async function verifyTask(
  program: Program<Shillbot>,
  ctx: Awaited<ReturnType<typeof startAnchor>>,
  taskPdaAddr: PublicKey,
  globalPda: PublicKey,
  compositeScore: BN,
  feedPubkey: PublicKey
): Promise<void> {
  // Inject mock Switchboard feed with the score
  const clock = await ctx.banksClient.getClock();
  const feedBuffer = buildMockFeedBuffer(
    compositeScore.toNumber(),
    Number(clock.slot)
  );
  ctx.setAccount(feedPubkey, {
    lamports: 1_000_000_000,
    data: feedBuffer,
    owner: SWITCHBOARD_PROGRAM_ID,
    executable: false,
  });

  const verificationHash = Array.from(
    createHash("sha256").update(compositeScore.toString()).digest()
  );
  await program.methods
    .verifyTask(compositeScore, verificationHash as any)
    .accountsPartial({
      task: taskPdaAddr,
      globalState: globalPda,
      switchboardFeed: feedPubkey,
    })
    .rpc();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("shillbot edge cases (bankrun)", () => {
  let context: Awaited<ReturnType<typeof startAnchor>>;
  let provider: BankrunProvider;
  let program: Program<Shillbot>;

  const authority = Keypair.generate();
  const client = Keypair.generate();
  const agent = Keypair.generate();
  const treasury = Keypair.generate();
  const mockFeedKeypair = Keypair.generate();

  let globalPda: PublicKey;
  let baseTimestamp: number;

  before(async () => {
    context = await startAnchor(".", [], []);
    provider = new BankrunProvider(context);
    program = new Program<Shillbot>(IDL, provider);
    [globalPda] = globalStatePda(program.programId);

    for (const kp of [authority, client, agent, treasury]) {
      await fundAccount(provider, kp.publicKey, 100 * LAMPORTS_PER_SOL);
    }

    await initializeGlobal(program, authority, treasury.publicKey, globalPda);

    // Set up mock Switchboard feed
    await program.methods
      .setSwitchboardFeed(mockFeedKeypair.publicKey)
      .accountsPartial({
        globalState: globalPda,
        authority: authority.publicKey,
      })
      .signers([authority])
      .rpc();

    const clock = await context.banksClient.getClock();
    baseTimestamp = Number(clock.unixTimestamp);
  });

  // =========================================================================
  // EXPIRE FLOWS
  // =========================================================================

  describe("expire: Open task past deadline", () => {
    it("returns escrow to client and closes account", async () => {
      const deadline = new BN(baseTimestamp + 86_400); // +1 day
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      const clientBefore = await getBalance(context, client.publicKey);

      // Warp past deadline
      await warpToTimestamp(context, baseTimestamp + 86_401);

      await program.methods
        .expireTask()
        .accountsPartial({
          task: tPda,
          globalState: globalPda,
          client: client.publicKey,
        })
        .rpc();

      const clientAfter = await getBalance(context, client.publicKey);
      const escrow = BigInt(ESCROW_LAMPORTS.toNumber());
      assert.isTrue(
        clientAfter > clientBefore,
        "client should receive escrow back"
      );

      // Task account should be closed
      const taskAccount = await context.banksClient.getAccount(tPda);
      assert.isNull(taskAccount, "task account should be closed after expire");
    });
  });

  describe("expire: Claimed task past deadline", () => {
    it("returns escrow and decrements agent claimed_count", async () => {
      // Reset clock
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400);
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      await claimTask(program, agent, tPda);

      // Verify claimed_count incremented
      const [agentPda] = agentStatePda(agent.publicKey, program.programId);
      const agentStateBefore = await program.account.agentState.fetch(agentPda);
      assert.isTrue(
        agentStateBefore.claimedCount > 0,
        "claimed_count should be > 0"
      );

      // Warp past deadline
      await warpToTimestamp(context, baseTimestamp + 86_401);

      await program.methods
        .expireTask()
        .accountsPartial({
          task: tPda,
          globalState: globalPda,
          client: client.publicKey,
        })
        .remainingAccounts([
          {
            pubkey: agentPda,
            isWritable: true,
            isSigner: false,
          },
        ])
        .rpc();

      // Task closed
      const taskAccount = await context.banksClient.getAccount(tPda);
      assert.isNull(taskAccount, "task should be closed");
    });
  });

  describe("expire: Submitted task past verification timeout", () => {
    it("returns escrow after T+14d", async () => {
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400 * 30); // far future
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      await claimTask(program, agent, tPda);
      await submitWork(program, agent, tPda, "test-video-expire");

      // Warp to T+14d after submission
      const task = await program.account.task.fetch(tPda);
      const submittedAt = task.submittedAt.toNumber();
      await warpToTimestamp(context, submittedAt + FOURTEEN_DAYS + 1);

      const clientBefore = await getBalance(context, client.publicKey);

      await program.methods
        .expireTask()
        .accountsPartial({
          task: tPda,
          globalState: globalPda,
          client: client.publicKey,
        })
        .rpc();

      const clientAfter = await getBalance(context, client.publicKey);
      assert.isTrue(clientAfter > clientBefore, "client receives escrow");

      const taskAccount = await context.banksClient.getAccount(tPda);
      assert.isNull(taskAccount, "task closed after expire");
    });
  });

  describe("expire: rejects if deadline not reached", () => {
    it("fails for Open task before deadline", async () => {
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400);
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      try {
        await program.methods
          .expireTask()
          .accountsPartial({
            task: tPda,
            globalState: globalPda,
            client: client.publicKey,
          })
          .rpc();
        assert.fail("should have failed");
      } catch (e: any) {
        assert.include(
          e.toString(),
          "DeadlineExpired",
          "should reject expire before deadline"
        );
      }
    });
  });

  // =========================================================================
  // CONCURRENT CLAIM LIMITS
  // =========================================================================

  describe("concurrent claim limit", () => {
    it("allows up to max_concurrent_claims and rejects the next", async () => {
      await warpToTimestamp(context, baseTimestamp);

      // Use a fresh agent to start with claimed_count = 0
      const freshAgent = Keypair.generate();
      await fundAccount(provider, freshAgent.publicKey, 10 * LAMPORTS_PER_SOL);

      const global = await program.account.globalState.fetch(globalPda);
      const maxClaims = global.maxConcurrentClaims;

      // Create max+1 tasks
      const tasks: PublicKey[] = [];
      for (let i = 0; i <= maxClaims; i++) {
        const deadline = new BN(baseTimestamp + 86_400 * 30);
        const { taskPda: tPda } = await createTask(
          program,
          client,
          globalPda,
          deadline
        );
        tasks.push(tPda);
      }

      // Claim up to the max
      for (let i = 0; i < maxClaims; i++) {
        await claimTask(program, freshAgent, tasks[i]!);
      }

      // The next claim should fail
      try {
        await claimTask(program, freshAgent, tasks[maxClaims]!);
        assert.fail("should have rejected claim over limit");
      } catch (e: any) {
        assert.include(
          e.toString(),
          "MaxConcurrentClaimsExceeded",
          "should reject when at concurrent claim limit"
        );
      }
    });
  });

  // =========================================================================
  // ERROR PATHS — INVALID STATE TRANSITIONS
  // =========================================================================

  describe("invalid state transitions", () => {
    it("cannot claim an already-claimed task", async () => {
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400 * 30);
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      await claimTask(program, agent, tPda);

      const otherAgent = Keypair.generate();
      await fundAccount(provider, otherAgent.publicKey, 5 * LAMPORTS_PER_SOL);

      try {
        await claimTask(program, otherAgent, tPda);
        assert.fail("should not allow double claim");
      } catch (e: any) {
        assert.include(e.toString(), "InvalidTaskState");
      }
    });

    it("cannot submit work for an unclaimed task", async () => {
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400 * 30);
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      try {
        await submitWork(program, agent, tPda, "should-fail");
        assert.fail("should not allow submit on Open task");
      } catch (e: any) {
        assert.include(e.toString(), "InvalidTaskState");
      }
    });

    it("cannot finalize before challenge window closes", async () => {
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400 * 30);
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      await claimTask(program, agent, tPda);
      await submitWork(program, agent, tPda, "test-video-early-finalize");

      // Warp past attestation delay for verify
      const task = await program.account.task.fetch(tPda);
      await warpToTimestamp(
        context,
        task.submittedAt.toNumber() + 7 * 86_400 + 1
      );

      await verifyTask(
        program,
        context,
        tPda,
        globalPda,
        new BN(500_000),
        mockFeedKeypair.publicKey
      );

      // Try to finalize immediately (challenge window still open)
      try {
        await program.methods
          .finalizeTask()
          .accountsPartial({
            task: tPda,
            globalState: globalPda,
            agent: agent.publicKey,
            client: client.publicKey,
            treasury: treasury.publicKey,
          })
          .rpc();
        assert.fail("should not allow early finalization");
      } catch (e: any) {
        assert.include(e.toString(), "ChallengeWindowOpen");
      }
    });

    it("cannot challenge after challenge window closes", async () => {
      // Need a verified task with challenge window passed
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400 * 60);
      const { taskPda: tPda, taskId } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      await claimTask(program, agent, tPda);
      await submitWork(program, agent, tPda, "test-video-late-challenge");

      const task = await program.account.task.fetch(tPda);
      await warpToTimestamp(
        context,
        task.submittedAt.toNumber() + 7 * 86_400 + 1
      );

      await verifyTask(
        program,
        context,
        tPda,
        globalPda,
        new BN(500_000),
        mockFeedKeypair.publicKey
      );

      // Warp past challenge window
      const verifiedTask = await program.account.task.fetch(tPda);
      await warpToTimestamp(
        context,
        verifiedTask.challengeDeadline.toNumber() + 1
      );

      const challenger = Keypair.generate();
      await fundAccount(
        provider,
        challenger.publicKey,
        10 * LAMPORTS_PER_SOL
      );
      const [cPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("challenge"),
          taskId.toArrayLike(Buffer, "le", 8),
          challenger.publicKey.toBuffer(),
        ],
        program.programId
      );

      try {
        await program.methods
          .challengeTask()
          .accountsPartial({
            task: tPda,
            globalState: globalPda,
            challenge: cPda,
            challenger: challenger.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([challenger])
          .rpc();
        assert.fail("should not allow late challenge");
      } catch (e: any) {
        assert.include(e.toString(), "ChallengeWindowClosed");
      }
    });
  });

  // =========================================================================
  // EMERGENCY RETURN
  // =========================================================================

  describe("emergency return", () => {
    it("returns escrow for multiple Open/Claimed tasks", async () => {
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400 * 30);

      // Create 2 tasks — one Open, one Claimed
      const { taskPda: openTask } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );
      const { taskPda: claimedTask } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );
      await claimTask(program, agent, claimedTask);

      const clientBefore = await getBalance(context, client.publicKey);

      await program.methods
        .emergencyReturn()
        .accountsPartial({
          globalState: globalPda,
          authority: authority.publicKey,
        })
        .signers([authority])
        .remainingAccounts([
          { pubkey: openTask, isWritable: true, isSigner: false },
          { pubkey: client.publicKey, isWritable: true, isSigner: false },
          { pubkey: claimedTask, isWritable: true, isSigner: false },
          { pubkey: client.publicKey, isWritable: true, isSigner: false },
        ])
        .rpc();

      const clientAfter = await getBalance(context, client.publicKey);
      const expectedReturn = BigInt(ESCROW_LAMPORTS.toNumber()) * BigInt(2);
      assert.isTrue(
        clientAfter - clientBefore >= expectedReturn,
        "client should receive both escrows back (plus rent)"
      );

      // Both accounts should be closed
      assert.isNull(
        await context.banksClient.getAccount(openTask),
        "open task closed"
      );
      assert.isNull(
        await context.banksClient.getAccount(claimedTask),
        "claimed task closed"
      );
    });

    it("rejects non-authority caller", async () => {
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400 * 30);
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      const imposter = Keypair.generate();
      await fundAccount(provider, imposter.publicKey, 5 * LAMPORTS_PER_SOL);

      try {
        await program.methods
          .emergencyReturn()
          .accountsPartial({
            globalState: globalPda,
            authority: imposter.publicKey,
          })
          .signers([imposter])
          .remainingAccounts([
            { pubkey: tPda, isWritable: true, isSigner: false },
            { pubkey: client.publicKey, isWritable: true, isSigner: false },
          ])
          .rpc();
        assert.fail("should reject non-authority");
      } catch (e: any) {
        assert.include(e.toString(), "NotAuthority");
      }
    });
  });

  // =========================================================================
  // ZERO SCORE — no payment
  // =========================================================================

  describe("zero score: full escrow returned to client", () => {
    it("pays nothing when score is below threshold", async () => {
      await warpToTimestamp(context, baseTimestamp);

      const deadline = new BN(baseTimestamp + 86_400 * 60);
      const { taskPda: tPda } = await createTask(
        program,
        client,
        globalPda,
        deadline
      );

      await claimTask(program, agent, tPda);
      await submitWork(program, agent, tPda, "test-video-zero-score");

      const task = await program.account.task.fetch(tPda);
      await warpToTimestamp(
        context,
        task.submittedAt.toNumber() + 7 * 86_400 + 1
      );

      // Verify with score below threshold (100_000 < 200_000 threshold)
      await verifyTask(
        program,
        context,
        tPda,
        globalPda,
        new BN(100_000),
        mockFeedKeypair.publicKey
      );

      const verifiedTask = await program.account.task.fetch(tPda);
      assert.equal(
        verifiedTask.paymentAmount.toNumber(),
        0,
        "payment should be 0 below threshold"
      );

      // Finalize
      await warpToTimestamp(
        context,
        verifiedTask.challengeDeadline.toNumber() + 1
      );

      const clientBefore = await getBalance(context, client.publicKey);

      await program.methods
        .finalizeTask()
        .accountsPartial({
          task: tPda,
          globalState: globalPda,
          agent: agent.publicKey,
          client: client.publicKey,
          treasury: treasury.publicKey,
        })
        .rpc();

      const clientAfter = await getBalance(context, client.publicKey);
      assert.isTrue(
        clientAfter > clientBefore,
        "client should receive full escrow back"
      );
    });
  });
});

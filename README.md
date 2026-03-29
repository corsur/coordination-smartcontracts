# Coordination DAO Smart Contracts

Solana programs for the [Coordination DAO](https://coordination.game): a DAO governing two protocols — the Coordination Game (anonymous social deduction) and Shillbot (AI agent task marketplace).

Built with [Anchor](https://www.anchor-lang.com/) on Solana.

## Programs

### Coordination Game (`coordination_game`)

An anonymous 1v1 social deduction game where players stake SOL and guess whether their opponent is human or AI.

Players are matched anonymously, chat via an off-chain relay, then each submits a guess via a commit-reveal scheme. Stakes are held in escrow on-chain and redistributed based on the payoff matrix when both guesses are revealed (or a timeout fires). Losing stake flows to the DAO treasury.

**Program ID:** `2qqVk7kUqffnahiJpcQJCsSd8ErbEUgKTgCn1zYsw64P`

### Shillbot (`shillbot`)

A task marketplace where autonomous AI agents create content (YouTube Shorts) on behalf of paying clients. Payment is escrowed on-chain and released based on oracle-verified performance metrics, with a challenge window for disputes.

**Program ID:** `2tR37nqMpwdV4DVUHjzUmL1rH2DtkA8zrRA4EAhT7KMi`

### Shared (`shared`)

Library crate (not a deployed program) containing platform-agnostic types used by both programs and off-chain services: `PlatformProof`, `EngagementMetrics`, `CompositeScore`, `ScoringWeights`.

## Architecture

```
smartcontracts/
├── programs/
│   ├── coordination/        # Coordination Game program
│   │   └── src/
│   │       ├── instructions/  # 12 instruction handlers
│   │       ├── state/         # Game, Tournament, PlayerProfile, Escrow, Session
│   │       ├── payoff.rs      # Payoff matrix computation
│   │       ├── errors.rs
│   │       └── events.rs
│   ├── shillbot/            # Shillbot Task Marketplace program
│   │   └── src/
│   │       ├── instructions/  # 11 instruction handlers
│   │       ├── state/         # Task, GlobalState, Challenge, AgentState
│   │       ├── scoring.rs     # Payment + bond computation (fixed-point)
│   │       ├── errors.rs
│   │       └── events.rs
│   └── shared/              # Shared types library crate
│       └── src/
│           ├── platform.rs    # PlatformProof, EngagementMetrics
│           ├── scoring.rs     # CompositeScore, ScoringWeights
│           └── constants.rs   # Shared constants
├── tests/
│   ├── coordination.ts        # Game end-to-end tests
│   └── shillbot.ts            # Shillbot end-to-end tests
├── sdk/                       # TypeScript SDK (published to GitHub Packages)
├── Anchor.toml
└── Makefile
```

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Solana CLI](https://docs.solanalabs.com/cli/install) v1.18+
- [Anchor CLI](https://www.anchor-lang.com/docs/installation) v0.32.1
- Node.js 20+

## Local Development

```sh
# Build all programs
make build

# Run the full test suite against a local validator
make test

# Clean build artifacts
make clean

# Run unit tests only (no validator needed)
cargo test

# Lint
cargo clippy -- -D warnings
```

`anchor test` starts a local validator, deploys programs, runs all end-to-end tests, then stops the validator.

## Coordination Game

### State Machine

```
         --(create_game)--> Pending
Pending --(join_game)--> Active
Active --(commit_guess: 1st)--> Committing
Committing --(commit_guess: 2nd)--> Revealing
Committing --(resolve_timeout)--> Resolved
Revealing --(reveal_guess: both)--> Resolved
Revealing --(resolve_timeout)--> Resolved
Resolved --(close_game)--> [account closed]
```

### Instructions

| Instruction | Signer | Description |
|---|---|---|
| `initialize` | any | One-time setup: creates `GameCounter` PDA |
| `create_tournament` | any | Create a time-bounded tournament with prize pool |
| `create_game` | player 1 | Create a game PDA and lock stake |
| `join_game` | player 2 | Join an existing game and lock stake |
| `deposit_stake` | player | Deposit stake into tournament escrow |
| `commit_guess` | player | Submit SHA-256(R) commitment |
| `reveal_guess` | player | Submit preimage R; resolves if both revealed |
| `resolve_timeout` | anyone | Slash non-participant after timeout |
| `close_game` | anyone | Reclaim rent from resolved game |
| `finalize_tournament` | anyone | Snapshot scores after tournament ends |
| `claim_reward` | player | Claim proportional prize from pool |
| `create_player_session` | player | Authorize an ephemeral session keypair |
| `close_player_session` | player | Revoke a session keypair |

### Commit-Reveal Scheme

Guesses are submitted in two phases to prevent front-running:

1. **Commit:** Generate random 32-byte preimage `R`. Encode guess in last bit: `R[31] = (R[31] & 0xFE) | guess`. Submit `commitment = SHA-256(R)`.
2. **Reveal:** Submit `R`. Program verifies `SHA-256(R) == commitment` and extracts `guess = R[31] & 1`.

### Payoff Matrix

| Matchup | Outcome | P1 Return | P2 Return |
|---|---|---|---|
| Same team | Both correct | -0.1 SOL | -0.1 SOL |
| Same team | Any wrong | -1 SOL | -1 SOL |
| Different teams | Both correct | First committer wins (+0.9, -1) |  |
| Different teams | Any wrong | First inaccurate loses (-1, +0.9) |  |

Losing stake accumulates in the Tournament PDA (DAO treasury).

### Session Keys

Players can authorize ephemeral session keypairs via `create_player_session` to avoid repeated wallet popups during gameplay. The session keypair signs game transactions (commit, reveal, deposit) on behalf of the player. Sessions expire after 24 hours or can be revoked with `close_player_session`.

## Shillbot Task Marketplace

### State Machine

```
         --(create_task)--> Open
Open --(claim_task)--> Claimed
Open --(expire_task)--> [escrow returned, closed]
Open --(emergency_return)--> [escrow returned, closed]
Claimed --(submit_work)--> Submitted
Claimed --(expire_task)--> [escrow returned, closed]
Submitted --(verify_task)--> Verified
Submitted --(expire_task: T+14d)--> [escrow returned, closed]
Verified --(finalize_task)--> [payment released, closed]
Verified --(challenge_task)--> Disputed
Disputed --(resolve_challenge)--> [resolved, closed]
```

### Instructions

| Instruction | Signer | Description |
|---|---|---|
| `initialize` | authority | One-time setup: creates `GlobalState` PDA |
| `create_task` | client | Create task PDA, fund escrow, set deadline |
| `claim_task` | agent | Claim an open task (max 5 concurrent) |
| `submit_work` | agent | Submit video ID hash as proof of work |
| `verify_task` | oracle | Record Switchboard-attested composite score |
| `finalize_task` | anyone | Release payment after challenge window (24h) |
| `challenge_task` | anyone | Post bond to dispute a verified task |
| `resolve_challenge` | multisig | Resolve dispute, distribute funds |
| `expire_task` | anyone | Return escrow for expired tasks |
| `emergency_return` | multisig | Batch-return escrow for Open/Claimed tasks |
| `revoke_session` | agent | Revoke MCP server session delegation |

### Payment Model

Payment scales linearly with the oracle-attested composite score:

- Below quality threshold: agent receives nothing, full escrow returned to client
- At threshold: agent receives minimum payment
- At max score: agent receives full payment minus protocol fee

All arithmetic uses checked operations with u128 intermediates. `payment + fee <= escrow` is asserted before every transfer.

### Challenge System

Anyone can challenge a verified task during the 24-hour challenge window by posting a bond (2-5x task escrow). The Squads multisig resolves disputes:

- **Challenger wins:** escrow returned to client, bond returned to challenger
- **Agent wins:** payment released, bond slashed (50/50 to agent and treasury)

Clients get 20% of campaign tasks as free challenges (no bond required).

## Security Model

- **PDA seed constraints** on all accounts — no account substitution attacks
- **Checked arithmetic** throughout — `#![deny(clippy::arithmetic_side_effects)]` at crate level
- **CEI ordering** — all state mutations before any CPI or lamport transfer
- **No `unsafe`** — zero unsafe blocks in all programs
- **No `.unwrap()`/`.expect()`** — all errors propagated via `?` or explicit match
- **Account ownership** verified via Anchor typed accounts
- **Signer checks** via Anchor `Signer` type
- **Upgrade authority** — Squads multisig on mainnet with 48h timelock; EOA on devnet

## Deployment

CI deploys to devnet on merge to `main` after all tests pass. Mainnet deployment requires Squads multisig approval.

The CI pipeline asserts the upgrade authority matches the expected Squads address on mainnet (fails if EOA).

## Code Standards

Full code standards are documented in [CLAUDE.md](./CLAUDE.md). Key rules:

- Functions ≤60 lines; thin instruction handlers that delegate to pure functions
- Minimum 2 assertions per function (pre/postconditions)
- No recursion (Solana BPF 4KB stack limit)
- All loops have fixed, verifiable upper bounds
- `init` for shillbot accounts; `init_if_needed` only for game PlayerProfile
- Events emitted for every state transition
- Named error variants for every failure mode

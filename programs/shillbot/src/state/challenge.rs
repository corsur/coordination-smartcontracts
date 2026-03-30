use anchor_lang::prelude::*;

#[account]
pub struct Challenge {
    pub task_id: u64,
    pub challenger: Pubkey,
    pub bond_lamports: u64,
    /// True if challenger is the task's client.
    pub is_client_challenge: bool,
    pub created_at: i64,
    pub resolved: bool,
    pub challenger_won: bool,
    pub bump: u8,
}

impl Challenge {
    pub const SPACE: usize = 8  // discriminator
        + 8   // task_id
        + 32  // challenger
        + 8   // bond_lamports
        + 1   // is_client_challenge
        + 8   // created_at
        + 1   // resolved
        + 1   // challenger_won
        + 1; // bump
}

#[account]
pub struct SessionDelegate {
    /// The agent who delegated.
    pub agent: Pubkey,
    /// The session key (MCP server holds this).
    pub delegate: Pubkey,
    /// Bitmask: 0x01 = claim_task, 0x02 = submit_work.
    pub allowed_instructions: u8,
    pub created_at: i64,
    pub bump: u8,
}

impl SessionDelegate {
    pub const SPACE: usize = 8  // discriminator
        + 32  // agent
        + 32  // delegate
        + 1   // allowed_instructions
        + 8   // created_at
        + 1; // bump
}

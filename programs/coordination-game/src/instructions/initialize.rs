use crate::state::GameCounter;
use anchor_lang::prelude::*;

/// One-time initialization to create the global game counter.
/// Must be called once before any games can be created.
pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    // Anchor's `init` constraint guarantees the account is newly created and
    // zeroed — no preconditions needed. The only mutations are writing the
    // bump and confirming count = 0, both of which are trivially correct by
    // construction. Adding require! here would assert constants, which clippy
    // correctly rejects as meaningless (clippy::assertions_on_constants).
    let counter = &mut ctx.accounts.game_counter;
    counter.count = 0;
    counter.bump = ctx.bumps.game_counter;
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = GameCounter::SPACE,
        seeds = [b"game_counter"],
        bump,
    )]
    pub game_counter: Account<'info, GameCounter>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

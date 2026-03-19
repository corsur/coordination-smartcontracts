use anchor_lang::prelude::*;
use crate::state::GameCounter;

/// One-time initialization to create the global game counter.
/// Must be called once before any games can be created.
pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
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

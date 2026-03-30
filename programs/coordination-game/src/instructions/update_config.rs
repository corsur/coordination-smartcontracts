use anchor_lang::prelude::*;

use crate::errors::CoordinationError;
use crate::state::global_config::{GlobalConfig, MAX_TREASURY_SPLIT_BPS, MIN_TREASURY_SPLIT_BPS};

/// Authority-gated: update GlobalConfig parameters.
pub fn update_config(ctx: Context<UpdateConfig>, treasury_split_bps: u16) -> Result<()> {
    let config = &ctx.accounts.global_config;

    // Checks
    require!(
        ctx.accounts.authority.key() == config.authority,
        CoordinationError::NotAuthority
    );
    require!(
        (MIN_TREASURY_SPLIT_BPS..=MAX_TREASURY_SPLIT_BPS).contains(&treasury_split_bps),
        CoordinationError::InvalidTreasurySplitBps
    );

    // Effects
    let config = &mut ctx.accounts.global_config;
    config.treasury_split_bps = treasury_split_bps;

    // Postcondition
    require!(
        config.treasury_split_bps >= MIN_TREASURY_SPLIT_BPS
            && config.treasury_split_bps <= MAX_TREASURY_SPLIT_BPS,
        CoordinationError::InvalidTreasurySplitBps
    );

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [b"global_config"],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
    pub authority: Signer<'info>,
}

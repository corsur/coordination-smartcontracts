use crate::errors::CoordinationError;
use crate::events::StakeDeposited;
use crate::instructions::session_utils::validate_session_authority;
use crate::state::{SessionAuthority, StakeEscrow, Tournament, FIXED_STAKE_LAMPORTS};
use anchor_lang::prelude::*;

/// Session-delegated variant of `deposit_stake`. The session key signs the
/// transaction instead of the player wallet. The session authority PDA's
/// lamports fund the stake transfer (the player pre-funded the session PDA
/// at creation time).
pub fn deposit_stake_session(ctx: Context<DepositStakeSession>) -> Result<()> {
    validate_session_authority(
        &ctx.accounts.session_authority,
        &ctx.accounts.player.key(),
        &ctx.accounts.session_signer.key(),
    )?;

    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.tournament.is_active(now),
        CoordinationError::OutsideTournamentWindow,
    );

    let escrow = &mut ctx.accounts.escrow;

    // Idempotent: if the escrow already has an unconsumed funded deposit at the
    // correct amount, no-op. If the amount doesn't match, fall through to re-deposit.
    if !escrow.consumed && escrow.amount > 0 {
        require!(
            escrow.player == ctx.accounts.player.key(),
            CoordinationError::InvalidGameState,
        );
        if escrow.amount == FIXED_STAKE_LAMPORTS {
            msg!("deposit_stake_session: escrow already active, no-op");
            return Ok(());
        }
        msg!("deposit_stake_session: stake amount changed, re-depositing");
    }
    escrow.player = ctx.accounts.player.key();
    escrow.tournament_id = ctx.accounts.tournament.tournament_id;
    escrow.amount = FIXED_STAKE_LAMPORTS;
    escrow.consumed = false;
    escrow.bump = ctx.bumps.escrow;

    // Postconditions
    require!(
        escrow.player == ctx.accounts.player.key(),
        CoordinationError::InvalidGameState,
    );
    require!(
        escrow.amount == FIXED_STAKE_LAMPORTS,
        CoordinationError::StakeMismatch,
    );

    // Transfer stake from the session signer (funded keypair) to escrow PDA.
    // The session signer was pre-funded by the player in the session setup tx.
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.session_signer.to_account_info(),
                to: ctx.accounts.escrow.to_account_info(),
            },
        ),
        FIXED_STAKE_LAMPORTS,
    )?;

    emit!(StakeDeposited {
        player: ctx.accounts.player.key(),
        tournament_id: ctx.accounts.tournament.tournament_id,
        amount: FIXED_STAKE_LAMPORTS,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct DepositStakeSession<'info> {
    #[account(
        init_if_needed,
        payer = session_signer,
        space = StakeEscrow::SPACE,
        seeds = [
            b"escrow",
            tournament.tournament_id.to_le_bytes().as_ref(),
            player.key().as_ref(),
        ],
        bump,
    )]
    pub escrow: Account<'info, StakeEscrow>,
    pub tournament: Account<'info, Tournament>,
    /// CHECK: The player wallet. Not a signer — the session key signs instead.
    /// Verified against session_authority.player in the handler.
    pub player: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            b"game_session",
            player.key().as_ref(),
            session_signer.key().as_ref(),
        ],
        bump = session_authority.bump,
    )]
    pub session_authority: Account<'info, SessionAuthority>,
    #[account(mut)]
    pub session_signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

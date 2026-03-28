use crate::events::SessionCreated;
use crate::state::{SessionAuthority, SESSION_DURATION_SECONDS, SESSION_FEE_FUND_LAMPORTS};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

/// Create a session authority PDA so the player can delegate transaction
/// signing to an ephemeral session key for the next 24 hours. A small SOL
/// amount is transferred to the session PDA to fund transaction fees.
pub fn create_player_session(ctx: Context<CreatePlayerSession>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let expires_at = now
        .checked_add(SESSION_DURATION_SECONDS)
        .ok_or(crate::errors::CoordinationError::ArithmeticOverflow)?;

    let session = &mut ctx.accounts.session_authority;
    session.player = ctx.accounts.player.key();
    session.session_key = ctx.accounts.session_key.key();
    session.expires_at = expires_at;
    session.bump = ctx.bumps.session_authority;

    // Postconditions
    require!(
        session.player == ctx.accounts.player.key(),
        crate::errors::CoordinationError::SessionPlayerMismatch,
    );
    require!(
        session.session_key == ctx.accounts.session_key.key(),
        crate::errors::CoordinationError::SessionSignerMismatch,
    );

    // Transfer fee funding SOL from player to session authority PDA
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.player.to_account_info(),
                to: ctx.accounts.session_authority.to_account_info(),
            },
        ),
        SESSION_FEE_FUND_LAMPORTS,
    )?;

    emit!(SessionCreated {
        player: ctx.accounts.player.key(),
        session_key: ctx.accounts.session_key.key(),
        expires_at,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CreatePlayerSession<'info> {
    #[account(
        init,
        payer = player,
        space = SessionAuthority::SPACE,
        seeds = [
            b"game_session",
            player.key().as_ref(),
            session_key.key().as_ref(),
        ],
        bump,
    )]
    pub session_authority: Account<'info, SessionAuthority>,
    #[account(mut)]
    pub player: Signer<'info>,
    /// The ephemeral session keypair's public key. Not required to sign here;
    /// the player is authorizing this key to act on their behalf.
    /// CHECK: This is the session public key provided by the player. No data
    /// is read from this account; it is only used for its key in PDA derivation.
    pub session_key: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

use crate::events::SessionClosed;
use crate::state::SessionAuthority;
use anchor_lang::prelude::*;

/// Close a session authority PDA, returning rent and any remaining SOL
/// (fee funding) to the player. Only the original player wallet can close
/// a session — the session key itself cannot.
pub fn close_player_session(ctx: Context<ClosePlayerSession>) -> Result<()> {
    let session = &ctx.accounts.session_authority;

    // Preconditions
    require!(
        session.player == ctx.accounts.player.key(),
        crate::errors::CoordinationError::SessionPlayerMismatch,
    );
    require!(
        session.session_key == ctx.accounts.session_key.key(),
        crate::errors::CoordinationError::SessionSignerMismatch,
    );

    let player_key = ctx.accounts.player.key();
    let session_key = ctx.accounts.session_key.key();

    // The `close = player` constraint on session_authority handles lamport
    // transfer and account zeroing automatically.

    emit!(SessionClosed {
        player: player_key,
        session_key,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct ClosePlayerSession<'info> {
    #[account(
        mut,
        close = player,
        seeds = [
            b"game_session",
            player.key().as_ref(),
            session_key.key().as_ref(),
        ],
        bump = session_authority.bump,
        constraint = session_authority.player == player.key(),
    )]
    pub session_authority: Account<'info, SessionAuthority>,
    #[account(mut)]
    pub player: Signer<'info>,
    /// CHECK: Used only for PDA seed derivation. Verified via the
    /// `session_authority.session_key` constraint in the handler.
    pub session_key: UncheckedAccount<'info>,
}

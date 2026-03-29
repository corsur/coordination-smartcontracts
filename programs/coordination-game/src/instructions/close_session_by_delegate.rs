use crate::errors::CoordinationError;
use crate::events::SessionClosed;
use crate::state::SessionAuthority;
use anchor_lang::prelude::*;

/// Close a session authority PDA using the session keypair itself.
/// The session key can close its own session — rent goes to the session
/// keypair (which then gets swept back to the player wallet).
///
/// This avoids requiring a wallet popup for cleanup. The existing
/// `close_player_session` instruction is still available when the
/// player wallet wants to close directly.
pub fn close_session_by_delegate(ctx: Context<CloseSessionByDelegate>) -> Result<()> {
    let session = &ctx.accounts.session_authority;

    // Checks: session key matches the signer
    require!(
        session.session_key == ctx.accounts.session_signer.key(),
        CoordinationError::SessionSignerMismatch,
    );

    let player_key = session.player;
    let session_key = ctx.accounts.session_signer.key();

    // The `close = session_signer` constraint handles lamport transfer
    // and account zeroing automatically.

    emit!(SessionClosed {
        player: player_key,
        session_key,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CloseSessionByDelegate<'info> {
    #[account(
        mut,
        close = session_signer,
        seeds = [
            b"game_session",
            session_authority.player.as_ref(),
            session_signer.key().as_ref(),
        ],
        bump = session_authority.bump,
    )]
    pub session_authority: Account<'info, SessionAuthority>,
    #[account(mut)]
    pub session_signer: Signer<'info>,
}

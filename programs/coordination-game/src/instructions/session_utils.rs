use crate::errors::CoordinationError;
use crate::state::SessionAuthority;
use anchor_lang::prelude::*;

/// Validate that a session authority PDA correctly authorizes the session
/// signer to act on behalf of the player. Checks:
/// 1. session_authority.player == player_key
/// 2. session_authority.session_key == session_signer_key
/// 3. session has not expired
pub fn validate_session_authority(
    session: &SessionAuthority,
    player_key: &Pubkey,
    session_signer_key: &Pubkey,
) -> Result<()> {
    require!(
        session.player == *player_key,
        CoordinationError::SessionPlayerMismatch,
    );
    require!(
        session.session_key == *session_signer_key,
        CoordinationError::SessionSignerMismatch,
    );
    let now = Clock::get()?.unix_timestamp;
    require!(now < session.expires_at, CoordinationError::SessionExpired);
    Ok(())
}

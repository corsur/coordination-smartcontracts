use anchor_lang::prelude::*;

use crate::errors::ShillbotError;
use crate::events::IdentityRevoked;
use crate::state::PlatformIdentity;

/// Agent revokes their platform identity.
///
/// Closes the PlatformIdentity PDA and returns rent to the agent.
/// Agent must revoke before re-registering with a different platform user ID.
pub fn revoke_identity(ctx: Context<RevokeIdentity>) -> Result<()> {
    // Checks: has_one = agent is enforced by Anchor constraint
    // Checks: identity account existence is enforced by Anchor deserialization
    let platform = ctx.accounts.identity.platform;
    let agent_key = ctx.accounts.agent.key();

    // Interactions (account closure handled by Anchor `close` constraint)
    emit!(IdentityRevoked {
        agent: agent_key,
        platform,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct RevokeIdentity<'info> {
    #[account(
        mut,
        close = agent,
        seeds = [
            b"identity",
            agent.key().as_ref(),
            &[identity.platform],
        ],
        bump = identity.bump,
        has_one = agent @ ShillbotError::NotTaskAgent,
    )]
    pub identity: Account<'info, PlatformIdentity>,
    #[account(mut)]
    pub agent: Signer<'info>,
}

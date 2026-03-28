use anchor_lang::prelude::*;

use crate::events::SessionRevoked;
use crate::state::SessionDelegate;

/// Agent revokes an MCP session delegation. Only the delegating agent can call.
pub fn revoke_session(ctx: Context<RevokeSession>) -> Result<()> {
    let session = &ctx.accounts.session_delegate;

    // Checks: the agent is the delegator (enforced by constraint below)
    // Checks: session account exists (enforced by Anchor deserialization)

    let agent_key = session.agent;
    let delegate_key = session.delegate;

    // Effects: account is closed by the `close` constraint

    // Interactions: none
    emit!(SessionRevoked {
        agent: agent_key,
        delegate: delegate_key,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct RevokeSession<'info> {
    #[account(
        mut,
        close = agent,
        seeds = [
            b"session",
            session_delegate.agent.as_ref(),
            session_delegate.delegate.as_ref(),
        ],
        bump = session_delegate.bump,
        has_one = agent,
    )]
    pub session_delegate: Account<'info, SessionDelegate>,
    #[account(mut)]
    pub agent: Signer<'info>,
}

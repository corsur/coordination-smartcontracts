use anchor_lang::prelude::*;

use crate::errors::ShillbotError;
use crate::events::TaskClaimed;
use crate::state::{Task, TaskState};
use crate::MAX_CONCURRENT_CLAIMS;

/// Agent claims an open task. Enforces minimum time buffer and concurrent claim limit.
///
/// Concurrent claim check: the caller passes agent's other Task accounts as
/// remaining_accounts. The handler counts those in Claimed state and rejects
/// if the limit is reached.
pub fn claim_task(ctx: Context<ClaimTask>) -> Result<()> {
    let clock = Clock::get()?;
    let task = &ctx.accounts.task;

    // Checks: state
    require!(task.state == TaskState::Open, ShillbotError::InvalidTaskState);

    // Checks: minimum time buffer before deadline
    let earliest_claim_deadline = clock
        .unix_timestamp
        .checked_add(task.claim_buffer)
        .ok_or(ShillbotError::ArithmeticOverflow)?;
    require!(
        earliest_claim_deadline < task.deadline,
        ShillbotError::ClaimBufferInsufficient
    );

    // Checks: concurrent claim limit via remaining accounts
    let agent_key = ctx.accounts.agent.key();
    let mut claimed_count: u8 = 0;
    require!(
        ctx.remaining_accounts.len() <= 20,
        ShillbotError::ArithmeticOverflow
    );
    for account_info in ctx.remaining_accounts.iter() {
        // Attempt to deserialize as Task; skip accounts that don't parse.
        // Only count tasks owned by this program, assigned to this agent, in Claimed state.
        if account_info.owner != ctx.program_id {
            continue;
        }
        let data = account_info.try_borrow_data()?;
        if data.len() < Task::SPACE {
            continue;
        }
        // Anchor discriminator check: first 8 bytes
        let disc = &data[..8];
        let expected_disc = Task::DISCRIMINATOR;
        if disc != expected_disc {
            continue;
        }
        if let Ok(other_task) = Task::try_deserialize(&mut &data[..]) {
            if other_task.agent == agent_key && other_task.state == TaskState::Claimed {
                claimed_count = claimed_count
                    .checked_add(1)
                    .ok_or(ShillbotError::ArithmeticOverflow)?;
            }
        }
    }
    require!(
        claimed_count < MAX_CONCURRENT_CLAIMS,
        ShillbotError::MaxConcurrentClaimsExceeded
    );

    // Effects
    let task = &mut ctx.accounts.task;
    task.agent = agent_key;
    task.state = TaskState::Claimed;

    // Interactions: none
    emit!(TaskClaimed {
        task_id: task.task_id,
        agent: agent_key,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimTask<'info> {
    #[account(mut)]
    pub task: Account<'info, Task>,
    pub agent: Signer<'info>,
}

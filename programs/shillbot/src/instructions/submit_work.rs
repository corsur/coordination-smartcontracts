use anchor_lang::prelude::*;

use crate::errors::ShillbotError;
use crate::events::WorkSubmitted;
use crate::state::{Task, TaskState};

/// Agent submits proof of work (YouTube video ID hash).
/// Must be called before deadline minus submit_margin.
pub fn submit_work(ctx: Context<SubmitWork>, video_id: Vec<u8>) -> Result<()> {
    let clock = Clock::get()?;
    let task = &ctx.accounts.task;

    // Checks: state
    require!(
        task.state == TaskState::Claimed,
        ShillbotError::InvalidTaskState
    );

    // Checks: agent identity
    require!(
        ctx.accounts.agent.key() == task.agent,
        ShillbotError::NotTaskAgent
    );

    // Checks: submission before deadline minus margin
    let submission_deadline = task
        .deadline
        .checked_sub(task.submit_margin)
        .ok_or(ShillbotError::ArithmeticOverflow)?;
    require!(
        clock.unix_timestamp < submission_deadline,
        ShillbotError::SubmitMarginInsufficient
    );

    // Compute video ID hash
    let video_id_hash: [u8; 32] = solana_sha256_hasher::hash(&video_id).to_bytes();

    // Effects
    let task = &mut ctx.accounts.task;
    task.video_id_hash = video_id_hash;
    task.submitted_at = clock.unix_timestamp;
    task.state = TaskState::Submitted;

    // Interactions: none
    emit!(WorkSubmitted {
        task_id: task.task_id,
        agent: ctx.accounts.agent.key(),
        video_id_hash,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SubmitWork<'info> {
    #[account(mut)]
    pub task: Account<'info, Task>,
    pub agent: Signer<'info>,
}

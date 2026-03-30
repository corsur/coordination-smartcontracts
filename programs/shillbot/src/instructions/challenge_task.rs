use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::errors::ShillbotError;
use crate::events::TaskChallenged;
use crate::scoring::compute_challenge_bond;
use crate::state::{Challenge, Task, TaskState};
use crate::MIN_CHALLENGE_BOND_MULTIPLIER;

/// Anyone can challenge a verified task by posting a bond during the challenge window.
/// All challengers (including the task's client) pay the standard bond.
pub fn challenge_task(ctx: Context<ChallengeTask>) -> Result<()> {
    let clock = Clock::get()?;
    let task = &ctx.accounts.task;
    let challenger_key = ctx.accounts.challenger.key();

    // Checks: task must be in Verified state
    require!(
        task.state == TaskState::Verified,
        ShillbotError::InvalidTaskState
    );

    // Checks: must be within challenge window
    require!(
        clock.unix_timestamp < task.challenge_deadline,
        ShillbotError::ChallengeWindowClosed
    );

    // Compute bond — all challengers pay the standard bond
    let is_client_challenge = challenger_key == task.client;
    let bond_lamports =
        compute_challenge_bond(task.escrow_lamports, MIN_CHALLENGE_BOND_MULTIPLIER)?;

    // Effects: initialize challenge PDA
    let challenge = &mut ctx.accounts.challenge;
    challenge.task_id = task.task_id;
    challenge.challenger = challenger_key;
    challenge.bond_lamports = bond_lamports;
    challenge.is_client_challenge = is_client_challenge;
    challenge.created_at = clock.unix_timestamp;
    challenge.resolved = false;
    challenge.challenger_won = false;
    challenge.bump = ctx.bumps.challenge;

    // Effects: transition task to Disputed
    let task = &mut ctx.accounts.task;
    task.state = TaskState::Disputed;

    // Interactions: transfer bond from challenger to challenge PDA
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.challenger.to_account_info(),
                to: ctx.accounts.challenge.to_account_info(),
            },
        ),
        bond_lamports,
    )?;

    emit!(TaskChallenged {
        task_id: task.task_id,
        challenger: challenger_key,
        bond_lamports,
        is_client_challenge,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ChallengeTask<'info> {
    #[account(
        mut,
        seeds = [
            b"task",
            task.task_id.to_le_bytes().as_ref(),
            task.client.as_ref(),
        ],
        bump = task.bump,
    )]
    pub task: Account<'info, Task>,
    #[account(
        init,
        payer = challenger,
        space = Challenge::SPACE,
        seeds = [
            b"challenge",
            task.task_id.to_le_bytes().as_ref(),
            challenger.key().as_ref(),
        ],
        bump,
    )]
    pub challenge: Account<'info, Challenge>,
    #[account(mut)]
    pub challenger: Signer<'info>,
    pub system_program: Program<'info, System>,
}

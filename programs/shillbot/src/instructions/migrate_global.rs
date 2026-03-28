use anchor_lang::prelude::*;

use crate::errors::ShillbotError;

/// One-time migration: close old GlobalState so it can be re-initialized.
/// Authority-only. Devnet only — remove after migration.
pub fn migrate_global(ctx: Context<MigrateGlobal>) -> Result<()> {
    // Manually verify the old account's authority field at known offset.
    // Old layout: discriminator (8) + task_counter (8) + authority (32) = offset 8..16 for counter, 16..48 for authority.
    let data = ctx.accounts.global_state.try_borrow_data()?;
    require!(data.len() >= 48, ShillbotError::InvalidTaskState);

    let mut authority_bytes = [0u8; 32];
    authority_bytes.copy_from_slice(&data[16..48]);
    let stored_authority = Pubkey::new_from_array(authority_bytes);
    require!(
        stored_authority == ctx.accounts.authority.key(),
        ShillbotError::NotAuthority
    );

    // Read task_counter so we can preserve it.
    let mut counter_bytes = [0u8; 8];
    counter_bytes.copy_from_slice(&data[8..16]);
    let task_counter = u64::from_le_bytes(counter_bytes);
    msg!("Preserving task_counter: {}", task_counter);
    drop(data);

    // Transfer all lamports from the PDA to authority (effectively closing it).
    let pda_lamports = ctx.accounts.global_state.lamports();
    **ctx.accounts.global_state.try_borrow_mut_lamports()? = 0;
    **ctx.accounts.authority.try_borrow_mut_lamports()? = ctx
        .accounts
        .authority
        .lamports()
        .checked_add(pda_lamports)
        .ok_or(ShillbotError::ArithmeticOverflow)?;

    // Zero the data so the account can be reclaimed.
    let mut data = ctx.accounts.global_state.try_borrow_mut_data()?;
    data.fill(0);

    // Store the task_counter in the migrate_data account so we can read it during re-init.
    // Actually, emit it as an event so the migration script knows the value.
    msg!("Migration complete. task_counter={}", task_counter);

    Ok(())
}

#[derive(Accounts)]
pub struct MigrateGlobal<'info> {
    /// CHECK: Raw account — cannot deserialize old layout as new GlobalState.
    #[account(
        mut,
        seeds = [b"shillbot_global"],
        bump,
    )]
    pub global_state: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

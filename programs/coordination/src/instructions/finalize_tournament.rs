use anchor_lang::prelude::*;
use anchor_lang::Discriminator;
use crate::errors::CoordinationError;
use crate::events::TournamentFinalized;
use crate::state::{PlayerProfile, Tournament};

/// Snapshots the prize pool and total player score after tournament end.
/// Permissionless — any wallet can call.
///
/// All PlayerProfile accounts for this tournament must be passed as
/// remaining_accounts. Each is verified as a valid PDA before its score
/// is included in the total.
///
/// Limitation: capped at ~30 profiles per transaction due to Solana
/// account limits. Redesign required for larger tournaments.
pub fn finalize_tournament(ctx: Context<FinalizeTournament>) -> Result<()> {
    let tournament = &ctx.accounts.tournament;
    require!(
        Clock::get()?.unix_timestamp > tournament.end_time,
        CoordinationError::TournamentNotEnded,
    );
    require!(!tournament.finalized, CoordinationError::TournamentNotFinalized);

    let prize_snapshot = tournament.prize_lamports;
    let tournament_id = tournament.tournament_id;
    let program_id = ctx.program_id;

    // Sum scores across all provided PlayerProfile accounts.
    // Read raw account data to avoid lifetime conflicts with the mutable
    // borrow of tournament that follows.
    let mut total_score: u64 = 0;
    for account_info in ctx.remaining_accounts.iter() {
        require!(
            account_info.owner == program_id,
            CoordinationError::ProfileTournamentMismatch,
        );

        let data = account_info.try_borrow_data()?;
        // Verify discriminator matches PlayerProfile (first 8 bytes)
        require!(
            data.len() >= PlayerProfile::SPACE
                && data[..8] == *PlayerProfile::DISCRIMINATOR,
            CoordinationError::ProfileTournamentMismatch,
        );

        // Deserialize score from the account data.
        // PlayerProfile layout after discriminator:
        //   wallet: Pubkey (32), tournament_id: u64 (8), wins: u64 (8),
        //   total_games: u64 (8), score: u64 (8), claimed: bool (1), bump: u8 (1)
        let profile_tournament_id = u64::from_le_bytes(
            data[8 + 32..8 + 32 + 8].try_into().unwrap()
        );
        require!(
            profile_tournament_id == tournament_id,
            CoordinationError::ProfileTournamentMismatch,
        );

        let score = u64::from_le_bytes(
            data[8 + 32 + 8 + 8 + 8..8 + 32 + 8 + 8 + 8 + 8].try_into().unwrap()
        );

        total_score = total_score
            .checked_add(score)
            .ok_or(CoordinationError::ArithmeticOverflow)?;
    }

    let tournament = &mut ctx.accounts.tournament;
    tournament.finalized = true;
    tournament.prize_snapshot = prize_snapshot;
    tournament.total_score_snapshot = total_score;

    emit!(TournamentFinalized {
        tournament_id,
        prize_snapshot,
        total_score_snapshot: total_score,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct FinalizeTournament<'info> {
    #[account(
        mut,
        seeds = [b"tournament", tournament.tournament_id.to_le_bytes().as_ref()],
        bump = tournament.bump,
    )]
    pub tournament: Account<'info, Tournament>,
    pub caller: Signer<'info>,
}

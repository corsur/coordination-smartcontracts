use anchor_lang::prelude::*;
use crate::errors::CoordinationError;
use crate::events::TimeoutSlash;
use crate::state::{Game, GameState, PlayerProfile, Tournament, REVEAL_TIMEOUT_SLOTS};

pub fn resolve_timeout(ctx: Context<ResolveTimeout>) -> Result<()> {
    let game = &ctx.accounts.game;
    require!(
        game.state == GameState::Committing || game.state == GameState::Revealing,
        CoordinationError::InvalidGameState,
    );

    let current_slot = Clock::get()?.slot;
    let now = Clock::get()?.unix_timestamp;

    let (slashed_player, slash_amount, winner_is_p1) =
        find_timeout(game, current_slot)?;

    // Transfer slash amount to tournament prize pool
    transfer_from_game(
        &ctx.accounts.game.to_account_info(),
        &ctx.accounts.tournament.to_account_info(),
        slash_amount,
    )?;

    // Return winner's stake
    let winner_return = game.stake_lamports;
    let winner_account = if winner_is_p1 {
        ctx.accounts.player_one_wallet.to_account_info()
    } else {
        ctx.accounts.player_two_wallet.to_account_info()
    };
    transfer_from_game(
        &ctx.accounts.game.to_account_info(),
        &winner_account,
        winner_return,
    )?;

    // Update tournament
    let tournament = &mut ctx.accounts.tournament;
    tournament.prize_lamports = tournament.prize_lamports
        .checked_add(slash_amount)
        .ok_or(CoordinationError::ArithmeticOverflow)?;
    tournament.game_count = tournament.game_count
        .checked_add(1)
        .ok_or(CoordinationError::ArithmeticOverflow)?;

    // Update player profiles: winner gets a win, loser gets a loss
    let tournament_id = ctx.accounts.tournament.tournament_id;
    update_profile(&mut ctx.accounts.p1_profile, winner_is_p1, tournament_id)?;
    update_profile(&mut ctx.accounts.p2_profile, !winner_is_p1, tournament_id)?;

    let game = &mut ctx.accounts.game;
    game.state = GameState::Resolved;
    game.resolved_at = now;

    emit!(TimeoutSlash {
        game_id: game.game_id,
        slashed_player,
        slash_amount,
    });
    Ok(())
}

/// Returns (slashed_player, slash_amount, winner_is_p1).
/// slash_amount = slashed player's stake_lamports (full forfeit).
fn find_timeout(game: &Game, current_slot: u64) -> Result<(Pubkey, u64, bool)> {
    match game.state {
        GameState::Committing => {
            // One player committed, the other hasn't — the non-committer timed out
            let p1_committed = game.p1_commit != [0u8; 32];
            let commit_slot = if p1_committed {
                game.p1_commit_slot
            } else {
                game.p2_commit_slot
            };
            require!(
                current_slot >= commit_slot
                    .checked_add(game.commit_timeout_slots)
                    .ok_or(CoordinationError::ArithmeticOverflow)?,
                CoordinationError::TimeoutNotElapsed,
            );
            if p1_committed {
                // P2 timed out; P1 wins
                Ok((game.player_two, game.stake_lamports, true))
            } else {
                // P1 timed out; P2 wins
                Ok((game.player_one, game.stake_lamports, false))
            }
        }
        GameState::Revealing => {
            // Both committed; one or both haven't revealed within REVEAL_TIMEOUT_SLOTS
            let p1_revealed = game.p1_guess != crate::state::GUESS_UNREVEALED;
            let p2_revealed = game.p2_guess != crate::state::GUESS_UNREVEALED;

            // Use the later commit slot as the timeout anchor — both had to commit
            // before revealing, so the clock starts from the last commit
            let anchor_slot = game.p1_commit_slot.max(game.p2_commit_slot);
            let deadline = anchor_slot
                .checked_add(REVEAL_TIMEOUT_SLOTS)
                .ok_or(CoordinationError::ArithmeticOverflow)?;
            require!(current_slot >= deadline, CoordinationError::TimeoutNotElapsed);

            match (p1_revealed, p2_revealed) {
                (true, false) => Ok((game.player_two, game.stake_lamports, true)),
                (false, true) => Ok((game.player_one, game.stake_lamports, false)),
                (false, false) => {
                    // Both timed out — both stakes go to tournament; no winner (treat as p2 wins
                    // for profile update symmetry: neither gets a win, both get a loss)
                    Ok((game.player_one, game.stake_lamports, false))
                }
                (true, true) => {
                    // Both revealed — should have been resolved already
                    err!(CoordinationError::InvalidGameState)
                }
            }
        }
        _ => err!(CoordinationError::InvalidGameState),
    }
}

fn update_profile(profile: &mut PlayerProfile, won: bool, tournament_id: u64) -> Result<()> {
    require!(
        profile.tournament_id == tournament_id,
        CoordinationError::ProfileTournamentMismatch,
    );
    if won {
        profile.wins = profile.wins
            .checked_add(1)
            .ok_or(CoordinationError::ArithmeticOverflow)?;
    }
    profile.total_games = profile.total_games
        .checked_add(1)
        .ok_or(CoordinationError::ArithmeticOverflow)?;
    if profile.total_games > 0 {
        profile.score = PlayerProfile::compute_score(profile.wins, profile.total_games)?;
    }
    Ok(())
}

fn transfer_from_game(from: &AccountInfo, to: &AccountInfo, lamports: u64) -> Result<()> {
    if lamports == 0 {
        return Ok(());
    }
    **from.try_borrow_mut_lamports()? = from
        .lamports()
        .checked_sub(lamports)
        .ok_or(CoordinationError::ArithmeticOverflow)?;
    **to.try_borrow_mut_lamports()? = to
        .lamports()
        .checked_add(lamports)
        .ok_or(CoordinationError::ArithmeticOverflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct ResolveTimeout<'info> {
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, Game>,
    #[account(
        mut,
        seeds = [
            b"player",
            tournament.tournament_id.to_le_bytes().as_ref(),
            game.player_one.as_ref(),
        ],
        bump = p1_profile.bump,
        constraint = p1_profile.wallet == game.player_one,
    )]
    pub p1_profile: Account<'info, PlayerProfile>,
    #[account(
        mut,
        seeds = [
            b"player",
            tournament.tournament_id.to_le_bytes().as_ref(),
            game.player_two.as_ref(),
        ],
        bump = p2_profile.bump,
        constraint = p2_profile.wallet == game.player_two,
    )]
    pub p2_profile: Account<'info, PlayerProfile>,
    #[account(
        mut,
        seeds = [b"tournament", game.tournament_id.to_le_bytes().as_ref()],
        bump = tournament.bump,
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: Verified by address constraint against game.player_one
    #[account(mut, address = game.player_one)]
    pub player_one_wallet: UncheckedAccount<'info>,
    /// CHECK: Verified by address constraint against game.player_two
    #[account(mut, address = game.player_two)]
    pub player_two_wallet: UncheckedAccount<'info>,
    /// Caller receives no prize but pays the transaction fee; rent reclaim via close_game
    pub caller: Signer<'info>,
}

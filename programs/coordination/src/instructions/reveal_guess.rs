use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};
use crate::errors::CoordinationError;
use crate::events::{GameResolved, GuessRevealed};
use crate::payoff::resolve_homogenous;
use crate::state::{Game, GameState, PlayerProfile, Tournament, GUESS_UNREVEALED};

pub fn reveal_guess(ctx: Context<RevealGuess>, guess: u8, salt: [u8; 32]) -> Result<()> {
    require!(
        ctx.accounts.game.state == GameState::Revealing,
        CoordinationError::InvalidGameState,
    );
    require!(
        guess == 0 || guess == 1,
        CoordinationError::InvalidGuessValue,
    );

    let player_key = ctx.accounts.player.key();
    let game = &ctx.accounts.game;
    let is_p1 = player_key == game.player_one;
    let is_p2 = player_key == game.player_two;
    require!(is_p1 || is_p2, CoordinationError::NotAParticipant);

    if is_p1 {
        require!(game.p1_guess == GUESS_UNREVEALED, CoordinationError::AlreadyRevealed);
    } else {
        require!(game.p2_guess == GUESS_UNREVEALED, CoordinationError::AlreadyRevealed);
    }

    // Verify commitment: SHA-256(guess_byte || salt)
    let computed: [u8; 32] = {
        let mut h = Sha256::new();
        h.update([guess]);
        h.update(salt);
        h.finalize().into()
    };
    let stored = if is_p1 { game.p1_commit } else { game.p2_commit };
    require!(computed == stored, CoordinationError::CommitmentMismatch);

    let game = &mut ctx.accounts.game;
    if is_p1 {
        game.p1_guess = guess;
    } else {
        game.p2_guess = guess;
    }

    emit!(GuessRevealed { game_id: game.game_id, player: player_key });

    let both_revealed =
        game.p1_guess != GUESS_UNREVEALED && game.p2_guess != GUESS_UNREVEALED;

    if both_revealed {
        finalize_game(ctx)?;
    }

    Ok(())
}

fn finalize_game(ctx: Context<RevealGuess>) -> Result<()> {
    let game = &ctx.accounts.game;
    let now = Clock::get()?.unix_timestamp;

    let resolution = resolve_homogenous(game.p1_guess, game.p2_guess, game.stake_lamports)?;

    let game_id = game.game_id;
    let tournament_id = game.tournament_id;

    // Late resolution: return full stakes, contribute nothing to prize pool
    let (p1_return, p2_return, tournament_gain) =
        if now > ctx.accounts.tournament.end_time {
            (game.stake_lamports, game.stake_lamports, 0u64)
        } else {
            (resolution.p1_return, resolution.p2_return, resolution.tournament_gain)
        };

    // Transfer lamports out of game PDA — game account pays out all stake
    transfer_from_game(
        &ctx.accounts.game.to_account_info(),
        &ctx.accounts.player_one_wallet.to_account_info(),
        p1_return,
    )?;
    transfer_from_game(
        &ctx.accounts.game.to_account_info(),
        &ctx.accounts.player_two_wallet.to_account_info(),
        p2_return,
    )?;
    if tournament_gain > 0 {
        transfer_from_game(
            &ctx.accounts.game.to_account_info(),
            &ctx.accounts.tournament.to_account_info(),
            tournament_gain,
        )?;
    }

    // Update tournament state
    if tournament_gain > 0 {
        let tournament = &mut ctx.accounts.tournament;
        tournament.prize_lamports = tournament.prize_lamports
            .checked_add(tournament_gain)
            .ok_or(CoordinationError::ArithmeticOverflow)?;
        tournament.game_count = tournament.game_count
            .checked_add(1)
            .ok_or(CoordinationError::ArithmeticOverflow)?;
    }

    // Update player profiles
    let p1_won = p1_return > p2_return;
    let p2_won = p2_return > p1_return;

    update_profile(&mut ctx.accounts.p1_profile, p1_won, tournament_id)?;
    update_profile(&mut ctx.accounts.p2_profile, p2_won, tournament_id)?;

    // Mark game resolved
    let game = &mut ctx.accounts.game;
    game.state = GameState::Resolved;
    game.resolved_at = now;

    emit!(GameResolved {
        game_id,
        p1_guess: game.p1_guess,
        p2_guess: game.p2_guess,
        p1_return,
        p2_return,
        tournament_gain,
    });
    Ok(())
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
    profile.score = PlayerProfile::compute_score(profile.wins, profile.total_games)?;
    Ok(())
}

/// Transfer lamports directly from game PDA (program-owned) to a destination.
/// Safe because the game account is owned by this program.
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
pub struct RevealGuess<'info> {
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, Game>,
    pub player: Signer<'info>,
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
    /// CHECK: Destination for player one's stake return — verified by game.player_one
    #[account(mut, address = game.player_one)]
    pub player_one_wallet: UncheckedAccount<'info>,
    /// CHECK: Destination for player two's stake return — verified by game.player_two
    #[account(mut, address = game.player_two)]
    pub player_two_wallet: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

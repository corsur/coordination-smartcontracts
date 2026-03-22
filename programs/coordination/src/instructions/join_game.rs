use crate::errors::CoordinationError;
use crate::events::GameStarted;
use crate::state::{Game, GameState, PlayerProfile, Tournament};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

pub fn join_game(ctx: Context<JoinGame>) -> Result<()> {
    let game = &ctx.accounts.game;
    require!(
        game.state == GameState::Pending,
        CoordinationError::InvalidGameState
    );
    require!(
        ctx.accounts.player.key() != game.player_one,
        CoordinationError::CannotJoinOwnGame,
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.tournament.is_active(now),
        CoordinationError::OutsideTournamentWindow,
    );

    // Init player profile if needed
    let tournament_id = ctx.accounts.tournament.tournament_id;
    ctx.accounts.player_profile.init_if_new(
        ctx.accounts.player.key(),
        tournament_id,
        ctx.bumps.player_profile,
    );
    require!(
        ctx.accounts.player_profile.tournament_id == tournament_id,
        CoordinationError::ProfileTournamentMismatch,
    );

    let stake_lamports = ctx.accounts.game.stake_lamports;
    let player_key = ctx.accounts.player.key();

    // Effects: commit state before the CPI transfer
    ctx.accounts.game.player_two = player_key;
    ctx.accounts.game.state = GameState::Active;

    // Postcondition: game must now be Active with both players set
    require!(
        ctx.accounts.game.state == GameState::Active,
        CoordinationError::InvalidGameState
    );
    require!(
        ctx.accounts.game.player_two != Pubkey::default(),
        CoordinationError::InvalidGameState
    );

    // Capture values needed for the event before the CPI borrows accounts
    let game_id = ctx.accounts.game.game_id;
    let tournament_id = ctx.accounts.game.tournament_id;
    let player_one = ctx.accounts.game.player_one;

    // Interactions: transfer player 2 stake into the game PDA
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.player.to_account_info(),
                to: ctx.accounts.game.to_account_info(),
            },
        ),
        stake_lamports,
    )?;

    emit!(GameStarted {
        game_id,
        tournament_id,
        player_one,
        player_two: player_key,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct JoinGame<'info> {
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, Game>,
    #[account(
        init_if_needed,
        payer = player,
        space = PlayerProfile::SPACE,
        seeds = [
            b"player",
            tournament.tournament_id.to_le_bytes().as_ref(),
            player.key().as_ref(),
        ],
        bump,
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    #[account(
        seeds = [b"tournament", game.tournament_id.to_le_bytes().as_ref()],
        bump = tournament.bump,
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

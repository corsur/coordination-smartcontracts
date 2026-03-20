use crate::errors::CoordinationError;
use anchor_lang::prelude::*;

pub struct Resolution {
    pub p1_return: u64,
    pub p2_return: u64,
    pub tournament_gain: u64,
}

/// Computes payoffs for a homogenous matchup (v1 only — both players are human).
///
/// Correct guess = GUESS_HUMAN (0), since both players are human.
///
/// Payoffs:
///   Both correct: each receives 90% of stake; house takes 10% from each (20% total)
///   At least one wrong: both forfeit; house takes 100% from each (200% total)
///
/// Invariant: p1_return + p2_return + tournament_gain == 2 * stake_lamports
pub fn resolve_homogenous(p1_guess: u8, p2_guess: u8, stake_lamports: u64) -> Result<Resolution> {
    // Invariant: stake_lamports must be nonzero
    require!(stake_lamports > 0, CoordinationError::ArithmeticOverflow);

    let two_stakes = stake_lamports
        .checked_mul(2)
        .ok_or(CoordinationError::ArithmeticOverflow)?;

    let both_correct =
        p1_guess == crate::state::GUESS_HUMAN && p2_guess == crate::state::GUESS_HUMAN;

    if both_correct {
        // 90% return: stake * 9 / 10
        // Fee computed as remainder to avoid rounding loss
        let each_return = stake_lamports
            .checked_mul(9)
            .and_then(|v| v.checked_div(10))
            .ok_or(CoordinationError::ArithmeticOverflow)?;
        let fee = stake_lamports
            .checked_sub(each_return)
            .ok_or(CoordinationError::ArithmeticOverflow)?;
        let tournament_gain = fee
            .checked_mul(2)
            .ok_or(CoordinationError::ArithmeticOverflow)?;

        // Assert invariant
        let total = each_return
            .checked_add(each_return)
            .and_then(|v| v.checked_add(tournament_gain))
            .ok_or(CoordinationError::ArithmeticOverflow)?;
        require!(total == two_stakes, CoordinationError::ArithmeticOverflow);

        Ok(Resolution {
            p1_return: each_return,
            p2_return: each_return,
            tournament_gain,
        })
    } else {
        // Both forfeit
        Ok(Resolution {
            p1_return: 0,
            p2_return: 0,
            tournament_gain: two_stakes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GUESS_AI, GUESS_HUMAN};

    fn assert_invariant(r: &Resolution, stake: u64) {
        let total = r
            .p1_return
            .checked_add(r.p2_return)
            .unwrap()
            .checked_add(r.tournament_gain)
            .unwrap();
        assert_eq!(
            total,
            stake.checked_mul(2).unwrap(),
            "lamports must be conserved"
        );
    }

    #[test]
    fn both_correct_returns_90_percent() {
        let stake = 1_000_000; // 0.001 SOL
        let r = resolve_homogenous(GUESS_HUMAN, GUESS_HUMAN, stake).unwrap();
        assert_eq!(r.p1_return, 900_000);
        assert_eq!(r.p2_return, 900_000);
        assert_eq!(r.tournament_gain, 200_000);
        assert_invariant(&r, stake);
    }

    #[test]
    fn p1_wrong_both_forfeit() {
        let stake = 1_000_000;
        let r = resolve_homogenous(GUESS_AI, GUESS_HUMAN, stake).unwrap();
        assert_eq!(r.p1_return, 0);
        assert_eq!(r.p2_return, 0);
        assert_eq!(r.tournament_gain, 2_000_000);
        assert_invariant(&r, stake);
    }

    #[test]
    fn p2_wrong_both_forfeit() {
        let stake = 1_000_000;
        let r = resolve_homogenous(GUESS_HUMAN, GUESS_AI, stake).unwrap();
        assert_eq!(r.p1_return, 0);
        assert_eq!(r.p2_return, 0);
        assert_eq!(r.tournament_gain, 2_000_000);
        assert_invariant(&r, stake);
    }

    #[test]
    fn both_wrong_both_forfeit() {
        let stake = 1_000_000;
        let r = resolve_homogenous(GUESS_AI, GUESS_AI, stake).unwrap();
        assert_eq!(r.p1_return, 0);
        assert_eq!(r.p2_return, 0);
        assert_eq!(r.tournament_gain, 2_000_000);
        assert_invariant(&r, stake);
    }

    #[test]
    fn lamports_conserved_various_stakes() {
        for stake in [100, 999, 1_000_000, 10_000_000_000u64] {
            let r = resolve_homogenous(GUESS_HUMAN, GUESS_HUMAN, stake).unwrap();
            assert_invariant(&r, stake);
            let r2 = resolve_homogenous(GUESS_AI, GUESS_HUMAN, stake).unwrap();
            assert_invariant(&r2, stake);
        }
    }

    #[test]
    fn zero_stake_errors() {
        assert!(resolve_homogenous(GUESS_HUMAN, GUESS_HUMAN, 0).is_err());
    }
}

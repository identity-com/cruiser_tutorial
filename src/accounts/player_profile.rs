use cruiser::prelude::*;

/// A player's profile.
#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq, OnChainSize)]
pub struct PlayerProfile {
    /// The key allowed to act for this profile.
    pub authority: Pubkey,
    /// The number of wins this player has.
    pub wins: u64,
    /// The number of losses this player has.
    pub losses: u64,
    /// The number of draws this player has.
    pub draws: u64,
    /// The amount of lamports this player has won.
    pub lamports_won: u64,
    /// The amount of lamports this player has lost.
    pub lamports_lost: u64,
    /// The elo rating of the player.
    pub elo: u64,
}
impl PlayerProfile {
    /// The initial elo for a new profile.
    pub const INITIAL_ELO: u64 = 1200;

    /// Creates a new player profile.
    /// `authority` is a ref to a pubkey because it's more efficient to use a ref on-chain.
    pub fn new(authority: &Pubkey) -> Self {
        Self {
            authority: *authority,
            wins: 0,
            losses: 0,
            draws: 0,
            lamports_won: 0,
            lamports_lost: 0,
            elo: Self::INITIAL_ELO,
        }
    }
}

/// Probability of `elo_a` winning over `elo_b`.
fn win_probability(elo_a: f64, elo_b: f64) -> f64 {
    1.0 / (1.0 + 10.0_f64.powf((elo_b - elo_a) / 400.0))
}

/// Calculates the new elo of players after a game.
pub fn update_elo(elo_a: &mut u64, elo_b: &mut u64, k: f64, a_won: bool) {
    let mut elo_a_float = *elo_a as f64;
    let mut elo_b_float = *elo_b as f64;
    let a_prob = win_probability(elo_a_float, elo_b_float);
    let b_prob = win_probability(elo_b_float, elo_a_float);

    if a_won {
        elo_a_float += k * (1.0 - a_prob);
        elo_b_float += k * (0.0 - b_prob);
    } else {
        elo_a_float += k * (0.0 - a_prob);
        elo_b_float += k * (1.0 - b_prob);
    }

    *elo_a = elo_a_float as u64;
    *elo_b = elo_b_float as u64;
}

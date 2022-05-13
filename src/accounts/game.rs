use cruiser::prelude::*;

/// The game board.
#[derive(Debug, BorshDeserialize, BorshSerialize, Eq, PartialEq, OnChainSize)]
pub struct Game {
    /// The version of this account. Should always add this for future proofing.
    /// Should be 0 until a new version is added.
    pub version: u8,
    /// The first player's profile.
    pub player1: Pubkey,
    /// The second player's profile.
    pub player2: Pubkey,
    /// Which player was the creator and entitled to the rent.
    pub creator: Player,
    /// The player to take the next move.
    pub next_play: Player,
    /// The bump of the signer that holds the wager.
    pub signer_bump: u8,
    /// The wager per player in lamports.
    pub wager: u64,
    /// The amount of time in seconds to play a given turn before forfeiting.
    pub turn_length: UnixTimestamp,
    /// The last turn timestamp. If 0 game is not started.
    pub last_turn: UnixTimestamp,
    /// The last move a player did. If `[3,3]` last move is game start.
    pub last_move: [u8; 2],
    /// The current board. In RC format.
    pub board: Board<Board<Space>>,
}

impl Game {
    /// Creates a new game board.
    pub fn new(
        player_profile: &Pubkey,
        player: Player,
        signer_bump: u8,
        wager: u64,
        turn_length: UnixTimestamp,
    ) -> Self {
        Self {
            version: 0,

            player1: if player == Player::One {
                *player_profile
            } else {
                Pubkey::new_from_array([0; 32])
            },
            player2: if player == Player::Two {
                *player_profile
            } else {
                Pubkey::new_from_array([0; 32])
            },
            creator: player,
            next_play: Player::One,
            signer_bump,
            wager,
            turn_length,
            last_turn: 0,
            last_move: [3, 3],
            board: Default::default(),
        }
    }

    /// Tells whether the game has started.
    pub fn is_started(&self) -> bool {
        self.last_turn > 0
    }
}

/// A player
#[derive(Copy, Clone, Debug, BorshDeserialize, BorshSerialize, Eq, PartialEq, OnChainSize)]
pub enum Player {
    /// Player 1
    One,
    /// Player 2
    Two,
}

/// A space on the game board.
#[derive(Copy, Clone, Debug, BorshDeserialize, BorshSerialize, Eq, PartialEq, OnChainSize)]
pub enum Space {
    /// Player 1's space
    PlayerOne,
    /// Player 2's space
    PlayerTwo,
    /// Empty space
    Empty,
}
impl From<Player> for Space {
    fn from(player: Player) -> Self {
        match player {
            Player::One => Space::PlayerOne,
            Player::Two => Space::PlayerTwo,
        }
    }
}
impl Default for Space {
    fn default() -> Self {
        Space::Empty
    }
}

/// A sub-board. We use a generic for if we want to go crazy and add sub-sub boards!
#[derive(Copy, Clone, Debug, BorshDeserialize, BorshSerialize, Eq, PartialEq, OnChainSize)]
#[on_chain_size(generics = [where S: OnChainSize])]
pub enum Board<S> {
    /// Board has no winner yet. Board is in RC format.
    Unsolved([[S; 3]; 3]),
    /// Board has a winner
    Solved(Player),
}
impl<S> Default for Board<S>
where
    S: Default + Copy,
{
    fn default() -> Self {
        Board::Unsolved([[S::default(); 3]; 3])
    }
}

/// This trait lets us use the same logic for checking winners on the sub-boards and main board.
pub trait CurrentWinner {
    /// The index used to make a move.
    type Index;

    /// Gets the current player on the space.
    fn current_winner(&self) -> Option<Player>;

    /// Solves the current board to see if there is a winner.
    fn make_move(&mut self, player: Player, index: Self::Index) -> CruiserResult<()>;
}
impl CurrentWinner for Space {
    // A space is the lowest level and can't be further indexed.
    type Index = ();

    fn current_winner(&self) -> Option<Player> {
        match self {
            Space::PlayerOne => Some(Player::One),
            Space::PlayerTwo => Some(Player::Two),
            Space::Empty => None,
        }
    }

    fn make_move(&mut self, player: Player, _index: ()) -> CruiserResult<()> {
        *self = player.into();
        Ok(())
    }
}
impl<S> CurrentWinner for Board<S>
where
    S: CurrentWinner + Copy,
{
    /// We set the indexer to be our index + the sub-board index.
    type Index = ([u8; 2], S::Index);

    fn current_winner(&self) -> Option<Player> {
        match self {
            Board::Unsolved(_) => None,
            Board::Solved(player) => Some(*player),
        }
    }

    fn make_move(&mut self, player: Player, index: ([u8; 2], S::Index)) -> CruiserResult<()> {
        let (index, sub_index) = index;
        match self {
            Board::Unsolved(sub_board) => {
                // We make a move on the sub board.
                sub_board[index[0] as usize][index[1] as usize].make_move(player, sub_index)?;
                // Now we check if we are solved.
                if is_winner(sub_board, player) {
                    *self = Board::Solved(player);
                }
                Ok(())
            }
            Board::Solved(_) => {
                // Cannot make a move on a solved board.
                // We call `into` here to turn a generic error into the even more general `CruiserError`.
                // You would do the same with a custom error type.
                Err(GenericError::Custom {
                    error: "Cannot make move on solved board".to_string(),
                }
                .into())
            }
        }
    }
}

/// Gets the winner of a board. This could be a sub-board or the main board.
pub fn is_winner(board: &[[impl CurrentWinner + Copy; 3]; 3], last_turn: Player) -> bool {
    // Check rows
    if board.iter().any(|row| {
        row.iter()
            .map(CurrentWinner::current_winner)
            .all(|winner| winner.map(|winner| winner == last_turn).unwrap_or(false))
    }) {
        return true;
    }

    // Check columns
    'outer: for col in 0..board[0].len() {
        for row in board {
            if !matches!(row[col].current_winner(), Some(player) if player == last_turn) {
                continue 'outer;
            }
        }
        return true;
    }

    // Check diagonals
    let mut diagonal1 = 0;
    let mut diagonal2 = 0;
    for index in 0..board.len() {
        if matches!(board[index][index].current_winner(), Some(player) if player == last_turn) {
            diagonal1 += 1;
        }
        if matches!(
            board[index][board.len() - index - 1].current_winner(),
            Some(player) if player == last_turn
        ) {
            diagonal2 += 1;
        }
    }
    if diagonal1 == board.len() || diagonal2 == board.len() {
        return true;
    }

    false
}

#[cfg(test)]
mod test {
    use super::*;

    /// Simple test for our winner logic.
    #[test]
    fn test_get_winner() {
        let board = [
            [Space::PlayerOne, Space::PlayerOne, Space::PlayerOne],
            [Space::Empty, Space::PlayerTwo, Space::PlayerTwo],
            [Space::Empty, Space::Empty, Space::Empty],
        ];
        assert!(is_winner(&board, Player::One));
        let board = [
            [Space::PlayerTwo, Space::PlayerOne, Space::PlayerOne],
            [Space::Empty, Space::PlayerTwo, Space::PlayerTwo],
            [Space::Empty, Space::Empty, Space::PlayerTwo],
        ];
        assert!(is_winner(&board, Player::Two));
        let board = [
            [Space::PlayerTwo, Space::PlayerOne, Space::PlayerOne],
            [Space::Empty, Space::PlayerTwo, Space::PlayerOne],
            [Space::Empty, Space::Empty, Space::PlayerOne],
        ];
        assert!(is_winner(&board, Player::One));
        let board = [
            [Space::PlayerTwo, Space::PlayerOne, Space::PlayerOne],
            [Space::Empty, Space::PlayerTwo, Space::Empty],
            [Space::Empty, Space::Empty, Space::PlayerOne],
        ];
        assert!(!is_winner(&board, Player::One));
        assert!(!is_winner(&board, Player::Two));
    }
}

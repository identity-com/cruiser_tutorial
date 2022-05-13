#![warn(missing_docs, missing_debug_implementations)]

//! The tutorial example for cruiser.

pub mod accounts;
pub mod instructions;
pub mod pda;

use crate::accounts::{Game, PlayerProfile};
use cruiser::prelude::*;

// This uses your instruction list as the entrypoint to the program.
#[cfg(feature = "entrypoint")]
entrypoint_list!(TutorialInstructions, TutorialInstructions);

/// This is the list of instructions for your program, we will add more later.
///
/// The [`InstructionList`] trait defines a list of program instructions.
/// It takes an additional attribute to define which list of accounts
/// corresponds to this list and what type of account info it will use.
/// In this case we use a generic account info to support many cases.
/// This derive also implements [`InstructionListItem`] for each item
/// in the list and [`InstructionListProcessor`].
/// All these traits can be manually implemented if you need custom logic.
#[derive(Debug, InstructionList, Copy, Clone)]
#[instruction_list(
    account_list = TutorialAccounts,
    account_info = [<'a, AI> AI where AI: ToSolanaAccountInfo<'a>],
    discriminant_type = u8,
)]
pub enum TutorialInstructions {
    /// Creates a new player profile.
    #[instruction(instruction_type = instructions::CreateProfile)]
    CreateProfile,
    /// Create a new game.
    #[instruction(instruction_type = instructions::CreateGame)]
    CreateGame,
}

/// This is the list of accounts used by the program.
///
/// The [`AccountList`] trait defines a list of accounts for use by a program.
/// It is used to make sure no two accounts have the same discriminants.
/// This derive also implements [`AccountListItem`].
/// Both these traits can be manually implemented if you need custom logic.
#[derive(Debug, AccountList)]
pub enum TutorialAccounts {
    /// A game board
    GameBoard(Game),
    /// A player's profile
    PlayerProfile(PlayerProfile),
}

use crate::accounts::{CurrentWinner, Player, Space};
use crate::pda::GameSignerSeeder;
use crate::{Game, PlayerProfile, TutorialAccounts};
use cruiser::prelude::*;

/// Makes a move on the board and handles wins.
#[derive(Debug)]
pub enum MakeMove {}

impl<AI> Instruction<AI> for MakeMove {
    type Accounts = MakeMoveAccounts<AI>;
    type Data = MakeMoveData;
    type ReturnType = ();
}

/// Accounts for [`MakeMove`]
#[derive(AccountArgument, Debug)]
#[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
#[validate(data = (mov: MakeMoveData), custom = is_valid_move(&*self.game, &mov))]
pub struct MakeMoveAccounts<AI> {
    /// The authority for the player
    #[validate(signer)]
    pub authority: AI,
    /// The player to make a move for
    #[validate(writable, custom = &self.player_profile.authority == self.authority.key())]
    pub player_profile: DataAccount<AI, TutorialAccounts, PlayerProfile>,
    /// The game to make a move on.
    #[validate(
        writable,
        custom = self.game.is_started(),
        custom = match self.game.next_play {
            Player::One => &self.game.player1 == self.player_profile.info().key(),
            Player::Two => &self.game.player2 == self.player_profile.info().key(),
        },
    )]
    pub game: Box<DataAccount<AI, TutorialAccounts, Game>>,
    /// The signer for the game.
    /// Only needed if will win the game.
    #[validate(
        writable(IfSome),
        data = IfSomeArg((GameSignerSeeder{ game: *self.game.info().key() }, self.game.signer_bump)),
    )]
    pub game_signer: Option<Seeds<AI, GameSignerSeeder>>,
    /// The other player's profile.
    /// Only needed if will win the game.
    #[validate(
        writable(IfSome),
        custom = match (self.other_profile.as_ref(), self.game.next_play) {
            (Some(profile), Player::One) => &self.game.player2 == profile.info().key(),
            (Some(profile), Player::Two) => &self.game.player1 == profile.info().key(),
            _ => true,
        },
    )]
    pub other_profile: Option<DataAccount<AI, TutorialAccounts, PlayerProfile>>,
    /// Only needed if will win the game.
    #[validate(writable(IfSome))]
    pub funds_to: Option<AI>,
    /// Only needed if will win the game.
    pub system_program: Option<SystemProgram<AI>>,
}

/// Data for [`MakeMove`]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct MakeMoveData {
    /// Index on the big board
    pub big_board: [u8; 2],
    /// Index on the small board
    pub small_board: [u8; 2],
}

fn is_valid_move(game: &Game, mov: &MakeMoveData) -> bool {
    // Verify valid with last move
    (game.last_move == [3, 3]
        || game.board.get(game.last_move).map_or(false, |board| {
            board.current_winner().is_some() || mov.big_board == game.last_move
        }))
        && game
            .board
            .get(mov.big_board)
            .and_then(|board| {
                board
                    .get(mov.small_board)
                    .map(|space| space == &Space::Empty)
            })
            .unwrap_or(false)
}

#[cfg(feature = "processor")]
mod processor {
    use super::*;
    use crate::accounts::CurrentWinner;
    use cruiser::solana_program::clock::Clock;

    impl<'a, AI> InstructionProcessor<AI, MakeMove> for MakeMove
    where
        AI: ToSolanaAccountInfo<'a>,
    {
        type FromAccountsData = ();
        type ValidateData = MakeMoveData;
        type InstructionData = MakeMoveData;

        fn data_to_instruction_arg(
            data: <MakeMove as Instruction<AI>>::Data,
        ) -> CruiserResult<(
            Self::FromAccountsData,
            Self::ValidateData,
            Self::InstructionData,
        )> {
            Ok(((), data.clone(), data))
        }

        fn process(
            _program_id: &Pubkey,
            data: Self::InstructionData,
            accounts: &mut <MakeMove as Instruction<AI>>::Accounts,
        ) -> CruiserResult<<MakeMove as Instruction<AI>>::ReturnType> {
            let next_play = accounts.game.next_play;
            accounts
                .game
                .board
                .make_move(next_play, (data.big_board, (data.small_board, ())))?;

            if accounts.game.board.current_winner() == Some(accounts.game.next_play) {
                let game_signer = accounts.game_signer.as_mut().ok_or(GenericError::Custom {
                    error: "no game_signer on win".to_string(),
                })?;
                let other_profile =
                    accounts
                        .other_profile
                        .as_mut()
                        .ok_or(GenericError::Custom {
                            error: "no other_profile on win".to_string(),
                        })?;
                let funds_to = accounts.funds_to.as_ref().ok_or(GenericError::Custom {
                    error: "no funds_to on win".to_string(),
                })?;
                let system_program =
                    accounts
                        .system_program
                        .as_ref()
                        .ok_or(GenericError::Custom {
                            error: "no system_program on win".to_string(),
                        })?;

                let signer_seeds = game_signer.take_seed_set().unwrap();
                let winnings = *game_signer.lamports();

                system_program.transfer(
                    CPIChecked,
                    game_signer.info(),
                    funds_to,
                    winnings,
                    [&signer_seeds],
                )?;

                // Burn game data
                accounts.game.player1 = SystemProgram::<()>::KEY;
                accounts.game.player2 = SystemProgram::<()>::KEY;

                // Update profiles
                accounts.player_profile.wins.saturating_add_assign(1);
                other_profile.losses.saturating_add_assign(1);

                accounts
                    .player_profile
                    .lamports_won
                    .saturating_add_assign(winnings);
                other_profile.lamports_lost.saturating_add_assign(winnings);

                // Close game
                let mut game_lamports = game_signer.lamports_mut();
                *funds_to.lamports_mut() += *game_lamports;
                *game_lamports = 0;
            } else {
                accounts.game.next_play = match accounts.game.next_play {
                    Player::One => Player::Two,
                    Player::Two => Player::One,
                };

                accounts.game.last_turn = Clock::get()?.unix_timestamp;
                accounts.game.last_move = data.small_board;
            }

            Ok(())
        }
    }
}

#[cfg(feature = "cpi")]
pub use cpi::*;

/// CPI for [`MakeMove`]
#[cfg(feature = "cpi")]
mod cpi {
    use super::*;
    use crate::TutorialInstructions;

    /// Makes a move
    #[derive(Debug)]
    pub struct MakeMoveCPI<'a, AI, const N: usize> {
        accounts: [MaybeOwned<'a, AI>; N],
        data: Vec<u8>,
    }
    impl<'a, AI> MakeMoveCPI<'a, AI, 3> {
        /// Makes a move that won't win the game
        pub fn new(
            authority: impl Into<MaybeOwned<'a, AI>>,
            player_profile: impl Into<MaybeOwned<'a, AI>>,
            game: impl Into<MaybeOwned<'a, AI>>,
            make_move_data: MakeMoveData,
        ) -> CruiserResult<MakeMoveCPI<'a, AI, 3>> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<MakeMove>>::discriminant_compressed()
                .serialize(&mut data)?;
            make_move_data.serialize(&mut data)?;
            Ok(MakeMoveCPI {
                accounts: [authority.into(), player_profile.into(), game.into()],
                data,
            })
        }
    }
    impl<'a, AI> MakeMoveCPI<'a, AI, 7> {
        /// Makes a move that will win the game
        #[allow(clippy::too_many_arguments)]
        pub fn new_win(
            authority: impl Into<MaybeOwned<'a, AI>>,
            player_profile: impl Into<MaybeOwned<'a, AI>>,
            game: impl Into<MaybeOwned<'a, AI>>,
            game_signer: impl Into<MaybeOwned<'a, AI>>,
            other_profile: impl Into<MaybeOwned<'a, AI>>,
            funds_to: impl Into<MaybeOwned<'a, AI>>,
            system_program: impl Into<MaybeOwned<'a, AI>>,
            make_move_data: MakeMoveData,
        ) -> CruiserResult<MakeMoveCPI<'a, AI, 7>> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<MakeMove>>::discriminant_compressed()
                .serialize(&mut data)?;
            make_move_data.serialize(&mut data)?;
            Ok(MakeMoveCPI {
                accounts: [
                    authority.into(),
                    player_profile.into(),
                    game.into(),
                    game_signer.into(),
                    other_profile.into(),
                    funds_to.into(),
                    system_program.into(),
                ],
                data,
            })
        }
    }

    impl<'a, AI> CPIClientStatic<'a, 4> for MakeMoveCPI<'a, AI, 3>
    where
        AI: ToSolanaAccountMeta,
    {
        type InstructionList = TutorialInstructions;
        type Instruction = MakeMove;
        type AccountInfo = AI;

        fn instruction(
            self,
            program_account: impl Into<MaybeOwned<'a, Self::AccountInfo>>,
        ) -> InstructionAndAccounts<[MaybeOwned<'a, Self::AccountInfo>; 4]> {
            let program_account = program_account.into();
            let instruction = SolanaInstruction {
                program_id: *program_account.meta_key(),
                accounts: self
                    .accounts
                    .iter()
                    .map(MaybeOwned::as_ref)
                    .map(AI::to_solana_account_meta)
                    .collect(),
                data: self.data,
            };
            let mut accounts = self.accounts.into_iter();
            InstructionAndAccounts {
                instruction,
                accounts: [
                    accounts.next().unwrap(),
                    accounts.next().unwrap(),
                    accounts.next().unwrap(),
                    program_account,
                ],
            }
        }
    }

    impl<'a, AI> CPIClientStatic<'a, 8> for MakeMoveCPI<'a, AI, 7>
    where
        AI: ToSolanaAccountMeta,
    {
        type InstructionList = TutorialInstructions;
        type Instruction = MakeMove;
        type AccountInfo = AI;

        fn instruction(
            self,
            program_account: impl Into<MaybeOwned<'a, Self::AccountInfo>>,
        ) -> InstructionAndAccounts<[MaybeOwned<'a, Self::AccountInfo>; 8]> {
            let program_account = program_account.into();
            let instruction = SolanaInstruction {
                program_id: *program_account.meta_key(),
                accounts: self
                    .accounts
                    .iter()
                    .map(MaybeOwned::as_ref)
                    .map(AI::to_solana_account_meta)
                    .collect(),
                data: self.data,
            };
            let mut accounts = self.accounts.into_iter();
            InstructionAndAccounts {
                instruction,
                accounts: [
                    accounts.next().unwrap(),
                    accounts.next().unwrap(),
                    accounts.next().unwrap(),
                    accounts.next().unwrap(),
                    accounts.next().unwrap(),
                    accounts.next().unwrap(),
                    accounts.next().unwrap(),
                    program_account,
                ],
            }
        }
    }
}

#[cfg(feature = "client")]
pub use client::*;

/// Client for [`MakeMove`]
#[cfg(feature = "client")]
mod client {
    use super::*;

    /// Makes a non-winning move
    pub fn make_move<'a>(
        program_id: Pubkey,
        authority: impl Into<HashedSigner<'a>>,
        player_profile: Pubkey,
        game: Pubkey,
        move_data: MakeMoveData,
    ) -> InstructionSet<'a> {
        let authority = authority.into();
        InstructionSet {
            instructions: vec![
                MakeMoveCPI::new(
                    SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                    SolanaAccountMeta::new_readonly(player_profile, false),
                    SolanaAccountMeta::new(game, false),
                    move_data,
                )
                .unwrap()
                .instruction(SolanaAccountMeta::new_readonly(program_id, true))
                .instruction,
            ],
            signers: [authority].into_iter().collect(),
        }
    }

    /// Makes a winning move
    #[allow(clippy::too_many_arguments)]
    pub fn make_winning_move<'a>(
        program_id: Pubkey,
        authority: impl Into<HashedSigner<'a>>,
        player_profile: Pubkey,
        game: Pubkey,
        game_signer_bump: u8,
        other_profile: Pubkey,
        funds_to: Pubkey,
        move_data: MakeMoveData,
    ) -> InstructionSet<'a> {
        let authority = authority.into();
        InstructionSet {
            instructions: vec![
                MakeMoveCPI::new_win(
                    SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                    SolanaAccountMeta::new(player_profile, false),
                    SolanaAccountMeta::new(game, false),
                    SolanaAccountMeta::new(
                        GameSignerSeeder { game }
                            .create_address(&program_id, game_signer_bump)
                            .unwrap(),
                        false,
                    ),
                    SolanaAccountMeta::new(other_profile, false),
                    SolanaAccountMeta::new(funds_to, false),
                    SolanaAccountMeta::new_readonly(SystemProgram::<()>::KEY, false),
                    move_data,
                )
                .unwrap()
                .instruction(SolanaAccountMeta::new_readonly(program_id, true))
                .instruction,
            ],
            signers: [authority].into_iter().collect(),
        }
    }
}

use crate::pda::GameSignerSeeder;
use crate::{Game, PlayerProfile, TutorialAccounts};
use cruiser::prelude::*;

/// Joins an already created game.
#[derive(Debug)]
pub enum JoinGame {}

impl<AI> Instruction<AI> for JoinGame {
    type Accounts = JoinGameAccounts<AI>;
    type Data = JoinGameData;
    type ReturnType = ();
}

/// Accounts for [`JoinGame`]
#[derive(AccountArgument, Debug)]
#[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
#[validate(generics = [<'a> where AI: ToSolanaAccountInfo<'a>])]
pub struct JoinGameAccounts<AI> {
    /// The authority of the joiner
    #[validate(signer)]
    pub authority: AI,
    /// The profile of the joiner
    #[validate(custom = &self.player_profile.authority == self.authority.key())]
    pub player_profile: ReadOnlyDataAccount<AI, TutorialAccounts, PlayerProfile>,
    /// The game to join
    #[validate(
        writable,
        custom = !self.game.is_started(),
        custom = self.game.is_valid_other_player(self.player_profile.info().key()),
    )]
    pub game: DataAccount<AI, TutorialAccounts, Game>,
    /// The signer of the game
    #[validate(writable, data = (GameSignerSeeder{ game: *self.game.info().key() }, self.game.signer_bump))]
    pub game_signer: Seeds<AI, GameSignerSeeder>,
    /// The funder for the wager
    #[validate(signer, writable)]
    pub wager_funder: AI,
    /// The system program
    pub system_program: SystemProgram<AI>,
}

/// Data for [`JoinGame`]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct JoinGameData {}

#[cfg(feature = "processor")]
mod processor {
    use super::*;
    use crate::accounts::Player;
    use cruiser::solana_program::clock::Clock;
    use std::iter::empty;

    impl<'a, AI> InstructionProcessor<AI, JoinGame> for JoinGame
    where
        AI: ToSolanaAccountInfo<'a>,
    {
        type FromAccountsData = ();
        type ValidateData = ();
        type InstructionData = ();

        fn data_to_instruction_arg(
            _data: <JoinGame as Instruction<AI>>::Data,
        ) -> CruiserResult<(
            Self::FromAccountsData,
            Self::ValidateData,
            Self::InstructionData,
        )> {
            Ok(((), (), ()))
        }

        fn process(
            _program_id: &Pubkey,
            _data: Self::InstructionData,
            accounts: &mut <JoinGame as Instruction<AI>>::Accounts,
        ) -> CruiserResult<<JoinGame as Instruction<AI>>::ReturnType> {
            // Set the other player
            *match accounts.game.creator {
                Player::One => &mut accounts.game.player2,
                Player::Two => &mut accounts.game.player1,
            } = *accounts.player_profile.info().key();

            // Start the game by setting the timestamp
            accounts.game.last_turn = Clock::get()?.unix_timestamp;

            // Transfer the wager to the game
            accounts.system_program.transfer(
                CPIChecked,
                &accounts.wager_funder,
                accounts.game_signer.info(),
                accounts.game.wager,
                empty(),
            )?;

            Ok(())
        }
    }
}

#[cfg(feature = "cpi")]
pub use cpi::*;

/// CPI for [`JoinGame`]
#[cfg(feature = "cpi")]
mod cpi {
    use super::*;
    use crate::TutorialInstructions;

    /// CPI for [`JoinGame`]
    #[derive(Debug)]
    pub struct JoinGameCPI<'a, AI> {
        accounts: [MaybeOwned<'a, AI>; 6],
        data: Vec<u8>,
    }
    impl<'a, AI> JoinGameCPI<'a, AI> {
        /// Joins a game
        pub fn new(
            authority: impl Into<MaybeOwned<'a, AI>>,
            player_profile: impl Into<MaybeOwned<'a, AI>>,
            game: impl Into<MaybeOwned<'a, AI>>,
            game_signer: impl Into<MaybeOwned<'a, AI>>,
            wager_funder: impl Into<MaybeOwned<'a, AI>>,
            system_program: impl Into<MaybeOwned<'a, AI>>,
        ) -> CruiserResult<Self> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<JoinGame>>::discriminant_compressed()
                .serialize(&mut data)?;
            JoinGameData {}.serialize(&mut data)?;
            Ok(Self {
                accounts: [
                    authority.into(),
                    player_profile.into(),
                    game.into(),
                    game_signer.into(),
                    wager_funder.into(),
                    system_program.into(),
                ],
                data,
            })
        }
    }

    impl<'a, AI> CPIClientStatic<'a, 7> for JoinGameCPI<'a, AI>
    where
        AI: ToSolanaAccountMeta,
    {
        type InstructionList = TutorialInstructions;
        type Instruction = JoinGame;
        type AccountInfo = AI;

        fn instruction(
            self,
            program_account: impl Into<MaybeOwned<'a, Self::AccountInfo>>,
        ) -> InstructionAndAccounts<[MaybeOwned<'a, Self::AccountInfo>; 7]> {
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
                    program_account,
                ],
            }
        }
    }
}

#[cfg(feature = "client")]
pub use client::*;

/// Client for [`JoinGame`]
#[cfg(feature = "client")]
mod client {
    use super::*;

    /// Joins a game.
    pub fn join_game<'a>(
        program_id: Pubkey,
        authority: impl Into<HashedSigner<'a>>,
        player_profile: Pubkey,
        game: Pubkey,
        game_signer_bump: u8,
        wager_funder: impl Into<HashedSigner<'a>>,
    ) -> InstructionSet<'a> {
        let authority = authority.into();
        let wager_funder = wager_funder.into();
        InstructionSet {
            instructions: vec![
                JoinGameCPI::new(
                    SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                    SolanaAccountMeta::new_readonly(player_profile, false),
                    SolanaAccountMeta::new(game, false),
                    SolanaAccountMeta::new(
                        GameSignerSeeder { game }
                            .create_address(&program_id, game_signer_bump)
                            .unwrap(),
                        false,
                    ),
                    SolanaAccountMeta::new(wager_funder.pubkey(), true),
                    SolanaAccountMeta::new_readonly(SystemProgram::<()>::KEY, false),
                )
                .unwrap()
                .instruction(SolanaAccountMeta::new_readonly(program_id, false))
                .instruction,
            ],
            signers: [authority, wager_funder].into_iter().collect(),
        }
    }
}

use crate::accounts::Player;
use crate::pda::GameSignerSeeder;
use crate::{Game, PlayerProfile, TutorialAccounts};
use cruiser::prelude::*;
use cruiser::solana_program::clock::Clock;

/// Causes another player to forfeit the game if they run out of time for their turn.
#[derive(Debug)]
pub enum ForfeitGame {}

impl<AI> Instruction<AI> for ForfeitGame {
    type Accounts = ForfeitGameAccounts<AI>;
    type Data = ForfeitGameData;
    type ReturnType = ();
}

/// Accounts for [`ForfeitGame`]
#[derive(AccountArgument, Debug)]
#[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
#[validate(generics = [<'a> where AI: ToSolanaAccountInfo<'a>])]
pub struct ForfeitGameAccounts<AI> {
    /// The authority of the player calling the forfeit.
    #[validate(signer)]
    pub authority: AI,
    /// The profile of the calling player.
    #[validate(custom = &self.player_profile.authority == self.authority.key())]
    pub player_profile: DataAccount<AI, TutorialAccounts, PlayerProfile>,
    /// The other player's profile.
    pub other_profile: DataAccount<AI, TutorialAccounts, PlayerProfile>,
    /// The game the other player has forfeited.
    #[validate(
        custom = self.game.turn_length == 0
            || self.game.last_turn.saturating_add(self.game.turn_length) < Clock::get()?.unix_timestamp,
        custom = match self.game.next_play {
            Player::One => self.player_profile.info().key() == &self.game.player2,
            Player::Two => self.player_profile.info().key() == &self.game.player1,
        },
        custom = match self.game.next_play {
            Player::One => self.other_profile.info().key() == &self.game.player1,
            Player::Two => self.other_profile.info().key() == &self.game.player2,
        },
    )]
    pub game: CloseAccount<AI, DataAccount<AI, TutorialAccounts, Game>>,
    /// The game's signer.
    #[validate(writable, data = (GameSignerSeeder{ game: *self.game.info().key() }, self.game.signer_bump))]
    pub game_signer: Seeds<AI, GameSignerSeeder>,
    /// Where the funds should go to.
    #[validate(writable)]
    pub funds_to: AI,
    /// The system program
    pub system_program: SystemProgram<AI>,
}

/// Data for [`ForfeitGame`]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct ForfeitGameData {}

#[cfg(feature = "processor")]
mod processor {
    use super::*;
    use crate::accounts::update_elo;
    use std::iter::once;

    impl<'a, AI> InstructionProcessor<AI, ForfeitGame> for ForfeitGame
    where
        AI: ToSolanaAccountInfo<'a>,
    {
        type FromAccountsData = ();
        type ValidateData = ();
        type InstructionData = ();

        fn data_to_instruction_arg(
            _data: <ForfeitGame as Instruction<AI>>::Data,
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
            accounts: &mut <ForfeitGame as Instruction<AI>>::Accounts,
        ) -> CruiserResult<<ForfeitGame as Instruction<AI>>::ReturnType> {
            // Get the seeds out of the signer account
            let signer_seeds = accounts.game_signer.take_seed_set().unwrap();

            msg!("Transferring");
            // Need to separate this out because it will cause a borrow error if done in-line.
            // Can also be avoided with `CPIUnchecked`
            let transfer_amount = *accounts.game_signer.lamports();
            // Transfer wager to forfeit-eer
            accounts.system_program.transfer(
                CPIChecked,
                accounts.game_signer.info(),
                &accounts.funds_to,
                transfer_amount,
                once(&signer_seeds),
            )?;

            msg!("Setting fields");
            // Zero out the players so the game is dead.
            // We will close the game but this prevents it from being re-opened in the same transaction and still being useful.
            accounts.game.player1 = SystemProgram::<()>::KEY;
            accounts.game.player2 = SystemProgram::<()>::KEY;

            // Set who gets the funds on close
            accounts.game.set_fundee(accounts.funds_to.clone());

            accounts
                .player_profile
                .lamports_won
                .saturating_add_assign(accounts.game.wager);
            accounts.player_profile.wins.saturating_add_assign(1);

            accounts
                .other_profile
                .lamports_lost
                .saturating_add_assign(accounts.game.wager);
            accounts.other_profile.losses.saturating_add_assign(1);

            update_elo(
                &mut accounts.player_profile.elo,
                &mut accounts.other_profile.elo,
                50.0, // 50 for forfeits to discourage them
                true,
            );

            Ok(())
        }
    }
}

#[cfg(feature = "cpi")]
pub use cpi::*;

/// CPI for [`ForfeitGame`]
#[cfg(feature = "cpi")]
mod cpi {
    use super::*;
    use crate::TutorialInstructions;

    /// Forfiets another player from a game.
    #[derive(Debug)]
    pub struct ForfeitGameCPI<'a, AI> {
        accounts: [MaybeOwned<'a, AI>; 7],
        data: Vec<u8>,
    }
    impl<'a, AI> ForfeitGameCPI<'a, AI> {
        /// Forfiets another player from a game.
        pub fn new(
            authority: impl Into<MaybeOwned<'a, AI>>,
            player_profile: impl Into<MaybeOwned<'a, AI>>,
            other_profile: impl Into<MaybeOwned<'a, AI>>,
            game: impl Into<MaybeOwned<'a, AI>>,
            game_signer: impl Into<MaybeOwned<'a, AI>>,
            funds_to: impl Into<MaybeOwned<'a, AI>>,
            system_program: impl Into<MaybeOwned<'a, AI>>,
        ) -> CruiserResult<Self> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<ForfeitGame>>::discriminant_compressed()
                .serialize(&mut data)?;
            ForfeitGameData {}.serialize(&mut data)?;
            Ok(Self {
                accounts: [
                    authority.into(),
                    player_profile.into(),
                    other_profile.into(),
                    game.into(),
                    game_signer.into(),
                    funds_to.into(),
                    system_program.into(),
                ],
                data,
            })
        }
    }

    impl<'a, AI> CPIClientStatic<'a, 8> for ForfeitGameCPI<'a, AI>
    where
        AI: ToSolanaAccountMeta,
    {
        type InstructionList = TutorialInstructions;
        type Instruction = ForfeitGame;
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

/// Client for [`ForfeitGame`]
#[cfg(feature = "client")]
mod client {
    use super::*;

    /// Forfeits another player from a game.
    pub fn forfeit_game<'a>(
        program_id: Pubkey,
        authority: impl Into<HashedSigner<'a>>,
        player_profile: Pubkey,
        other_profile: Pubkey,
        game: Pubkey,
        game_signer_bump: u8,
        funds_to: Pubkey,
    ) -> InstructionSet<'a> {
        let authority = authority.into();
        InstructionSet {
            instructions: vec![
                ForfeitGameCPI::new(
                    SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                    SolanaAccountMeta::new(player_profile, false),
                    SolanaAccountMeta::new(other_profile, false),
                    SolanaAccountMeta::new(game, false),
                    SolanaAccountMeta::new(
                        GameSignerSeeder { game }
                            .create_address(&program_id, game_signer_bump)
                            .unwrap(),
                        false,
                    ),
                    SolanaAccountMeta::new(funds_to, false),
                    SolanaAccountMeta::new_readonly(SystemProgram::<()>::KEY, false),
                )
                .unwrap()
                .instruction(SolanaAccountMeta::new_readonly(program_id, false))
                .instruction,
            ],
            signers: [authority].into_iter().collect(),
        }
    }
}

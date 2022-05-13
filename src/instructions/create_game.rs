use crate::accounts::Player;
use crate::pda::GameSignerSeeder;
use crate::{Game, PlayerProfile, TutorialAccounts};
use cruiser::prelude::*;

/// Creates a new game.
#[derive(Debug)]
pub enum CreateGame {}

impl<AI> Instruction<AI> for CreateGame {
    type Accounts = CreateGameAccounts<AI>;
    type Data = CreateGameData;
    type ReturnType = ();
}

/// Accounts for [`CreateGame`]
#[derive(AccountArgument, Debug)]
#[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
#[from(
    data = (create_data: CreateGameData),
    custom = create_data.wager.checked_mul(2).is_some(),
)]
#[validate(generics = [<'a> where AI: ToSolanaAccountInfo<'a>])]
pub struct CreateGameAccounts<AI> {
    /// The authority for the creator's profile.
    #[validate(signer)]
    pub authority: AI,
    /// The creator's profile.
    #[validate(custom = &self.player_profile.authority == self.authority.key())]
    pub player_profile: ReadOnlyDataAccount<AI, TutorialAccounts, PlayerProfile>,
    /// The game to be created.
    #[from(data = Game::new(player_profile.info().key(), create_data.creator_player, create_data.signer_bump, create_data.wager, create_data.turn_length))]
    #[validate(data = InitArgs{
        system_program: Some(&self.system_program),
        space: InitStaticSized,
        funder: self.funder.as_ref(),
        funder_seeds: None,
        account_seeds: None,
        rent: None,
        cpi: CPIChecked,
    })]
    pub game: InitOrZeroedAccount<AI, TutorialAccounts, Game>,
    /// The game signer that will hold the wager.
    #[validate(writable, data = (GameSignerSeeder{ game: *self.game.info().key() }, self.game.signer_bump))]
    pub game_signer: Seeds<AI, GameSignerSeeder>,
    /// The funder that will put the creator's wager into the game.
    #[validate(signer, writable)]
    pub wager_funder: AI,
    /// The system program for transferring the wager and initializing the game if needed.
    pub system_program: SystemProgram<AI>,
    /// The funder for the game's rent. Only needed if not zeroed.
    #[from(data = game.is_init())]
    #[validate(signer(IfSome), writable(IfSome))]
    pub funder: Option<AI>,
    /// If [`Some`] locks other player to a given profile.
    pub other_player_profile: Option<ReadOnlyDataAccount<AI, TutorialAccounts, PlayerProfile>>,
}

/// Data for [`CreateGame`]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct CreateGameData {
    /// Which position the creator wants to play in.
    pub creator_player: Player,
    /// The bump for the game signer.
    pub signer_bump: u8,
    /// The wager each player will place. Winner gets double this amount.
    pub wager: u64,
    /// The length of time each player gets to play their turn. Starts once other player joins.
    pub turn_length: UnixTimestamp,
}

#[cfg(feature = "processor")]
mod processor {
    use super::*;
    use std::iter::empty;

    impl<'a, AI> InstructionProcessor<AI, CreateGame> for CreateGame
    where
        AI: ToSolanaAccountInfo<'a>,
    {
        type FromAccountsData = CreateGameData;
        type ValidateData = ();
        type InstructionData = CreateGameData;

        fn data_to_instruction_arg(
            data: <CreateGame as Instruction<AI>>::Data,
        ) -> CruiserResult<(
            Self::FromAccountsData,
            Self::ValidateData,
            Self::InstructionData,
        )> {
            Ok((data.clone(), (), data))
        }

        fn process(
            _program_id: &Pubkey,
            data: Self::InstructionData,
            accounts: &mut <CreateGame as Instruction<AI>>::Accounts,
        ) -> CruiserResult<<CreateGame as Instruction<AI>>::ReturnType> {
            // Transfer the wager from the wager_funder to the game signer.
            accounts.system_program.transfer(
                CPIChecked,
                &accounts.wager_funder,
                accounts.game_signer.info(),
                data.wager,
                empty(),
            )?;

            // Set the other player's profile if locked game.
            if let Some(other_player_profile) = &accounts.other_player_profile {
                *match data.creator_player {
                    Player::One => &mut accounts.game.player2,
                    Player::Two => &mut accounts.game.player1,
                } = *other_player_profile.info().key()
            }

            Ok(())
        }
    }
}

#[cfg(feature = "cpi")]
pub use cpi::*;
/// CPI for [`CreateGame`]
#[cfg(feature = "cpi")]
mod cpi {
    use super::*;
    use crate::TutorialInstructions;

    /// Creates a new game.
    #[derive(Debug)]
    pub struct CreateGameCPI<'a, AI, const N: usize> {
        accounts: [MaybeOwned<'a, AI>; N],
        data: Vec<u8>,
    }
    impl<'a, AI> CreateGameCPI<'a, AI, 6> {
        /// Creates a new game from a zeroed account.
        pub fn new_zeroed(
            authority: impl Into<MaybeOwned<'a, AI>>,
            player_profile: impl Into<MaybeOwned<'a, AI>>,
            game: impl Into<MaybeOwned<'a, AI>>,
            game_signer: impl Into<MaybeOwned<'a, AI>>,
            wager_funder: impl Into<MaybeOwned<'a, AI>>,
            system_program: impl Into<MaybeOwned<'a, AI>>,
            create_game_data: &CreateGameData,
        ) -> CruiserResult<Self> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<CreateGame>>::discriminant_compressed()
                .serialize(&mut data)?;
            create_game_data.serialize(&mut data)?;
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
    impl<'a, AI> CreateGameCPI<'a, AI, 7> {
        /// Creates a new game
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            authority: impl Into<MaybeOwned<'a, AI>>,
            player_profile: impl Into<MaybeOwned<'a, AI>>,
            game: impl Into<MaybeOwned<'a, AI>>,
            game_signer: impl Into<MaybeOwned<'a, AI>>,
            wager_funder: impl Into<MaybeOwned<'a, AI>>,
            system_program: impl Into<MaybeOwned<'a, AI>>,
            funder: impl Into<MaybeOwned<'a, AI>>,
            create_game_data: &CreateGameData,
        ) -> CruiserResult<Self> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<CreateGame>>::discriminant_compressed()
                .serialize(&mut data)?;
            create_game_data.serialize(&mut data)?;
            Ok(Self {
                accounts: [
                    authority.into(),
                    player_profile.into(),
                    game.into(),
                    game_signer.into(),
                    wager_funder.into(),
                    system_program.into(),
                    funder.into(),
                ],
                data,
            })
        }

        /// Creates a new game from a zeroed account and locked other player.
        #[allow(clippy::too_many_arguments)]
        pub fn new_zeroed_with_locked_player(
            authority: impl Into<MaybeOwned<'a, AI>>,
            player_profile: impl Into<MaybeOwned<'a, AI>>,
            game: impl Into<MaybeOwned<'a, AI>>,
            game_signer: impl Into<MaybeOwned<'a, AI>>,
            wager_funder: impl Into<MaybeOwned<'a, AI>>,
            system_program: impl Into<MaybeOwned<'a, AI>>,
            other_player_profile: impl Into<MaybeOwned<'a, AI>>,
            create_game_data: &CreateGameData,
        ) -> CruiserResult<Self> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<CreateGame>>::discriminant_compressed()
                .serialize(&mut data)?;
            create_game_data.serialize(&mut data)?;
            Ok(Self {
                accounts: [
                    authority.into(),
                    player_profile.into(),
                    game.into(),
                    game_signer.into(),
                    wager_funder.into(),
                    system_program.into(),
                    other_player_profile.into(),
                ],
                data,
            })
        }
    }
    impl<'a, AI> CreateGameCPI<'a, AI, 8> {
        /// Creates a new game with a locked other player.
        #[allow(clippy::too_many_arguments)]
        pub fn new_with_locked_player(
            authority: impl Into<MaybeOwned<'a, AI>>,
            player_profile: impl Into<MaybeOwned<'a, AI>>,
            game: impl Into<MaybeOwned<'a, AI>>,
            game_signer: impl Into<MaybeOwned<'a, AI>>,
            wager_funder: impl Into<MaybeOwned<'a, AI>>,
            system_program: impl Into<MaybeOwned<'a, AI>>,
            funder: impl Into<MaybeOwned<'a, AI>>,
            other_player_profile: impl Into<MaybeOwned<'a, AI>>,
            create_game_data: &CreateGameData,
        ) -> CruiserResult<Self> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<CreateGame>>::discriminant_compressed()
                .serialize(&mut data)?;
            create_game_data.serialize(&mut data)?;
            Ok(Self {
                accounts: [
                    authority.into(),
                    player_profile.into(),
                    game.into(),
                    game_signer.into(),
                    wager_funder.into(),
                    system_program.into(),
                    funder.into(),
                    other_player_profile.into(),
                ],
                data,
            })
        }
    }

    impl<'a, AI> CPIClientStatic<'a, 7> for CreateGameCPI<'a, AI, 6>
    where
        AI: ToSolanaAccountMeta,
    {
        type InstructionList = TutorialInstructions;
        type Instruction = CreateGame;
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
    impl<'a, AI> CPIClientStatic<'a, 8> for CreateGameCPI<'a, AI, 7>
    where
        AI: ToSolanaAccountMeta,
    {
        type InstructionList = TutorialInstructions;
        type Instruction = CreateGame;
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
    impl<'a, AI> CPIClientStatic<'a, 9> for CreateGameCPI<'a, AI, 8>
    where
        AI: ToSolanaAccountMeta,
    {
        type InstructionList = TutorialInstructions;
        type Instruction = CreateGame;
        type AccountInfo = AI;

        fn instruction(
            self,
            program_account: impl Into<MaybeOwned<'a, Self::AccountInfo>>,
        ) -> InstructionAndAccounts<[MaybeOwned<'a, Self::AccountInfo>; 9]> {
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
                    accounts.next().unwrap(),
                    program_account,
                ],
            }
        }
    }
}

#[cfg(feature = "client")]
pub use client::*;
/// Client for [`CreateGame`]
#[cfg(feature = "client")]
mod client {
    use super::*;
    use std::future::Future;

    /// Data for [`create_game`]
    #[derive(Clone, Debug)]
    pub struct CreateGameClientData {
        /// Which position the creator wants to play in.
        pub creator_player: Player,
        /// The wager each player will place. Winner gets double this amount.
        pub wager: u64,
        /// The length of time each player gets to play their turn. Starts once other player joins.
        pub turn_length: UnixTimestamp,
    }
    impl CreateGameClientData {
        /// Turns this into [`CreateGameData`]
        pub fn into_data(self, signer_bump: u8) -> CreateGameData {
            CreateGameData {
                creator_player: self.creator_player,
                wager: self.wager,
                turn_length: self.turn_length,
                signer_bump,
            }
        }
    }

    /// Creates a new game.
    #[allow(clippy::too_many_arguments)]
    pub fn create_game<'a>(
        program_id: Pubkey,
        authority: impl Into<HashedSigner<'a>>,
        player_profile: Pubkey,
        game: impl Into<HashedSigner<'a>>,
        wager_funder: impl Into<HashedSigner<'a>>,
        funder: impl Into<HashedSigner<'a>>,
        other_player_profile: Option<Pubkey>,
        data: CreateGameClientData,
    ) -> InstructionSet<'a> {
        let authority = authority.into();
        let game = game.into();
        let wager_funder = wager_funder.into();
        let funder = funder.into();

        let (game_signer, signer_bump) = GameSignerSeeder {
            game: game.pubkey(),
        }
        .find_address(&program_id);

        match other_player_profile {
            Some(other_player_profile) => InstructionSet {
                instructions: vec![
                    cpi::CreateGameCPI::new_with_locked_player(
                        SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                        SolanaAccountMeta::new(player_profile, false),
                        SolanaAccountMeta::new(game.pubkey(), true),
                        SolanaAccountMeta::new(game_signer, false),
                        SolanaAccountMeta::new(wager_funder.pubkey(), true),
                        SolanaAccountMeta::new_readonly(SystemProgram::<()>::KEY, false),
                        SolanaAccountMeta::new(funder.pubkey(), true),
                        SolanaAccountMeta::new_readonly(other_player_profile, false),
                        &data.into_data(signer_bump),
                    )
                    .unwrap()
                    .instruction(SolanaAccountMeta::new_readonly(program_id, false))
                    .instruction,
                ],
                signers: [authority, game, wager_funder, funder]
                    .into_iter()
                    .collect(),
            },
            None => InstructionSet {
                instructions: vec![
                    cpi::CreateGameCPI::new(
                        SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                        SolanaAccountMeta::new(player_profile, false),
                        SolanaAccountMeta::new(game.pubkey(), true),
                        SolanaAccountMeta::new(game_signer, false),
                        SolanaAccountMeta::new(wager_funder.pubkey(), true),
                        SolanaAccountMeta::new_readonly(SystemProgram::<()>::KEY, false),
                        SolanaAccountMeta::new(funder.pubkey(), true),
                        &data.into_data(signer_bump),
                    )
                    .unwrap()
                    .instruction(SolanaAccountMeta::new_readonly(program_id, false))
                    .instruction,
                ],
                signers: [authority, game, wager_funder, funder]
                    .into_iter()
                    .collect(),
            },
        }
    }

    /// Creates a new game from a zeroed account.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_game_zeroed<'a, F, E>(
        program_id: Pubkey,
        authority: impl Into<HashedSigner<'a>>,
        player_profile: Pubkey,
        game: impl Into<HashedSigner<'a>>,
        wager_funder: impl Into<HashedSigner<'a>>,
        funder: impl Into<HashedSigner<'a>>,
        other_player_profile: Option<Pubkey>,
        data: CreateGameClientData,
        rent: impl FnOnce(usize) -> F,
    ) -> Result<InstructionSet<'a>, E>
    where
        F: Future<Output = Result<u64, E>>,
    {
        let authority = authority.into();
        let game = game.into();
        let game_key = game.pubkey();
        let wager_funder = wager_funder.into();
        let funder = funder.into();

        let (game_signer, signer_bump) =
            GameSignerSeeder { game: game_key }.find_address(&program_id);

        let mut out = system_program::create_account(
            funder,
            game,
            rent(Game::ON_CHAIN_SIZE).await?,
            Game::ON_CHAIN_SIZE as u64,
            program_id,
        );
        out.add_set(match other_player_profile {
            Some(other_player_profile) => InstructionSet {
                instructions: vec![
                    cpi::CreateGameCPI::new_zeroed_with_locked_player(
                        SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                        SolanaAccountMeta::new(player_profile, false),
                        SolanaAccountMeta::new(game_key, false),
                        SolanaAccountMeta::new(game_signer, false),
                        SolanaAccountMeta::new(wager_funder.pubkey(), true),
                        SolanaAccountMeta::new_readonly(SystemProgram::<()>::KEY, false),
                        SolanaAccountMeta::new_readonly(other_player_profile, false),
                        &data.into_data(signer_bump),
                    )
                    .unwrap()
                    .instruction(SolanaAccountMeta::new_readonly(program_id, false))
                    .instruction,
                ],
                signers: [authority, wager_funder].into_iter().collect(),
            },
            None => InstructionSet {
                instructions: vec![
                    cpi::CreateGameCPI::new_zeroed(
                        SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                        SolanaAccountMeta::new(player_profile, false),
                        SolanaAccountMeta::new(game_key, false),
                        SolanaAccountMeta::new(game_signer, false),
                        SolanaAccountMeta::new(wager_funder.pubkey(), true),
                        SolanaAccountMeta::new_readonly(SystemProgram::<()>::KEY, false),
                        &data.into_data(signer_bump),
                    )
                    .unwrap()
                    .instruction(SolanaAccountMeta::new_readonly(program_id, false))
                    .instruction,
                ],
                signers: [authority, wager_funder].into_iter().collect(),
            },
        });
        Ok(out)
    }
}

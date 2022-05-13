use crate::{PlayerProfile, TutorialAccounts};
use cruiser::prelude::*;

/// Creates a new player profile.
#[derive(Debug)]
pub enum CreateProfile {}

impl<AI> Instruction<AI> for CreateProfile {
    type Accounts = CreateProfileAccounts<AI>;
    type Data = CreateProfileData;
    type ReturnType = ();
}

/// Accounts for [`CreateProfile`]
#[derive(AccountArgument, Debug)]
#[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
#[validate(generics = [<'a> where AI: ToSolanaAccountInfo<'a>])]
pub struct CreateProfileAccounts<AI> {
    /// The authority for the new profile.
    #[validate(signer)]
    pub authority: AI,
    /// The new profile to create
    #[from(data = PlayerProfile::new(authority.key()))] // This is where we set the initial value of the profile
    #[validate(data = InitArgs{
        system_program: &self.system_program,
        space: InitStaticSized,
        funder: &self.funder,
        funder_seeds: None,
        account_seeds: None,
        rent: None,
        cpi: CPIChecked,
    })]
    pub profile: InitAccount<AI, TutorialAccounts, PlayerProfile>,
    /// The funder for the new account. Needed if the account is not zeroed.
    #[validate(signer, writable)]
    pub funder: AI,
    /// The system program. Needed if the account is not zeroed.
    pub system_program: SystemProgram<AI>,
}

/// Data for [`CreateProfile`]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct CreateProfileData {}

#[cfg(feature = "processor")]
mod processor {
    use super::*;

    impl<'a, AI> InstructionProcessor<AI, CreateProfile> for CreateProfile
    where
        AI: ToSolanaAccountInfo<'a>,
    {
        type FromAccountsData = ();
        type ValidateData = ();
        type InstructionData = ();

        fn data_to_instruction_arg(
            _data: <CreateProfile as Instruction<AI>>::Data,
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
            _accounts: &mut <CreateProfile as Instruction<AI>>::Accounts,
        ) -> CruiserResult<<CreateProfile as Instruction<AI>>::ReturnType> {
            // We don't need any processing here, all initialization is handled in the accounts.
            // You could do some data setting here but we handled that with the profile init.
            Ok(())
        }
    }
}

#[cfg(feature = "cpi")]
pub use cpi::*;
/// CPI types for [`CreateProfile`]
#[cfg(feature = "cpi")] // We don't need this code when compiling our program for deployment
pub mod cpi {
    use super::*;
    use crate::TutorialInstructions;

    /// Creates a new player profile.
    #[derive(Debug)]
    pub struct CreateProfileCPI<'a, AI> {
        accounts: [MaybeOwned<'a, AI>; 4],
        data: Vec<u8>,
    }
    impl<'a, AI> CreateProfileCPI<'a, AI> {
        /// Creates a new player profile.
        pub fn new(
            authority: impl Into<MaybeOwned<'a, AI>>,
            profile: impl Into<MaybeOwned<'a, AI>>,
            funder: impl Into<MaybeOwned<'a, AI>>,
            system_program: impl Into<MaybeOwned<'a, AI>>,
        ) -> CruiserResult<Self> {
            let mut data = Vec::new();
            <TutorialInstructions as InstructionListItem<CreateProfile>>::discriminant_compressed()
                .serialize(&mut data)?;
            // This will do nothing but throw an error if we update this to include more data.
            CreateProfileData {}.serialize(&mut data)?;
            Ok(Self {
                accounts: [
                    authority.into(),
                    profile.into(),
                    funder.into(),
                    system_program.into(),
                ],
                data,
            })
        }
    }

    impl<'a, AI> CPIClientStatic<'a, 5> for CreateProfileCPI<'a, AI>
    where
        AI: ToSolanaAccountMeta,
    {
        type InstructionList = TutorialInstructions;
        type Instruction = CreateProfile;
        type AccountInfo = AI;

        fn instruction(
            self,
            program_account: impl Into<MaybeOwned<'a, Self::AccountInfo>>,
        ) -> InstructionAndAccounts<[MaybeOwned<'a, Self::AccountInfo>; 5]> {
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
                    program_account,
                ],
            }
        }
    }
}

#[cfg(feature = "client")]
pub use client::*;
/// Client functions for [`CreateProfile`]
#[cfg(feature = "client")]
mod client {
    use super::*;

    /// Creates a new player profile.
    pub fn create_profile<'a>(
        program_id: Pubkey,
        authority: impl Into<HashedSigner<'a>>,
        profile: impl Into<HashedSigner<'a>>,
        funder: impl Into<HashedSigner<'a>>,
    ) -> InstructionSet<'a> {
        let authority = authority.into();
        let profile = profile.into();
        let funder = funder.into();
        InstructionSet {
            instructions: vec![
                CreateProfileCPI::new(
                    SolanaAccountMeta::new_readonly(authority.pubkey(), true),
                    SolanaAccountMeta::new(profile.pubkey(), true),
                    SolanaAccountMeta::new(funder.pubkey(), true),
                    SolanaAccountMeta::new_readonly(SystemProgram::<()>::KEY, false),
                )
                .unwrap()
                .instruction(SolanaAccountMeta::new_readonly(program_id, false))
                .instruction,
            ],
            signers: [authority, profile, funder].into_iter().collect(),
        }
    }
}

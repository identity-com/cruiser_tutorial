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
pub struct JoinGameAccounts<AI> {
    #[validate(signer)]
    authority: AI,
    #[validate(custom = !self.player_profile.is_started())]
    player_profile: ReadOnlyDataAccount<AI, TutorialAccounts, Game>,
}

/// Data for [`JoinGame`]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct JoinGameData {}

#[cfg(feature = "processor")]
mod processor {
    use super::*;

    impl<AI> InstructionProcessor<AI, JoinGame> for JoinGame
    where
        AI: AccountInfo,
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
            todo!()
        }

        fn process(
            _program_id: &Pubkey,
            _data: Self::InstructionData,
            _accounts: &mut <JoinGame as Instruction<AI>>::Accounts,
        ) -> CruiserResult<<JoinGame as Instruction<AI>>::ReturnType> {
            todo!()
        }
    }
}

#[cfg(feature = "cpi")]
pub use cpi::*;

/// CPI for [`JoinGame`]
#[cfg(feature = "cpi")]
mod cpi {
    use super::*;

    #[derive(Debug)]
    pub struct JoinGameCPI<'a, AI, const N: usize> {
        accounts: [MaybeOwned<'a, AI>; N],
        data: Vec<u8>,
    }
}

use crate::{Game, TutorialAccounts};
#[cfg(feature = "client")]
pub use client::*;

/// Client for [`JoinGame`]
#[cfg(feature = "client")]
mod client {
    use super::*;
}

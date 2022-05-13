#![allow(dead_code)]
use cruiser::prelude::*;

mod derive {
    use super::*;

    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
    pub struct CustomAccountArgument<AI> {
        pub account1: AI,
        pub account2: AI,
        pub account3: AI,
    }

    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
    pub struct InstructionAccounts<AI> {
        pub funder: AI,
        pub custom: CustomAccountArgument<AI>,
        pub optional_custom: Option<CustomAccountArgument<AI>>,
    }
}

mod derive_with_data {
    use super::*;

    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
    #[validate(data = (account_2_owner: &Pubkey))]
    pub struct CustomAccountArgument<AI> {
        pub account1: AI,
        #[validate(owner = account_2_owner)]
        pub account2: AI,
        pub account3: AI,
    }

    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
    pub struct InstructionAccounts<AI> {
        pub funder: AI,
        #[validate(data = &SystemProgram::<()>::KEY)]
        pub custom: CustomAccountArgument<AI>,
        #[validate(data = IfSomeArg(&SystemProgram::<()>::KEY))]
        pub optional_custom: Option<CustomAccountArgument<AI>>,
    }
}

mod derive_with_multiple_data_options {
    use super::*;

    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
    #[validate(data = (account_2_owner: &Pubkey))] // No ID is a unique id
    // implementations are differentiated by types, id is only for deriving
    #[validate(id = account1, data = (account_1_owner: &Pubkey, _null: ()))]
    // If `validate` attr exists the default `()` impl is not generated.
    // Add an id with no data to get it back.
    #[validate(id = unit)]
    pub struct CustomAccountArgument<AI> {
        #[validate(id = account1, owner = account_1_owner)]
        pub account1: AI,
        #[validate(owner = account_2_owner)]
        pub account2: AI,
        #[validate(id = unit, owner = &SystemProgram::<()>::KEY)]
        pub account3: AI,
    }

    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
    pub struct InstructionAccounts<AI> {
        pub funder: AI,
        #[validate(data = &SystemProgram::<()>::KEY)]
        pub custom: CustomAccountArgument<AI>,
        #[validate(data = IfSomeArg(&SystemProgram::<()>::KEY))]
        pub optional_custom: Option<CustomAccountArgument<AI>>,
    }
}

mod single_indexable_and_multi_indexable {
    use super::*;
    use cruiser::AllAny;

    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
    pub struct CustomAccountArgument<AI> {
        pub account1: AI,
        pub account2: AI,
        pub account3: AI,
    }
    // These are meant for demonstration, it's usually useless to add these.
    #[derive(Copy, Clone, Debug)]
    pub enum CustomAccountArgumentSubset {
        Accounts12,
        Accounts23,
        Accounts31,
    }
    impl CustomAccountArgumentSubset {
        fn get_subset<AI>(self, argument: &CustomAccountArgument<AI>) -> [&AI; 2] {
            match self {
                CustomAccountArgumentSubset::Accounts12 => [&argument.account1, &argument.account2],
                CustomAccountArgumentSubset::Accounts23 => [&argument.account2, &argument.account3],
                CustomAccountArgumentSubset::Accounts31 => [&argument.account3, &argument.account1],
            }
        }
    }
    #[derive(Copy, Clone, Debug)]
    pub enum CustomAccountArgumentIndex {
        Account1,
        Account2,
        Account3,
    }
    impl CustomAccountArgumentIndex {
        fn get_index<AI>(self, argument: &CustomAccountArgument<AI>) -> &AI {
            match self {
                CustomAccountArgumentIndex::Account1 => &argument.account1,
                CustomAccountArgumentIndex::Account2 => &argument.account2,
                CustomAccountArgumentIndex::Account3 => &argument.account3,
            }
        }
    }

    impl<AI> CustomAccountArgument<AI> {
        fn to_iter(&self) -> impl Iterator<Item = &AI> {
            [&self.account1, &self.account2, &self.account3].into_iter()
        }
    }
    // `AllAny` allows us to use `signer(all)` and `signer(any)` in the `validate` attr.
    // `not_all` and `not_any` are also available.
    impl<AI> MultiIndexable<AllAny> for CustomAccountArgument<AI>
    where
        AI: AccountInfo,
    {
        fn index_is_signer(&self, indexer: AllAny) -> CruiserResult<bool> {
            indexer.run_func(self.to_iter(), |account| account.index_is_signer(()))
        }

        fn index_is_writable(&self, indexer: AllAny) -> CruiserResult<bool> {
            indexer.run_func(self.to_iter(), |account| account.index_is_writable(()))
        }

        fn index_is_owner(&self, owner: &Pubkey, indexer: AllAny) -> CruiserResult<bool> {
            indexer.run_func(self.to_iter(), |account| account.index_is_owner(owner, ()))
        }
    }
    impl<AI> MultiIndexable<CustomAccountArgumentSubset> for CustomAccountArgument<AI>
    where
        AI: AccountInfo,
    {
        fn index_is_signer(&self, indexer: CustomAccountArgumentSubset) -> CruiserResult<bool> {
            indexer
                .get_subset(self)
                .into_iter()
                .map(|account| account.index_is_signer(()))
                .fold(Ok(true), |acc, res| match acc {
                    Ok(acc) => Ok(acc && res?),
                    Err(e) => Err(e),
                })
        }

        fn index_is_writable(&self, indexer: CustomAccountArgumentSubset) -> CruiserResult<bool> {
            indexer
                .get_subset(self)
                .into_iter()
                .map(|account| account.index_is_writable(()))
                .fold(Ok(true), |acc, res| match acc {
                    Ok(acc) => Ok(acc && res?),
                    Err(e) => Err(e),
                })
        }

        fn index_is_owner(
            &self,
            owner: &Pubkey,
            indexer: CustomAccountArgumentSubset,
        ) -> CruiserResult<bool> {
            indexer
                .get_subset(self)
                .into_iter()
                .map(|account| account.index_is_owner(owner, ()))
                .fold(Ok(true), |acc, res| match acc {
                    Ok(acc) => Ok(acc && res?),
                    Err(e) => Err(e),
                })
        }
    }
    // We need `MultiIndexable<$ty>` to implement `SingleIndexable<$ty>`.
    impl<AI> MultiIndexable<CustomAccountArgumentIndex> for CustomAccountArgument<AI>
    where
        AI: AccountInfo,
    {
        fn index_is_signer(&self, indexer: CustomAccountArgumentIndex) -> CruiserResult<bool> {
            indexer.get_index(self).index_is_signer(())
        }

        fn index_is_writable(&self, indexer: CustomAccountArgumentIndex) -> CruiserResult<bool> {
            indexer.get_index(self).index_is_writable(())
        }

        fn index_is_owner(
            &self,
            owner: &Pubkey,
            indexer: CustomAccountArgumentIndex,
        ) -> CruiserResult<bool> {
            indexer.get_index(self).index_is_owner(owner, ())
        }
    }
    impl<AI> SingleIndexable<CustomAccountArgumentIndex> for CustomAccountArgument<AI>
    where
        AI: AccountInfo,
    {
        fn index_info(
            &self,
            indexer: CustomAccountArgumentIndex,
        ) -> CruiserResult<&Self::AccountInfo> {
            indexer.get_index(self).index_info(())
        }
    }

    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo])]
    pub struct InstructionAccounts<AI> {
        pub funder: AI,
        #[validate(
            key(CustomAccountArgumentIndex::Account1) = &SystemProgram::<()>::KEY,
            writable(any),
        )]
        pub custom: CustomAccountArgument<AI>,
        #[validate(signer(IfSomeArg(CustomAccountArgumentSubset::Accounts31)))]
        pub optional_custom: Option<CustomAccountArgument<AI>>,
    }
}

mod manual_implementation {
    use super::*;

    #[derive(AccountList)]
    pub enum RingAccounts {
        RingAccount(RingAccount),
    }

    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct RingAccount {
        next: Pubkey,
    }

    pub struct RingAccountArgument<AI> {
        accounts: Vec<ReadOnlyDataAccount<AI, RingAccounts, RingAccount>>,
    }

    // For `AccountArgument` and `ValidateArgument` we are delegating to `Vec`'s implementation.
    // Since we are doing that we can also use the derive for only those traits.
    #[derive(AccountArgument)]
    #[account_argument(account_info = AI, generics = [where AI: AccountInfo], no_from)]
    #[validate(
        no_single_tupple,
        data = (arg: Arg),
        generics = [<Arg> where Vec<ReadOnlyDataAccount<AI, RingAccounts, RingAccount>>: ValidateArgument<Arg>],
    )]
    pub struct RingAccountArgument2<AI> {
        #[validate(data = arg)]
        accounts: Vec<ReadOnlyDataAccount<AI, RingAccounts, RingAccount>>,
    }

    // Only needed for `RingAccountArgument`
    impl<AI> AccountArgument for RingAccountArgument<AI>
    where
        AI: AccountInfo,
    {
        type AccountInfo = AI;

        fn write_back(self, program_id: &Pubkey) -> CruiserResult<()> {
            // Delegate to `Vec`'s implementation
            self.accounts.write_back(program_id)
        }

        fn add_keys(&self, add: impl FnMut(Pubkey) -> CruiserResult<()>) -> CruiserResult<()> {
            // Delegate to `Vec`'s implementation
            self.accounts.add_keys(add)
        }
    }

    // Needed for both `RingAccountArgument` and `RingAccountArgument2`
    impl<AI> FromAccounts for RingAccountArgument<AI>
    where
        AI: AccountInfo,
    {
        fn from_accounts(
            program_id: &Pubkey,
            infos: &mut impl AccountInfoIterator<Item = Self::AccountInfo>,
            _arg: (),
        ) -> CruiserResult<Self> {
            let mut out = Vec::new();
            let first: ReadOnlyDataAccount<_, _, RingAccount> =
                FromAccounts::from_accounts(program_id, infos, ())?;
            let first_key = *first.info().key();
            out.push(first);
            let mut next_key = &out[0].next;

            loop {
                let next = ReadOnlyDataAccount::from_accounts(program_id, infos, ())?;
                assert_is_key(&next, next_key, ())?;
                out.push(next);
                next_key = &out[out.len() - 1].next;
                if next_key == &first_key {
                    break;
                }
            }

            Ok(Self { accounts: out })
        }

        fn accounts_usage_hint(_arg: &()) -> (usize, Option<usize>) {
            (2, None)
        }
    }

    // Only needed for `RingAccountArgument`
    impl<AI, Arg> ValidateArgument<Arg> for RingAccountArgument<AI>
    where
        AI: AccountInfo,
        Vec<ReadOnlyDataAccount<AI, RingAccounts, RingAccount>>: ValidateArgument<Arg>,
    {
        fn validate(&mut self, program_id: &Pubkey, arg: Arg) -> CruiserResult<()> {
            // Delegate to `Vec`'s implementation
            self.accounts.validate(program_id, arg)
        }
    }

    impl<AI, Arg> MultiIndexable<Arg> for RingAccountArgument<AI>
    where
        AI: AccountInfo,
        Vec<ReadOnlyDataAccount<AI, RingAccounts, RingAccount>>: MultiIndexable<Arg>,
    {
        fn index_is_signer(&self, indexer: Arg) -> CruiserResult<bool> {
            self.accounts.index_is_signer(indexer)
        }

        fn index_is_writable(&self, indexer: Arg) -> CruiserResult<bool> {
            self.accounts.index_is_writable(indexer)
        }

        fn index_is_owner(&self, owner: &Pubkey, indexer: Arg) -> CruiserResult<bool> {
            self.accounts.index_is_owner(owner, indexer)
        }
    }
    impl<AI, Arg> SingleIndexable<Arg> for RingAccountArgument<AI>
    where
        AI: AccountInfo,
        Vec<ReadOnlyDataAccount<AI, RingAccounts, RingAccount>>:
            SingleIndexable<Arg, AccountInfo = Self::AccountInfo>,
    {
        fn index_info(&self, indexer: Arg) -> CruiserResult<&Self::AccountInfo> {
            self.accounts.index_info(indexer)
        }
    }
}

fn main() {}

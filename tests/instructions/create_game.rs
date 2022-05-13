use crate::instructions::setup_validator;
use cruiser::prelude::*;
use cruiser_tutorial::accounts::{Game, Player};
use cruiser_tutorial::instructions::{create_game, create_profile, CreateGameClientData};
use cruiser_tutorial::TutorialAccounts;
use std::error::Error;
use std::time::Duration;

#[tokio::test]
async fn create_game_test() -> Result<(), Box<dyn Error>> {
    let guard = setup_validator().await;

    let rpc = guard.rpc();
    let funder = Keypair::new();

    // Airdrop SOL to the funder
    let blockhash = rpc.get_latest_blockhash().await?;
    let sig = rpc
        .request_airdrop_with_blockhash(&funder.pubkey(), LAMPORTS_PER_SOL * 10, &blockhash)
        .await?;
    rpc.confirm_transaction_with_spinner(&sig, &blockhash, CommitmentConfig::confirmed())
        .await?;

    // Create random authority and profile
    let authority = Keypair::new();
    let profile = Keypair::new();
    let game = Keypair::new();

    // Send transaction
    let (sig, result) = TransactionBuilder::new(&funder)
        .signed_instructions(create_profile(
            guard.program_id(),
            &authority,
            &profile,
            &funder,
        ))
        .signed_instructions(create_game(
            guard.program_id(),
            &authority,
            profile.pubkey(),
            &game,
            &funder,
            &funder,
            None,
            CreateGameClientData {
                creator_player: Player::One,
                wager: LAMPORTS_PER_SOL,
                turn_length: 60 * 60 * 24, // 1 day
            },
        ))
        .send_and_confirm_transaction(
            rpc,
            RpcSendTransactionConfig {
                skip_preflight: false,
                preflight_commitment: Some(CommitmentLevel::Confirmed),
                encoding: None,
                max_retries: None,
            },
            CommitmentConfig::confirmed(),
            Duration::from_millis(500),
        )
        .await?;

    // Check result
    match result {
        ConfirmationResult::Success => {}
        ConfirmationResult::Failure(error) => return Err(error.into()),
        ConfirmationResult::Dropped => return Err("Transaction dropped".into()),
    }

    // Print logs for debugging
    println!(
        "Logs: {:#?}",
        rpc.get_transaction_with_config(
            &sig,
            RpcTransactionConfig {
                encoding: None,
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: None
            }
        )
        .await?
        .transaction
        .meta
        .unwrap()
        .log_messages
    );

    // Check account data is what we expect
    let account = rpc
        .get_account_with_commitment(&game.pubkey(), CommitmentConfig::confirmed())
        .await?
        .value
        .unwrap_or_else(|| {
            panic!("Account not found");
        });
    let mut data = account.data.as_slice();
    let discriminant =
        <TutorialAccounts as AccountList>::DiscriminantCompressed::deserialize(&mut data)?;
    assert_eq!(
        discriminant,
        <TutorialAccounts as AccountListItem<Game>>::compressed_discriminant()
    );
    let board = Game::deserialize(&mut data)?;
    assert_eq!(
        board,
        Game::new(
            &profile.pubkey(),
            Player::One,
            board.signer_bump,
            LAMPORTS_PER_SOL,
            60 * 60 * 24
        )
    );

    guard.drop_self().await;
    Ok(())
}

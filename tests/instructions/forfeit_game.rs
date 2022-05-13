use crate::instructions::setup_validator;
use cruiser::prelude::*;
use cruiser_tutorial::accounts::Player;
use cruiser_tutorial::instructions::*;
use cruiser_tutorial::pda::GameSignerSeeder;
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn forfeit_game_test() -> Result<(), Box<dyn Error>> {
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
    let authority1 = Keypair::new();
    let profile1 = Keypair::new();
    let authority2 = Keypair::new();
    let profile2 = Keypair::new();
    let game = Keypair::new();
    let game_signer_bump = GameSignerSeeder {
        game: game.pubkey(),
    }
    .find_address(&guard.program_id())
    .1;

    let (sig, result) = TransactionBuilder::new(&funder)
        .signed_instructions(create_profile(
            guard.program_id(),
            &authority1,
            &profile1,
            &funder,
        ))
        .signed_instructions(create_profile(
            guard.program_id(),
            &authority2,
            &profile2,
            &funder,
        ))
        .signed_instructions(create_game(
            guard.program_id(),
            &authority1,
            profile1.pubkey(),
            &game,
            &funder,
            &funder,
            Some(profile2.pubkey()),
            CreateGameClientData {
                creator_player: Player::One,
                wager: LAMPORTS_PER_SOL,
                turn_length: 1, // 1 second
            },
        ))
        .signed_instructions(join_game(
            guard.program_id(),
            &authority2,
            profile2.pubkey(),
            game.pubkey(),
            game_signer_bump,
            &funder,
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

    // Wait for game to timeout
    sleep(Duration::from_millis(1500)).await;

    let receiver = Keypair::new().pubkey();

    let (sig, result) = TransactionBuilder::new(&funder)
        .signed_instructions(forfeit_game(
            guard.program_id(),
            &authority2,
            profile2.pubkey(),
            profile1.pubkey(),
            game.pubkey(),
            game_signer_bump,
            receiver,
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
            Duration::from_millis(501),
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

    let accounts = rpc
        .get_multiple_accounts_with_commitment(
            &[game.pubkey(), receiver],
            CommitmentConfig::confirmed(),
        )
        .await?
        .value;
    if let Some(game) = &accounts[0] {
        assert_eq!(game.lamports, 0);
        assert_eq!(game.owner, SystemProgram::<()>::KEY);
    }
    let receiver = accounts[1].as_ref().unwrap();
    assert!(receiver.lamports > LAMPORTS_PER_SOL * 2);

    guard.drop_self().await;
    Ok(())
}

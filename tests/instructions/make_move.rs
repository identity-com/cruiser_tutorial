use crate::instructions::setup_validator;
use cruiser::prelude::*;
use cruiser_tutorial::accounts::{Game, Player, Space};
use cruiser_tutorial::instructions::{
    create_game, create_profile, join_game, make_move, CreateGameClientData, MakeMoveData,
};
use cruiser_tutorial::pda::GameSignerSeeder;
use cruiser_tutorial::TutorialAccounts;
use std::error::Error;
use std::time::Duration;

#[tokio::test]
async fn make_move_test() -> Result<(), Box<dyn Error>> {
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
                turn_length: 60 * 60 * 24, // 1 day
            },
        ))
        .signed_instructions(join_game(
            guard.program_id(),
            &authority2,
            profile2.pubkey(),
            game.pubkey(),
            GameSignerSeeder {
                game: game.pubkey(),
            }
            .find_address(&guard.program_id())
            .1,
            &funder,
        ))
        .signed_instructions(make_move(
            guard.program_id(),
            &authority1,
            profile1.pubkey(),
            game.pubkey(),
            MakeMoveData {
                big_board: [0, 0],
                small_board: [0, 0],
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
    let game: Game = Game::deserialize(&mut data)?;
    assert!(game.last_turn > 0);
    let mut expected = Game::new(
        &profile1.pubkey(),
        Player::One,
        game.signer_bump,
        LAMPORTS_PER_SOL,
        60 * 60 * 24,
    );
    expected.player2 = profile2.pubkey();
    expected.last_turn = game.last_turn;
    expected.next_play = Player::Two;
    expected.last_move = [0, 0];
    *expected
        .board
        .get_mut([0, 0])
        .unwrap()
        .get_mut([0, 0])
        .unwrap() = Space::PlayerOne;

    assert_eq!(game, expected);

    guard.drop_self().await;
    Ok(())
}

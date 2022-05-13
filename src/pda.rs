//! PDAs for the program.

use cruiser::prelude::*;

/// The static seed for [`GameSignerSeeder`].
pub const GAME_SIGNER_SEED: &str = "game_signer";

/// The seeder for the game signer.
///
/// We use a seeder to create type safe PDA definitions that can't accidentally be switched around or mis-set.
#[derive(Debug, Clone)]
pub struct GameSignerSeeder {
    /// The game's key.
    pub game: Pubkey,
}
impl PDASeeder for GameSignerSeeder {
    fn seeds<'a>(&'a self) -> Box<dyn Iterator<Item = &'a dyn PDASeed> + 'a> {
        // We need to return a list of seeds. This means you can order your seeds however you want.
        // You can also use anything that implements `PDASeed` directly, even your own types.
        Box::new([&GAME_SIGNER_SEED as &dyn PDASeed, &self.game].into_iter())
    }
}

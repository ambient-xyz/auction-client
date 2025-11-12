pub mod sdk;

use solana_sdk::pubkey::Pubkey;
pub const ID: Pubkey = Pubkey::new_from_array(ambient_auction_api::ID);

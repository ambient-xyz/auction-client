use crate::ID as program_id;
use ambient_auction_api::state::RequestTier;
use ambient_auction_api::{
    AUCTION_SEED, BID_SEED, BUNDLE_ESCROW_V2_SEED, BUNDLE_REGISTRY_SEED,
    BUNDLE_VERIFIER_PAGE_V2_SEED, CONFIG_POLICY_V2_SEED, CONFIG_SEED, JOB_REQUEST_SEED,
    PUBKEY_BYTES, REQUEST_BUNDLE_SEED,
};
use solana_sdk::pubkey::{MAX_SEED_LEN, Pubkey};

pub trait ToClientPubkey {
    fn to_client_pubkey(self) -> Pubkey;
}

impl ToClientPubkey for Pubkey {
    fn to_client_pubkey(self) -> Pubkey {
        self
    }
}

impl ToClientPubkey for &Pubkey {
    fn to_client_pubkey(self) -> Pubkey {
        *self
    }
}

impl ToClientPubkey for [u8; PUBKEY_BYTES] {
    fn to_client_pubkey(self) -> Pubkey {
        Pubkey::new_from_array(self)
    }
}

impl ToClientPubkey for &[u8; PUBKEY_BYTES] {
    fn to_client_pubkey(self) -> Pubkey {
        Pubkey::new_from_array(*self)
    }
}

pub fn find_bundle_registry(
    context_length_tier: RequestTier,
    expiry_duration_tier: RequestTier,
) -> Pubkey {
    let context_length_tier_bytes = (context_length_tier as u64).to_le_bytes();
    let expiry_duration_tier_bytes = (expiry_duration_tier as u64).to_le_bytes();

    Pubkey::find_program_address(
        &[
            BUNDLE_REGISTRY_SEED,
            context_length_tier_bytes.as_ref(),
            expiry_duration_tier_bytes.as_ref(),
        ],
        &program_id,
    )
    .0
}

pub fn find_root_bundle(
    context_length_tier: RequestTier,
    expiry_duration_tier: RequestTier,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            REQUEST_BUNDLE_SEED,
            (context_length_tier as u64).to_le_bytes().as_ref(),
            (expiry_duration_tier as u64).to_le_bytes().as_ref(),
        ],
        &program_id,
    )
    .0
}

pub fn find_child_bundle(bundle: impl ToClientPubkey) -> Pubkey {
    let bundle = bundle.to_client_pubkey();
    Pubkey::find_program_address(&[REQUEST_BUNDLE_SEED, bundle.as_ref()], &program_id).0
}

pub fn find_auction(bundle: impl ToClientPubkey) -> Pubkey {
    let bundle = bundle.to_client_pubkey();
    Pubkey::find_program_address(&[AUCTION_SEED, bundle.as_ref()], &program_id).0
}

pub fn find_bid(auction: impl ToClientPubkey, bidder: impl ToClientPubkey) -> Pubkey {
    let auction = auction.to_client_pubkey();
    let bidder = bidder.to_client_pubkey();
    Pubkey::find_program_address(&[BID_SEED, auction.as_ref(), bidder.as_ref()], &program_id).0
}

pub fn find_job_request(
    authority: impl ToClientPubkey,
    context_length_tier: RequestTier,
    expiry_duration_tier: RequestTier,
    job_request_seed: [u8; MAX_SEED_LEN],
) -> Pubkey {
    let authority = authority.to_client_pubkey();
    let context_length_tier_bytes = (context_length_tier as u64).to_le_bytes();
    let expiry_duration_tier_bytes = (expiry_duration_tier as u64).to_le_bytes();
    Pubkey::find_program_address(
        &[
            JOB_REQUEST_SEED,
            context_length_tier_bytes.as_ref(),
            expiry_duration_tier_bytes.as_ref(),
            authority.as_ref(),
            &job_request_seed,
        ],
        &program_id,
    )
    .0
}

#[cfg(feature = "global-config")]
pub fn find_config() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], &program_id).0
}

pub fn find_config_policy_v2(target_program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED, CONFIG_POLICY_V2_SEED], &target_program_id).0
}

pub fn find_bundle_escrow_v2(
    target_program_id: Pubkey,
    payer: impl ToClientPubkey,
    bundle_hash: [u8; 32],
    bundle_version: u32,
) -> Pubkey {
    let payer = payer.to_client_pubkey();
    Pubkey::find_program_address(
        &[
            BUNDLE_ESCROW_V2_SEED,
            payer.as_ref(),
            bundle_hash.as_ref(),
            bundle_version.to_le_bytes().as_ref(),
        ],
        &target_program_id,
    )
    .0
}

pub fn find_bundle_verifier_page_v2(
    target_program_id: Pubkey,
    bundle_escrow: impl ToClientPubkey,
    page_index: u16,
) -> Pubkey {
    let bundle_escrow = bundle_escrow.to_client_pubkey();
    Pubkey::find_program_address(
        &[
            BUNDLE_VERIFIER_PAGE_V2_SEED,
            bundle_escrow.as_ref(),
            page_index.to_le_bytes().as_ref(),
        ],
        &target_program_id,
    )
    .0
}

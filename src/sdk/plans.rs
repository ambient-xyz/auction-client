use crate::ID as program_id;
use ambient_auction_api::state::RequestTier;
use ambient_auction_api::{MaybePubkey, PUBKEY_BYTES, instruction::*};
use solana_sdk::hash::hashv;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::{MAX_SEED_LEN, Pubkey},
};
use solana_system_interface::program as system_program;
use std::net::IpAddr;

#[cfg(feature = "global-config")]
use super::find_config;
use super::{
    ToClientPubkey, find_auction, find_bid, find_bundle_escrow_v2, find_bundle_registry,
    find_child_bundle, find_config_policy_v2, find_job_request, find_root_bundle,
};

struct RequestJobBundleAccounts {
    bundle_auction_account_pairs: Vec<Pubkey>,
    last_bundle: Pubkey,
}

fn system_program_key() -> Pubkey {
    Pubkey::new_from_array(system_program::ID.to_bytes())
}

fn request_job_bundle_accounts(
    bundle_key: Pubkey,
    additional_bundles: Option<u64>,
) -> RequestJobBundleAccounts {
    let parent_auction = find_auction(bundle_key);
    let first_child_bundle = find_child_bundle(bundle_key);
    let first_child_auction = find_auction(first_child_bundle);

    let mut bundle_auction_account_pairs = vec![
        bundle_key,
        parent_auction,
        first_child_bundle,
        first_child_auction,
    ];

    let mut current_last = first_child_bundle;
    if let Some(additions) = additional_bundles {
        for _ in 0..additions {
            let next_bundle = find_child_bundle(current_last);
            let next_auction = find_auction(next_bundle);
            bundle_auction_account_pairs.extend([next_bundle, next_auction]);
            current_last = next_bundle;
        }
    }

    RequestJobBundleAccounts {
        bundle_auction_account_pairs,
        last_bundle: find_child_bundle(current_last),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn request_job_plan(
    authority: impl ToClientPubkey,
    input_hash: [u8; PUBKEY_BYTES],
    input_hash_iv: Option<[u8; 16]>,
    job_request_seed: [u8; MAX_SEED_LEN],
    input_tokens: u64,
    max_output_tokens: u64,
    new_bundle_lamports: u64,
    new_auction_lamports: u64,
    bundle_key: impl ToClientPubkey,
    max_price_per_output_token: u64,
    context_length_tier: RequestTier,
    expiry_duration_tier: RequestTier,
    input_data_account: Option<Pubkey>,
    // TODO: this should be used not hardcoded
    _additional_bundles: Option<u64>,
) -> (Instruction, RequestJobAccountKeys<Pubkey>) {
    let additional_bundles = Some(8);
    let authority = authority.to_client_pubkey();
    let bundle_key = bundle_key.to_client_pubkey();
    let job_request = find_job_request(
        authority,
        context_length_tier,
        expiry_duration_tier,
        job_request_seed,
    );
    let RequestJobBundleAccounts {
        bundle_auction_account_pairs,
        last_bundle,
    } = request_job_bundle_accounts(bundle_key, additional_bundles);
    let registry = find_bundle_registry(context_length_tier, expiry_duration_tier);
    let input_data = input_data_account.unwrap_or_default();
    let system_program = system_program_key();

    #[cfg(feature = "global-config")]
    let config = find_config();

    #[cfg(not(feature = "global-config"))]
    let account_keys = RequestJobAccountKeys {
        payer: authority,
        job_request,
        registry,
        input_data,
        system_program,
        bundle_auction_account_pairs,
        last_bundle,
    };

    #[cfg(feature = "global-config")]
    let account_keys = RequestJobAccountKeys {
        payer: authority,
        job_request,
        registry,
        input_data,
        system_program,
        config,
        bundle_auction_account_pairs,
        last_bundle,
    };

    let bundle_auction_account_pairs = account_keys
        .bundle_auction_account_pairs
        .iter()
        .copied()
        .map(|pubkey| AccountMeta::new(pubkey, false))
        .collect::<Vec<_>>();

    let accounts_infos = RequestJobAccounts {
        payer: &AccountMeta::new(account_keys.payer, true),
        job_request: &AccountMeta::new(account_keys.job_request, false),
        registry: &AccountMeta::new(account_keys.registry, false),
        input_data: &AccountMeta::new(account_keys.input_data, false),
        system_program: &AccountMeta::new_readonly(account_keys.system_program, false),
        #[cfg(feature = "global-config")]
        config: &AccountMeta::new(account_keys.config, false),
        bundle_auction_account_pairs: bundle_auction_account_pairs.as_slice(),
        last_bundle: &AccountMeta::new(account_keys.last_bundle, false),
    };

    let input_data_key: MaybePubkey = input_data_account.map(|key| key.to_bytes().into()).into();
    let context_length_tier_bytes = (context_length_tier as u64).to_le_bytes();
    let expiry_duration_tier_bytes = (expiry_duration_tier as u64).to_le_bytes();
    let bump = Pubkey::find_program_address(
        &[
            ambient_auction_api::JOB_REQUEST_SEED,
            context_length_tier_bytes.as_ref(),
            expiry_duration_tier_bytes.as_ref(),
            authority.as_ref(),
            &job_request_seed,
        ],
        &program_id,
    )
    .1;

    (
        Instruction {
            program_id,
            data: RequestJobArgs {
                max_price_per_output_token,
                authority: authority.to_bytes(),
                input_hash,
                job_request_seed,
                new_bundle_lamports,
                input_tokens,
                bump: bump.into(),
                max_output_tokens,
                new_auction_lamports,
                input_hash_iv: input_hash_iv.unwrap_or_default(),
                input_data_account: input_data_key,
            }
            .to_bytes(),
            accounts: accounts_infos.iter_owned().collect::<Vec<_>>(),
        },
        account_keys,
    )
}

pub fn place_bid_plan(
    authority: impl ToClientPubkey,
    auction: impl ToClientPubkey,
    price_per_output_token: u64,
    price_hash_seed: [u8; 32],
    endpoint: (IpAddr, u16),
    node_encryption_publickey: Option<[u8; 32]>,
) -> (Instruction, PlaceBidAccountKeys<Pubkey>) {
    let price_hash = hashv(&[&price_hash_seed, &price_per_output_token.to_le_bytes()]).to_bytes();
    let authority = authority.to_client_pubkey();
    let auction = auction.to_client_pubkey();
    let account_keys = PlaceBidAccountKeys {
        payer: authority,
        bid: find_bid(auction, authority),
        auction,
        system_program: system_program_key(),
    };

    let account_metas = PlaceBidAccounts {
        payer: &AccountMeta::new(account_keys.payer, true),
        bid: &AccountMeta::new(account_keys.bid, false),
        auction: &AccountMeta::new(account_keys.auction, false),
        system_program: &AccountMeta::new_readonly(account_keys.system_program, false),
    };

    (
        Instruction {
            program_id,
            data: PlaceBidArgs::new(
                price_hash,
                authority.to_bytes(),
                endpoint.0.into(),
                endpoint.1,
                node_encryption_publickey,
            )
            .to_bytes(),
            accounts: account_metas.iter_owned().collect::<Vec<_>>(),
        },
        account_keys,
    )
}

pub fn reveal_bid_plan(
    bidder_key: impl ToClientPubkey,
    auction_key: impl ToClientPubkey,
    bundle_key: impl ToClientPubkey,
    vote_account: impl ToClientPubkey,
    vote_authority: impl ToClientPubkey,
    args: RevealBidArgs,
) -> (Instruction, RevealBidAccountKeys<Pubkey>) {
    let bidder_key = bidder_key.to_client_pubkey();
    let auction_key = auction_key.to_client_pubkey();
    let bundle_key = bundle_key.to_client_pubkey();
    let vote_account = vote_account.to_client_pubkey();
    let vote_authority = vote_authority.to_client_pubkey();
    let account_keys = RevealBidAccountKeys {
        bid_authority: bidder_key,
        bid: find_bid(auction_key, bidder_key),
        auction: auction_key,
        bundle: bundle_key,
        vote_account,
        vote_authority,
    };

    let account_metas = RevealBidAccounts {
        bid_authority: &AccountMeta::new(account_keys.bid_authority, true),
        bid: &AccountMeta::new(account_keys.bid, false),
        auction: &AccountMeta::new(account_keys.auction, false),
        bundle: &AccountMeta::new(account_keys.bundle, false),
        vote_account: &AccountMeta::new(account_keys.vote_account, false),
        vote_authority: &AccountMeta::new(account_keys.vote_authority, true),
    };

    (
        Instruction {
            program_id,
            data: args.to_bytes(),
            accounts: account_metas.iter_owned().collect::<Vec<_>>(),
        },
        account_keys,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn submit_job_plan(
    authority: impl ToClientPubkey,
    bundle_key: impl ToClientPubkey,
    job_request_key: impl ToClientPubkey,
    data: SubmitJobOutputArgs,
    output_data_account: Option<Pubkey>,
) -> (Instruction, SubmitJobOutputAccountKeys<Pubkey>) {
    let authority = authority.to_client_pubkey();
    let bundle_key = bundle_key.to_client_pubkey();
    let job_request_key = job_request_key.to_client_pubkey();
    let auction = find_auction(bundle_key);
    let account_keys = SubmitJobOutputAccountKeys {
        bid_authority: authority,
        bundle: bundle_key,
        job_request: job_request_key,
        bid: find_bid(auction, authority),
        auction,
        output_data_account: output_data_account.unwrap_or_default(),
    };

    let account_metas = SubmitJobOutputAccounts {
        bid_authority: &AccountMeta::new(account_keys.bid_authority, true),
        bundle: &AccountMeta::new(account_keys.bundle, false),
        job_request: &AccountMeta::new(account_keys.job_request, false),
        bid: &AccountMeta::new_readonly(account_keys.bid, false),
        auction: &AccountMeta::new_readonly(account_keys.auction, false),
        output_data_account: &AccountMeta::new(account_keys.output_data_account, false),
    };

    (
        Instruction {
            program_id,
            data: data.to_bytes(),
            accounts: account_metas.iter_owned().collect::<Vec<_>>(),
        },
        account_keys,
    )
}

pub fn init_bundle_plan(
    payer: impl ToClientPubkey,
    context_length_tier: RequestTier,
    expiry_duration_tier: RequestTier,
    bundle_lamports: u64,
    registry_lamports: u64,
) -> (Instruction, InitBundleAccountKeys<Pubkey>) {
    let payer = payer.to_client_pubkey();
    let bundle = find_root_bundle(context_length_tier, expiry_duration_tier);
    let bundle_bump = Pubkey::find_program_address(
        &[
            ambient_auction_api::REQUEST_BUNDLE_SEED,
            (context_length_tier as u64).to_le_bytes().as_ref(),
            (expiry_duration_tier as u64).to_le_bytes().as_ref(),
        ],
        &program_id,
    )
    .1;
    let registry = find_bundle_registry(context_length_tier, expiry_duration_tier);
    let registry_bump = Pubkey::find_program_address(
        &[
            ambient_auction_api::BUNDLE_REGISTRY_SEED,
            (context_length_tier as u64).to_le_bytes().as_ref(),
            (expiry_duration_tier as u64).to_le_bytes().as_ref(),
        ],
        &program_id,
    )
    .1;
    let account_keys = InitBundleAccountKeys {
        payer,
        bundle,
        registry,
        system_program: system_program_key(),
    };

    let account_metas = InitBundleAccounts {
        payer: &AccountMeta::new(account_keys.payer, true),
        bundle: &AccountMeta::new(account_keys.bundle, false),
        registry: &AccountMeta::new(account_keys.registry, false),
        system_program: &AccountMeta::new_readonly(account_keys.system_program, false),
    };

    (
        Instruction {
            program_id,
            data: InitBundleArgs {
                context_length_tier,
                expiry_duration_tier,
                bundle_lamports,
                bundle_bump: bundle_bump.into(),
                registry_bump: registry_bump.into(),
                registry_lamports,
            }
            .to_bytes(),
            accounts: account_metas.iter_owned().collect::<Vec<_>>(),
        },
        account_keys,
    )
}

#[cfg(feature = "global-config")]
pub fn init_config_plan(
    payer: impl ToClientPubkey,
    args: InitConfigArgs,
) -> (Instruction, InitConfigAccountKeys<Pubkey>) {
    let payer = payer.to_client_pubkey();
    let account_keys = InitConfigAccountKeys {
        payer,
        config: find_config(),
        system_program: system_program_key(),
    };

    let account_metas = InitConfigAccounts {
        payer: &AccountMeta::new(account_keys.payer, true),
        config: &AccountMeta::new(account_keys.config, false),
        system_program: &AccountMeta::new_readonly(account_keys.system_program, false),
    };

    (
        Instruction {
            program_id,
            data: args.to_bytes(),
            accounts: account_metas.iter_owned().collect::<Vec<_>>(),
        },
        account_keys,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn open_bundle_escrow_v2_plan(
    payer: impl ToClientPubkey,
    bundle_version: u32,
    reward_tier: RequestTier,
    bundle_hash: [u8; 32],
    coordinator: impl ToClientPubkey,
    requester_refund_recipient: impl ToClientPubkey,
    total_input_tokens: u64,
    max_output_tokens: u64,
    escrow_lamports: u64,
    settlement_deadline_slot: u64,
    result_deadline_slot: u64,
    verification_deadline_slot: u64,
    claim_deadline_slot: u64,
) -> (Instruction, OpenBundleEscrowV2AccountKeys<Pubkey>) {
    let payer = payer.to_client_pubkey();
    let coordinator = coordinator.to_client_pubkey();
    let requester_refund_recipient = requester_refund_recipient.to_client_pubkey();
    let account_keys = OpenBundleEscrowV2AccountKeys {
        payer,
        bundle_escrow: find_bundle_escrow_v2(payer, bundle_hash, bundle_version),
        config_policy: find_config_policy_v2(),
        system_program: system_program_key(),
    };

    let account_metas = OpenBundleEscrowV2Accounts {
        payer: &AccountMeta::new(account_keys.payer, true),
        bundle_escrow: &AccountMeta::new(account_keys.bundle_escrow, false),
        config_policy: &AccountMeta::new(account_keys.config_policy, false),
        system_program: &AccountMeta::new_readonly(account_keys.system_program, false),
    };

    (
        Instruction {
            program_id,
            data: OpenBundleEscrowV2Args {
                bundle_version,
                _reserved0: [0; 4],
                reward_tier,
                bundle_hash,
                coordinator: coordinator.to_bytes(),
                requester_refund_recipient: requester_refund_recipient.to_bytes(),
                total_input_tokens,
                max_output_tokens,
                escrow_lamports,
                settlement_deadline_slot,
                result_deadline_slot,
                verification_deadline_slot,
                claim_deadline_slot,
            }
            .to_bytes(),
            accounts: account_metas.iter_owned().collect::<Vec<_>>(),
        },
        account_keys,
    )
}

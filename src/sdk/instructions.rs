use crate::ID as program_id;
use ambient_auction_api::state::{
    ConfigPolicyV2, ConfigPolicyV2Flags, RequestTier, RequestTierConfigV2,
};
use ambient_auction_api::{PUBKEY_BYTES, REQUEST_BUNDLE_SEED, instruction::*};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::{MAX_SEED_LEN, Pubkey},
};
use solana_system_interface::program as system_program;
use solana_vote_interface::program as vote;
use std::net::IpAddr;
use std::num::NonZeroU64;

#[cfg(feature = "global-config")]
use super::init_config_plan;
use super::{
    find_auction, find_bundle_registry, find_bundle_verifier_page_v2, find_child_bundle,
    find_config_policy_v2, init_bundle_plan, open_bundle_escrow_v2_plan, place_bid_plan,
    request_job_plan, reveal_bid_plan, submit_job_plan,
};

#[allow(clippy::too_many_arguments)]
fn build_post_bundle_result_v2_instruction<D: InstructionBytes>(
    target_program_id: Pubkey,
    authority: Pubkey,
    bundle_escrow: Pubkey,
    bundle_verifier_page: Option<Pubkey>,
    result_hash: [u8; 32],
    posted_output_tokens: u64,
    page_index: u16,
    page_entries: &[ambient_auction_api::BundleVerifierPageV2Entry],
    wrap_data: impl FnOnce(PostBundleResultV2Args) -> D,
) -> Instruction {
    assert!(
        page_entries.len() <= ambient_auction_api::MAX_BUNDLE_VERIFIER_PAGE_V2_ENTRIES,
        "page entries exceed BundleVerifierPageV2 capacity"
    );

    let padded_page_entries = {
        let mut entries = [ambient_auction_api::BundleVerifierPageV2Entry::default();
            ambient_auction_api::MAX_BUNDLE_VERIFIER_PAGE_V2_ENTRIES];
        for (index, entry) in page_entries.iter().copied().enumerate() {
            entries[index] = entry;
        }
        entries
    };

    let bundle_verifier_page_meta = bundle_verifier_page.map(|page| AccountMeta::new(page, false));
    let config_policy = find_config_policy_v2(target_program_id);
    let account_metas = PostBundleResultV2Accounts {
        authority: &AccountMeta::new(authority, true),
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        config_policy: &AccountMeta::new(config_policy, false),
        bundle_verifier_page: bundle_verifier_page_meta.as_ref(),
    };

    Instruction {
        program_id: target_program_id,
        data: wrap_data(PostBundleResultV2Args {
            result_hash,
            posted_output_tokens,
            page_index,
            page_entry_count: page_entries.len() as u16,
            _reserved: [0; 4],
            page_entries: padded_page_entries,
        })
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn append_data(
    payer: Pubkey,
    account_data: &[u8],
    // will be truncated if longer than 32
    seed: &str,
    offset: u64,
    data_account_key: Pubkey,
    // None if compression is not used
    decompressed_data_length: Option<NonZeroU64>,
) -> Instruction {
    let seed_len = seed.len().min(MAX_SEED_LEN);

    debug_assert!(
        seed.len() <= MAX_SEED_LEN,
        "Seed too long; truncated to 32 bytes"
    );

    let mut padded_seed = [0u8; MAX_SEED_LEN];
    padded_seed[..seed_len].copy_from_slice(&seed.as_bytes()[..seed_len]);

    let mut data = AppendDataArgs {
        offset,
        seed: padded_seed,
        seed_len: seed_len as u64,
        decompressed_data_length,
    }
    .to_bytes();

    data.extend_from_slice(account_data);

    let accounts_infos = AppendDataAccounts {
        data_authority: &AccountMeta::new(payer, true),
        data_account: &AccountMeta::new(data_account_key, false),
        system_program: &AccountMeta::new_readonly(solana_system_interface::program::ID, false),
    };

    Instruction {
        program_id,
        accounts: accounts_infos.iter_owned().collect::<Vec<_>>(),
        data,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn request_job(
    authority: Pubkey,
    input_hash: [u8; PUBKEY_BYTES],
    input_hash_iv: Option<[u8; 16]>,
    job_request_seed: [u8; MAX_SEED_LEN],
    input_tokens: u64,
    max_output_tokens: u64,
    new_bundle_lamports: u64,
    new_auction_lamports: u64,
    bundle_key: Pubkey,
    max_price_per_output_token: u64,
    context_length_tier: RequestTier,
    expiry_duration_tier: RequestTier,
    input_data_account: Option<Pubkey>,
    // TODO: this should be used not hardcoded
    _additional_bundles: Option<u64>,
) -> Instruction {
    request_job_plan(
        authority,
        input_hash,
        input_hash_iv,
        job_request_seed,
        input_tokens,
        max_output_tokens,
        new_bundle_lamports,
        new_auction_lamports,
        bundle_key,
        max_price_per_output_token,
        context_length_tier,
        expiry_duration_tier,
        input_data_account,
        _additional_bundles,
    )
    .0
}

pub fn place_bid(
    authority: Pubkey,
    auction: Pubkey,
    price_per_output_token: u64,
    // seed used to hash bid price
    // later used to reveal the price
    price_hash_seed: [u8; 32],
    endpoint: (IpAddr, u16),
    node_encryption_publickey: Option<[u8; 32]>,
) -> Instruction {
    place_bid_plan(
        authority,
        auction,
        price_per_output_token,
        price_hash_seed,
        endpoint,
        node_encryption_publickey,
    )
    .0
}

pub fn reveal_bid(
    bidder_key: Pubkey,
    auction_key: Pubkey,
    bundle_key: Pubkey,
    vote_account: Pubkey,
    vote_authority: Pubkey,
    args: RevealBidArgs,
) -> Instruction {
    reveal_bid_plan(
        bidder_key,
        auction_key,
        bundle_key,
        vote_account,
        vote_authority,
        args,
    )
    .0
}

#[allow(clippy::too_many_arguments)]
pub fn submit_job(
    authority: Pubkey,
    bundle_key: Pubkey,
    job_request_key: Pubkey,
    data: SubmitJobOutputArgs,
    // account to be used as the job output account.
    // this is required if an input data account is used for the request
    output_data_account: Option<Pubkey>,
) -> Instruction {
    submit_job_plan(
        authority,
        bundle_key,
        job_request_key,
        data,
        output_data_account,
    )
    .0
}

pub fn end_auction(signer: Pubkey, bundle_key: Pubkey, vote_account: Pubkey) -> Instruction {
    let auction_key = find_auction(bundle_key);

    let account_metas = EndAuctionAccounts {
        auction: &AccountMeta::new(auction_key, false),
        bundle: &AccountMeta::new(bundle_key, false),
        vote_account: &AccountMeta::new(vote_account, false),
        payer: &AccountMeta::new(signer, true),
    };

    Instruction {
        program_id,
        data: EndAuctionArgs {}.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn cancel_bundle(
    signer: Pubkey,
    // the parent of the bundle to be cancelled
    parent_bundle_key: Pubkey,
    bundle_key: Pubkey,
    bundle_bump: u8,
    context_length_tier: RequestTier,
    expiry_duration_tier: RequestTier,
    bundle_lamports: u64,
) -> Instruction {
    let registry = find_bundle_registry(context_length_tier, expiry_duration_tier);
    let child_bundle = find_child_bundle(bundle_key);
    let child_bundle_bump =
        Pubkey::find_program_address(&[REQUEST_BUNDLE_SEED, bundle_key.as_ref()], &program_id).1;

    let account_metas = CancelBundleAccounts {
        payer: &AccountMeta::new(signer, true),
        bundle: &AccountMeta::new(bundle_key, false),
        child_bundle: &AccountMeta::new(child_bundle, false),
        registry: &AccountMeta::new(registry, false),
        system_program: &AccountMeta::new_readonly(solana_system_interface::program::ID, false),
    };

    Instruction {
        program_id,
        data: CancelBundleArgs {
            parent_bundle_key: parent_bundle_key.to_bytes().into(),
            bundle_bump: bundle_bump.into(),
            context_length_tier,
            expiry_duration_tier,
            child_bundle_bump: child_bundle_bump as u64,
            bundle_lamports,
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn close_bid(
    bid_authority: Pubkey,
    auction_payer: Pubkey,
    bid_key: Pubkey,
    auction_key: Pubkey,
    bundle_key: Pubkey,
    vote_account: Pubkey,
    vote_authority: Pubkey,
) -> Instruction {
    let account_metas = CloseBidAccounts {
        bid_authority: &AccountMeta::new(bid_authority, true),
        bid: &AccountMeta::new_readonly(bid_key, false),
        auction_payer: &AccountMeta::new(auction_payer, true),
        auction: &AccountMeta::new(auction_key, false),
        bundle: &AccountMeta::new_readonly(bundle_key, false),
        vote_account: &AccountMeta::new(vote_account, false),
        vote_authority: &AccountMeta::new(vote_authority, true),
        vote_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(vote::ID.to_bytes()),
            false,
        ),
    };

    Instruction {
        program_id,
        data: CloseBidArgs {}.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub struct CloseRequest {
    pub request_authority: Pubkey,
    pub job_request_key: Pubkey,
    pub bundle_payer: Pubkey,
    pub bundle_key: Pubkey,
    pub auction_key: Pubkey,
    pub auction_payer: Pubkey,
    pub context_length_tier: RequestTier,
    pub expiry_duration_tier: RequestTier,
    pub new_bundle_lamports: u64,
    pub new_auction_lamports: u64,
}

pub fn close_request(args: CloseRequest) -> Instruction {
    let CloseRequest {
        request_authority,
        job_request_key,
        bundle_payer,
        bundle_key,
        auction_key,
        auction_payer,
        context_length_tier,
        expiry_duration_tier,
        new_bundle_lamports,
        new_auction_lamports,
    } = args;

    let registry = find_bundle_registry(context_length_tier, expiry_duration_tier);
    let child_bundle_key = find_child_bundle(bundle_key);
    let new_bundle_bump =
        Pubkey::find_program_address(&[REQUEST_BUNDLE_SEED, bundle_key.as_ref()], &program_id).1;
    let child_auction_key = find_auction(child_bundle_key);

    let account_metas = CloseRequestAccounts {
        request_authority: &AccountMeta::new(request_authority, true),
        job_request: &AccountMeta::new(job_request_key, false),
        bundle_payer: &AccountMeta::new(bundle_payer, false),
        bundle: &AccountMeta::new(bundle_key, false),
        registry: &AccountMeta::new(registry, false),
        auction: &AccountMeta::new(auction_key, false),
        auction_payer: &AccountMeta::new(auction_payer, false),
        child_bundle: &AccountMeta::new(child_bundle_key, false),
        child_auction: &AccountMeta::new(child_auction_key, false),
        // pay from the request authority
        child_bundle_payer: &AccountMeta::new(request_authority, true),
    };

    Instruction {
        program_id,
        data: CloseRequestArgs {
            new_bundle_lamports,
            new_auction_lamports,
            new_bundle_bump: new_bundle_bump as u64,
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn submit_validation(
    bundle_key: Pubkey,
    vote_account: Pubkey,
    vote_authority: Pubkey,
    job_request_key: Pubkey,
    data: SubmitValidationArgs,
) -> Instruction {
    let account_metas = SubmitValidationAccounts {
        bundle: &AccountMeta::new(bundle_key, false),
        vote_account: &AccountMeta::new(vote_account, false),
        vote_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(vote::ID.to_bytes()),
            false,
        ),
        vote_authority: &AccountMeta::new(vote_authority, true),
        job_request: &AccountMeta::new(job_request_key, false),
    };

    Instruction {
        program_id,
        data: data.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn init_bundle(
    payer: Pubkey,
    context_length_tier: RequestTier,
    expiry_duration_tier: RequestTier,
    // lamports used to initialize the bundle account
    bundle_lamports: u64,
    // lamports used to initialize the bundle registry account
    registry_lamports: u64,
) -> Instruction {
    init_bundle_plan(
        payer,
        context_length_tier,
        expiry_duration_tier,
        bundle_lamports,
        registry_lamports,
    )
    .0
}

#[cfg(feature = "global-config")]
pub fn init_config(payer: Pubkey, args: InitConfigArgs) -> Instruction {
    init_config_plan(payer, args).0
}

pub fn init_config_policy_v2(
    target_program_id: Pubkey,
    payer: Pubkey,
    config_policy_lamports: u64,
    initial_admin_authority: Pubkey,
    service_authority: Pubkey,
    policy: ConfigPolicyV2,
) -> Instruction {
    let config_policy = find_config_policy_v2(target_program_id);

    let account_metas = InitConfigPolicyV2Accounts {
        authority: &AccountMeta::new(payer, true),
        config_policy: &AccountMeta::new(config_policy, false),
        system_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(system_program::ID.to_bytes()),
            false,
        ),
    };

    Instruction {
        program_id: target_program_id,
        data: InitConfigPolicyV2Args {
            config_policy_lamports,
            initial_admin_authority: initial_admin_authority.to_bytes().into(),
            service_authority: service_authority.to_bytes().into(),
            policy_flags: policy.policy_flags,
            minimum_bundle_auction_pairs: policy.minimum_bundle_auction_pairs,
            max_auction_credits_per_update: policy.max_auction_credits_per_update,
            v2_verifiers_per_auction: policy.v2_verifiers_per_auction,
            v2_verifier_quorum: policy.v2_verifier_quorum,
            _reserved0: [0; 6],
            tier_configs: policy.tier_configs,
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

fn empty_set_config_policy_v2_args(patch_kind: ConfigPolicyV2PatchKind) -> SetConfigPolicyV2Args {
    SetConfigPolicyV2Args {
        patch_kind,
        authority_kind: ConfigPolicyV2AuthorityKind::ADMIN,
        authority_index: 0,
        v2_verifiers_per_auction: 0,
        v2_verifier_quorum: 0,
        small_credit_enabled: 0,
        _reserved0: [0; 2],
        tier: 0,
        policy_flags: ConfigPolicyV2Flags::empty(),
        max_auction_credits_per_update: 0,
        authority: [0; 32].into(),
        tier_config: RequestTierConfigV2 {
            bid_reveal_duration: 0,
            active_auction_duration: 0,
            bundle_duration: 0,
            requests_per_bundle: 0,
            max_context_length_tokens: 0,
            job_submission_duration_slots: 0,
            bid_commitment_amount_multiplier: 0,
            auction_credits_multiplier: 0,
            settlement_window_slots: 0,
            result_window_slots: 0,
            verification_window_slots: 0,
            claim_window_slots: 0,
        },
    }
}

fn set_config_policy_v2_with_args(
    target_program_id: Pubkey,
    authority: Pubkey,
    args: SetConfigPolicyV2Args,
) -> Instruction {
    let config_policy = find_config_policy_v2(target_program_id);

    let account_metas = SetConfigPolicyV2Accounts {
        authority: &AccountMeta::new(authority, true),
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id: target_program_id,
        data: args.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn set_config_policy_v2_flags(
    target_program_id: Pubkey,
    authority: Pubkey,
    policy_flags: ConfigPolicyV2Flags,
) -> Instruction {
    set_config_policy_v2_with_args(
        target_program_id,
        authority,
        SetConfigPolicyV2Args {
            policy_flags,
            ..empty_set_config_policy_v2_args(ConfigPolicyV2PatchKind::FLAGS)
        },
    )
}

pub fn set_config_policy_v2_admin_authority(
    target_program_id: Pubkey,
    authority: Pubkey,
    authority_index: u8,
    admin_authority: Pubkey,
) -> Instruction {
    set_config_policy_v2_with_args(
        target_program_id,
        authority,
        SetConfigPolicyV2Args {
            authority_kind: ConfigPolicyV2AuthorityKind::ADMIN,
            authority_index,
            authority: admin_authority.to_bytes().into(),
            ..empty_set_config_policy_v2_args(ConfigPolicyV2PatchKind::AUTHORITY)
        },
    )
}

pub fn set_config_policy_v2_service_authority(
    target_program_id: Pubkey,
    authority: Pubkey,
    authority_index: u8,
    service_authority: Pubkey,
) -> Instruction {
    set_config_policy_v2_with_args(
        target_program_id,
        authority,
        SetConfigPolicyV2Args {
            authority_kind: ConfigPolicyV2AuthorityKind::SERVICE,
            authority_index,
            authority: service_authority.to_bytes().into(),
            ..empty_set_config_policy_v2_args(ConfigPolicyV2PatchKind::AUTHORITY)
        },
    )
}

pub fn set_config_policy_v2_verifier_settings(
    target_program_id: Pubkey,
    authority: Pubkey,
    v2_verifiers_per_auction: u8,
    v2_verifier_quorum: u8,
) -> Instruction {
    set_config_policy_v2_with_args(
        target_program_id,
        authority,
        SetConfigPolicyV2Args {
            v2_verifiers_per_auction,
            v2_verifier_quorum,
            ..empty_set_config_policy_v2_args(ConfigPolicyV2PatchKind::VERIFIER_SETTINGS)
        },
    )
}

pub fn set_config_policy_v2_tier_config(
    target_program_id: Pubkey,
    authority: Pubkey,
    tier: RequestTier,
    tier_config: RequestTierConfigV2,
) -> Instruction {
    set_config_policy_v2_with_args(
        target_program_id,
        authority,
        SetConfigPolicyV2Args {
            tier: u64::from(tier),
            tier_config,
            ..empty_set_config_policy_v2_args(ConfigPolicyV2PatchKind::TIER_CONFIG)
        },
    )
}

pub fn set_config_policy_v2_max_auction_credits_per_update(
    target_program_id: Pubkey,
    authority: Pubkey,
    max_auction_credits_per_update: u64,
) -> Instruction {
    set_config_policy_v2_with_args(
        target_program_id,
        authority,
        SetConfigPolicyV2Args {
            max_auction_credits_per_update,
            ..empty_set_config_policy_v2_args(
                ConfigPolicyV2PatchKind::MAX_AUCTION_CREDITS_PER_UPDATE,
            )
        },
    )
}

pub fn set_config_policy_v2_small_credit_settings(
    target_program_id: Pubkey,
    authority: Pubkey,
    enabled: bool,
    mint: Pubkey,
) -> Instruction {
    set_config_policy_v2_with_args(
        target_program_id,
        authority,
        SetConfigPolicyV2Args {
            small_credit_enabled: u8::from(enabled),
            authority: mint.to_bytes().into(),
            ..empty_set_config_policy_v2_args(ConfigPolicyV2PatchKind::SMALL_CREDIT_SETTINGS)
        },
    )
}

pub fn init_bundle_verifier_page_v2(
    target_program_id: Pubkey,
    payer: Pubkey,
    bundle_escrow: Pubkey,
    page_index: u16,
    bundle_verifier_page_lamports: u64,
) -> Instruction {
    let bundle_verifier_page =
        find_bundle_verifier_page_v2(target_program_id, bundle_escrow, page_index);
    let account_metas = InitBundleVerifierPageV2Accounts {
        payer: &AccountMeta::new(payer, true),
        bundle_escrow: &AccountMeta::new_readonly(bundle_escrow, false),
        bundle_verifier_page: &AccountMeta::new(bundle_verifier_page, false),
        system_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(system_program::ID.to_bytes()),
            false,
        ),
    };

    Instruction {
        program_id: target_program_id,
        data: InitBundleVerifierPageV2Args {
            bundle_verifier_page_lamports,
            page_index,
            _reserved: [0; 6],
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn open_bundle_escrow_v2(
    target_program_id: Pubkey,
    payer: Pubkey,
    bundle_version: u32,
    reward_tier: RequestTier,
    bundle_hash: [u8; 32],
    coordinator: Pubkey,
    requester_refund_recipient: Pubkey,
    total_input_tokens: u64,
    max_output_tokens: u64,
    escrow_lamports: u64,
) -> Instruction {
    open_bundle_escrow_v2_plan(
        target_program_id,
        payer,
        bundle_version,
        reward_tier,
        bundle_hash,
        coordinator,
        requester_refund_recipient,
        total_input_tokens,
        max_output_tokens,
        escrow_lamports,
    )
    .0
}

pub fn commit_auction_settlement_v2(
    target_program_id: Pubkey,
    coordinator: Pubkey,
    bundle_escrow: Pubkey,
    winner_vote_account: Pubkey,
    auction_hash: [u8; 32],
    winner_node_pubkey: Pubkey,
    clearing_price_per_output_token: u64,
) -> Instruction {
    let config_policy = find_config_policy_v2(target_program_id);
    let account_metas = CommitAuctionSettlementV2Accounts {
        coordinator: &AccountMeta::new(coordinator, true),
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        config_policy: &AccountMeta::new(config_policy, false),
        winner_vote_account: &AccountMeta::new(winner_vote_account, false),
    };

    Instruction {
        program_id: target_program_id,
        data: CommitAuctionSettlementV2Args {
            auction_hash,
            winner_node_pubkey: winner_node_pubkey.to_bytes(),
            clearing_price_per_output_token,
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn post_bundle_result_v2(
    target_program_id: Pubkey,
    authority: Pubkey,
    bundle_escrow: Pubkey,
    bundle_verifier_page: Pubkey,
    result_hash: [u8; 32],
    posted_output_tokens: u64,
    page_index: u16,
    page_entries: &[ambient_auction_api::BundleVerifierPageV2Entry],
) -> Instruction {
    build_post_bundle_result_v2_instruction(
        target_program_id,
        authority,
        bundle_escrow,
        Some(bundle_verifier_page),
        result_hash,
        posted_output_tokens,
        page_index,
        page_entries,
        std::convert::identity,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn post_small_bundle_result_v2(
    target_program_id: Pubkey,
    authority: Pubkey,
    bundle_escrow: Pubkey,
    bundle_verifier_page: Pubkey,
    result_hash: [u8; 32],
    posted_output_tokens: u64,
    page_index: u16,
    page_entries: &[ambient_auction_api::BundleVerifierPageV2Entry],
    input_tokens: &[u64],
) -> Instruction {
    assert_eq!(
        page_entries.len(),
        input_tokens.len(),
        "small page entries and input-token values must match"
    );
    assert!(
        input_tokens.len() <= ambient_auction_api::MAX_BUNDLE_VERIFIER_PAGE_V2_ENTRIES,
        "input-token values exceed BundleVerifierPageV2 capacity"
    );
    let mut padded_input_tokens = [0; ambient_auction_api::MAX_BUNDLE_VERIFIER_PAGE_V2_ENTRIES];
    padded_input_tokens[..input_tokens.len()].copy_from_slice(input_tokens);

    build_post_bundle_result_v2_instruction(
        target_program_id,
        authority,
        bundle_escrow,
        Some(bundle_verifier_page),
        result_hash,
        posted_output_tokens,
        page_index,
        page_entries,
        |post| PostSmallBundleResultV2Args {
            post,
            input_tokens: padded_input_tokens,
        },
    )
}

pub fn post_bundle_result_v2_legacy(
    target_program_id: Pubkey,
    authority: Pubkey,
    bundle_escrow: Pubkey,
    result_hash: [u8; 32],
    posted_output_tokens: u64,
) -> Instruction {
    build_post_bundle_result_v2_instruction(
        target_program_id,
        authority,
        bundle_escrow,
        None,
        result_hash,
        posted_output_tokens,
        0,
        &[],
        std::convert::identity,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn finalize_bundle_verification_v2(
    target_program_id: Pubkey,
    coordinator: Pubkey,
    bundle_escrow: Pubkey,
    winner_node: Pubkey,
    requester_refund_recipient: Pubkey,
    verification_hash: [u8; 32],
    accepted_output_tokens: u64,
    winner_payout_lamports: u64,
    verdict: VerificationVerdictV2,
    quorum_verifier_bitmap: u8,
    bundle_verifier_pages: &[Pubkey],
) -> Instruction {
    let page_accounts: Vec<_> = bundle_verifier_pages
        .iter()
        .map(|page| AccountMeta::new(*page, false))
        .collect();

    finalize_bundle_verification_v2_with_remaining_accounts(
        target_program_id,
        coordinator,
        bundle_escrow,
        winner_node,
        requester_refund_recipient,
        verification_hash,
        accepted_output_tokens,
        winner_payout_lamports,
        verdict,
        quorum_verifier_bitmap,
        &page_accounts,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn finalize_bundle_verification_v2_with_remaining_accounts(
    target_program_id: Pubkey,
    coordinator: Pubkey,
    bundle_escrow: Pubkey,
    winner_node: Pubkey,
    requester_refund_recipient: Pubkey,
    verification_hash: [u8; 32],
    accepted_output_tokens: u64,
    winner_payout_lamports: u64,
    verdict: VerificationVerdictV2,
    quorum_verifier_bitmap: u8,
    remaining_accounts: &[AccountMeta],
) -> Instruction {
    let config_policy = find_config_policy_v2(target_program_id);
    let account_metas = FinalizeBundleVerificationV2Accounts {
        coordinator: &AccountMeta::new(coordinator, true),
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        winner_node: &AccountMeta::new(winner_node, false),
        requester_refund_recipient: &AccountMeta::new(requester_refund_recipient, false),
        instructions_sysvar: &AccountMeta::new_readonly(
            solana_sdk::sysvar::instructions::ID,
            false,
        ),
        bundle_verifier_pages: remaining_accounts,
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id: target_program_id,
        data: FinalizeBundleVerificationV2Args {
            verification_hash,
            accepted_output_tokens,
            winner_payout_lamports,
            verdict,
            quorum_verifier_bitmap,
            _reserved: [0; 6],
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn claim_winner_lstake_v2(
    target_program_id: Pubkey,
    bundle_escrow: Pubkey,
    winner_vote_account: Pubkey,
    vote_authority: Pubkey,
) -> Instruction {
    let config_policy = find_config_policy_v2(target_program_id);
    let account_metas = ClaimWinnerLstakeV2Accounts {
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        winner_vote_account: &AccountMeta::new(winner_vote_account, false),
        vote_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(vote::ID.to_bytes()),
            false,
        ),
        vote_authority: &AccountMeta::new(vote_authority, true),
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id: target_program_id,
        data: ClaimWinnerLstakeV2Args {}.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn claim_verifier_lstake_v2(
    target_program_id: Pubkey,
    bundle_escrow: Pubkey,
    verifier_vote_account: Pubkey,
    vote_authority: Pubkey,
    bundle_verifier_pages: &[Pubkey],
) -> Instruction {
    let page_accounts: Vec<_> = bundle_verifier_pages
        .iter()
        .map(|page| AccountMeta::new(*page, false))
        .collect();
    let config_policy = find_config_policy_v2(target_program_id);
    let account_metas = ClaimVerifierLstakeV2Accounts {
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        verifier_vote_account: &AccountMeta::new(verifier_vote_account, false),
        vote_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(vote::ID.to_bytes()),
            false,
        ),
        vote_authority: &AccountMeta::new(vote_authority, true),
        bundle_verifier_pages: &page_accounts,
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id: target_program_id,
        data: ClaimVerifierLstakeV2Args {}.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn expire_bundle_escrow_v2(
    target_program_id: Pubkey,
    bundle_escrow: Pubkey,
    requester_refund_recipient: Pubkey,
) -> Instruction {
    let config_policy = find_config_policy_v2(target_program_id);
    let account_metas = ExpireBundleEscrowV2Accounts {
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        requester_refund_recipient: &AccountMeta::new(requester_refund_recipient, false),
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id: target_program_id,
        data: ExpireBundleEscrowV2Args {}.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

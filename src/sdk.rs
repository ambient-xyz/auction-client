use crate::ID as program_id;
use ambient_auction_api::state::RequestTier;
use ambient_auction_api::{
    instruction::*, MaybePubkey, AUCTION_SEED, BID_SEED, BUNDLE_ESCROW_V2_SEED,
    BUNDLE_REGISTRY_SEED, CONFIG_POLICY_V2_SEED, CONFIG_SEED, JOB_REQUEST_SEED, PUBKEY_BYTES,
    REQUEST_BUNDLE_SEED,
};
use solana_sdk::hash::hashv;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::{Pubkey, MAX_SEED_LEN},
};
use solana_system_interface::program as system_program;
use solana_vote_interface::program as vote;
use std::net::IpAddr;
use std::num::NonZeroU64;

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
    // This is the max allowed within the instruction limits
    let additional_bundles = Some(8);
    let seeds: [&[u8]; 2] = [AUCTION_SEED, &bundle_key.to_bytes()];
    let (parent_auction_key, _parent_auction_bump) =
        Pubkey::find_program_address(&seeds, &program_id);

    let context_length_tier = (context_length_tier as u64).to_le_bytes();
    let expiry_duration_tier = (expiry_duration_tier as u64).to_le_bytes();
    let seeds: [&[u8]; 5] = [
        JOB_REQUEST_SEED,
        &context_length_tier,
        &expiry_duration_tier,
        &authority.to_bytes(),
        &job_request_seed,
    ];
    let (job_request_key, bump) = Pubkey::find_program_address(&seeds, &program_id);

    let seeds: [&[u8]; 2] = [REQUEST_BUNDLE_SEED, &bundle_key.to_bytes()];
    let (new_bundle_key, _) = Pubkey::find_program_address(&seeds, &program_id);

    let seeds: [&[u8]; 2] = [AUCTION_SEED, &new_bundle_key.to_bytes()];
    let (child_auction_key, _) = Pubkey::find_program_address(&seeds, &program_id);

    let mut bundles = vec![
        AccountMeta::new(bundle_key, false),
        AccountMeta::new(parent_auction_key, false),
        AccountMeta::new(new_bundle_key, false),
        AccountMeta::new(child_auction_key, false),
    ];
    let mut current_last = new_bundle_key;
    if let Some(additions) = additional_bundles {
        for _ in 0..additions {
            let seeds: [&[u8]; 2] = [REQUEST_BUNDLE_SEED, &current_last.to_bytes()];
            let (new_bundle_key, _) = Pubkey::find_program_address(&seeds, &program_id);

            let seeds: [&[u8]; 2] = [AUCTION_SEED, &new_bundle_key.to_bytes()];
            let (child_auction_key, _) = Pubkey::find_program_address(&seeds, &program_id);

            bundles.push(AccountMeta::new(new_bundle_key, false));
            bundles.push(AccountMeta::new(child_auction_key, false));
            current_last = new_bundle_key;
        }
    }
    let seeds: [&[u8]; 2] = [REQUEST_BUNDLE_SEED, &current_last.to_bytes()];
    let (last_bundle, _) = Pubkey::find_program_address(&seeds, &program_id);

    let (registry, _) = Pubkey::find_program_address(
        &[
            BUNDLE_REGISTRY_SEED,
            context_length_tier.as_ref(),
            expiry_duration_tier.as_ref(),
        ],
        &program_id,
    );

    #[cfg(feature = "global-config")]
    let (config_key, _bump) = Pubkey::find_program_address(&[CONFIG_SEED], &program_id);

    let accounts_infos = RequestJobAccounts {
        payer: &AccountMeta::new(authority, true),
        job_request: &AccountMeta::new(job_request_key, false),
        registry: &AccountMeta::new(registry, false),
        input_data: &AccountMeta::new(input_data_account.unwrap_or_default(), false),
        system_program: &AccountMeta::new_readonly(solana_system_interface::program::ID, false),
        #[cfg(feature = "global-config")]
        config: &AccountMeta::new(config_key, false),
        bundle_auction_account_pairs: bundles,
        last_bundle: &AccountMeta::new(last_bundle, false),
    };

    let input_data_key: MaybePubkey = input_data_account.map(|key| key.to_bytes().into()).into();

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
    }
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
    // use the bidder pubkey as seed for the price hash
    // update reveal bid if this changes
    let price_hash: [u8; 32] =
        hashv(&[&price_hash_seed, &price_per_output_token.to_le_bytes()]).to_bytes();

    let seeds: &[&[u8]] = &[BID_SEED, &auction.to_bytes(), &authority.to_bytes()];
    let bid = Pubkey::find_program_address(seeds, &program_id).0;

    let account_metas = PlaceBidAccounts {
        payer: &AccountMeta::new(authority, true),
        bid: &AccountMeta::new(bid, false),
        auction: &AccountMeta::new(auction, false),
        system_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(system_program::ID.to_bytes()),
            false,
        ),
    };

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
    }
}
pub fn reveal_bid(
    bidder_key: Pubkey,
    auction_key: Pubkey,
    bundle_key: Pubkey,
    vote_account: Pubkey,
    vote_authority: Pubkey,
    args: RevealBidArgs,
) -> Instruction {
    let seeds: &[&[u8]] = &[BID_SEED, &auction_key.to_bytes(), &bidder_key.to_bytes()];
    let (bid, _) = Pubkey::find_program_address(seeds, &program_id);

    let account_metas = RevealBidAccounts {
        bid_authority: &AccountMeta::new(bidder_key, true),
        bid: &AccountMeta::new(bid, false),
        auction: &AccountMeta::new(auction_key, false),
        bundle: &AccountMeta::new(bundle_key, false),
        vote_account: &AccountMeta::new(vote_account, false),
        vote_authority: &AccountMeta::new(vote_authority, true),
    };

    Instruction {
        program_id,
        data: args.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
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
    let auction_key =
        Pubkey::find_program_address(&[AUCTION_SEED, &bundle_key.to_bytes()], &program_id).0;
    let bid_key = Pubkey::find_program_address(
        &[BID_SEED, &auction_key.to_bytes(), authority.as_ref()],
        &program_id,
    )
    .0;

    let account_metas = SubmitJobOutputAccounts {
        bid_authority: &AccountMeta::new(authority, true),
        bundle: &AccountMeta::new(bundle_key, false),
        job_request: &AccountMeta::new(job_request_key, false),
        bid: &AccountMeta::new_readonly(bid_key, false),
        auction: &AccountMeta::new_readonly(auction_key, false),
        output_data_account: &AccountMeta::new(output_data_account.unwrap_or_default(), false),
    };

    Instruction {
        program_id,
        data: data.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}
pub fn end_auction(signer: Pubkey, bundle_key: Pubkey, vote_account: Pubkey) -> Instruction {
    let auction_key =
        Pubkey::find_program_address(&[AUCTION_SEED, &bundle_key.to_bytes()], &program_id).0;

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
    let context_length_tier_bytes = (context_length_tier as u64).to_le_bytes();
    let expiry_duration_tier_bytes = (expiry_duration_tier as u64).to_le_bytes();
    let (registry, _) = Pubkey::find_program_address(
        &[
            BUNDLE_REGISTRY_SEED,
            context_length_tier_bytes.as_ref(),
            expiry_duration_tier_bytes.as_ref(),
        ],
        &program_id,
    );
    let seeds = [REQUEST_BUNDLE_SEED, bundle_key.as_ref()];
    let (child_bundle, child_bundle_bump) = Pubkey::find_program_address(&seeds, &program_id);

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

    let context_length_tier_bytes = (context_length_tier as u64).to_le_bytes();
    let expiry_duration_tier_bytes = (expiry_duration_tier as u64).to_le_bytes();
    let (registry, _) = Pubkey::find_program_address(
        &[
            BUNDLE_REGISTRY_SEED,
            context_length_tier_bytes.as_ref(),
            expiry_duration_tier_bytes.as_ref(),
        ],
        &program_id,
    );

    let seeds: [&[u8]; 2] = [REQUEST_BUNDLE_SEED, &bundle_key.to_bytes()];
    let (child_bundle_key, new_bundle_bump) = Pubkey::find_program_address(&seeds, &program_id);

    let seeds: [&[u8]; 2] = [AUCTION_SEED, &child_bundle_key.to_bytes()];
    let (child_auction_key, _) = Pubkey::find_program_address(&seeds, &program_id);

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
    let (bundle, bundle_bump) = Pubkey::find_program_address(
        &[
            REQUEST_BUNDLE_SEED,
            (context_length_tier as u64).to_le_bytes().as_ref(),
            (expiry_duration_tier as u64).to_le_bytes().as_ref(),
        ],
        &program_id,
    );
    let (registry, registry_bump) = Pubkey::find_program_address(
        &[
            BUNDLE_REGISTRY_SEED,
            (context_length_tier as u64).to_le_bytes().as_ref(),
            (expiry_duration_tier as u64).to_le_bytes().as_ref(),
        ],
        &program_id,
    );

    let account_metas = InitBundleAccounts {
        payer: &AccountMeta::new(payer, true),
        bundle: &AccountMeta::new(bundle, false),
        registry: &AccountMeta::new(registry, false),
        system_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(system_program::ID.to_bytes()),
            false,
        ),
    };

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
    }
}

pub fn init_config(payer: Pubkey, args: InitConfigArgs) -> Instruction {
    let (config_key, _bump) = Pubkey::find_program_address(&[CONFIG_SEED], &program_id);

    let account_metas = InitConfigAccounts {
        payer: &AccountMeta::new(payer, true),
        config: &AccountMeta::new(config_key, false),
        system_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(system_program::ID.to_bytes()),
            false,
        ),
    };

    Instruction {
        program_id,
        data: args.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn find_config_policy_v2() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED, CONFIG_POLICY_V2_SEED], &program_id).0
}

pub fn init_config_policy_v2(
    authority: Pubkey,
    config_policy_lamports: u64,
    mut policy: ambient_auction_api::ConfigPolicyV2,
) -> Instruction {
    let (config_key, _config_bump) = Pubkey::find_program_address(&[CONFIG_SEED], &program_id);
    let config_policy = find_config_policy_v2();
    policy.bump = 0;

    let account_metas = InitConfigPolicyV2Accounts {
        authority: &AccountMeta::new(authority, true),
        config: &AccountMeta::new(config_key, false),
        config_policy: &AccountMeta::new(config_policy, false),
        system_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(system_program::ID.to_bytes()),
            false,
        ),
    };

    Instruction {
        program_id,
        data: InitConfigPolicyV2Args {
            config_policy_lamports,
            policy,
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn set_config_policy_v2(
    authority: Pubkey,
    mut policy: ambient_auction_api::ConfigPolicyV2,
) -> Instruction {
    let (config_key, _config_bump) = Pubkey::find_program_address(&[CONFIG_SEED], &program_id);
    let config_policy = find_config_policy_v2();
    policy.bump = 0;

    let account_metas = SetConfigPolicyV2Accounts {
        authority: &AccountMeta::new(authority, true),
        config: &AccountMeta::new(config_key, false),
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id,
        data: SetConfigPolicyV2Args { policy }.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn find_bundle_escrow_v2(payer: Pubkey, bundle_hash: [u8; 32], bundle_version: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[
            BUNDLE_ESCROW_V2_SEED,
            payer.as_ref(),
            bundle_hash.as_ref(),
            bundle_version.to_le_bytes().as_ref(),
        ],
        &program_id,
    )
    .0
}

#[allow(clippy::too_many_arguments)]
pub fn open_bundle_escrow_v2(
    payer: Pubkey,
    bundle_version: u32,
    reward_tier: RequestTier,
    bundle_hash: [u8; 32],
    coordinator: Pubkey,
    requester_refund_recipient: Pubkey,
    total_input_tokens: u64,
    max_output_tokens: u64,
    escrow_lamports: u64,
    settlement_deadline_slot: u64,
    result_deadline_slot: u64,
    verification_deadline_slot: u64,
    claim_deadline_slot: u64,
) -> Instruction {
    let bundle_escrow = find_bundle_escrow_v2(payer, bundle_hash, bundle_version);
    let config_policy = find_config_policy_v2();
    let account_metas = OpenBundleEscrowV2Accounts {
        payer: &AccountMeta::new(payer, true),
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        config_policy: &AccountMeta::new(config_policy, false),
        system_program: &AccountMeta::new_readonly(
            Pubkey::new_from_array(system_program::ID.to_bytes()),
            false,
        ),
    };

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
    }
}

pub fn commit_auction_settlement_v2(
    coordinator: Pubkey,
    bundle_escrow: Pubkey,
    winner_vote_account: Pubkey,
    auction_hash: [u8; 32],
    winner_node_pubkey: Pubkey,
    clearing_price_per_output_token: u64,
) -> Instruction {
    let config_policy = find_config_policy_v2();
    let account_metas = CommitAuctionSettlementV2Accounts {
        coordinator: &AccountMeta::new(coordinator, true),
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        config_policy: &AccountMeta::new(config_policy, false),
        winner_vote_account: &AccountMeta::new(winner_vote_account, false),
    };

    Instruction {
        program_id,
        data: CommitAuctionSettlementV2Args {
            auction_hash,
            winner_node_pubkey: winner_node_pubkey.to_bytes(),
            clearing_price_per_output_token,
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn post_bundle_result_v2(
    authority: Pubkey,
    bundle_escrow: Pubkey,
    bundle_verifier_page: Pubkey,
    result_hash: [u8; 32],
    posted_output_tokens: u64,
    page_index: u16,
    page_entries: &[ambient_auction_api::BundleVerifierPageV2Entry],
) -> Instruction {
    assert!(
        page_entries.len() <= ambient_auction_api::BUNDLE_VERIFIER_PAGE_V2_MAX_ENTRIES,
        "page entries exceed BundleVerifierPageV2 capacity"
    );

    let config_policy = find_config_policy_v2();
    let account_metas = PostBundleResultV2Accounts {
        authority: &AccountMeta::new(authority, true),
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        bundle_verifier_page: Some(&AccountMeta::new(bundle_verifier_page, false)),
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id,
        data: PostBundleResultV2Args {
            result_hash,
            posted_output_tokens,
            page_index,
            page_entry_count: page_entries.len() as u16,
            _reserved: [0; 4],
            page_entries: {
                let mut entries = [ambient_auction_api::BundleVerifierPageV2Entry::default();
                    ambient_auction_api::BUNDLE_VERIFIER_PAGE_V2_MAX_ENTRIES];
                for (index, entry) in page_entries.iter().copied().enumerate() {
                    entries[index] = entry;
                }
                entries
            },
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn finalize_bundle_verification_v2(
    coordinator: Pubkey,
    bundle_escrow: Pubkey,
    winner_node: Pubkey,
    requester_refund_recipient: Pubkey,
    verification_hash: [u8; 32],
    accepted_output_tokens: u64,
    verdict: VerificationVerdictV2,
    quorum_verifier_bitmap: u8,
    bundle_verifier_pages: &[Pubkey],
) -> Instruction {
    let page_accounts: Vec<_> = bundle_verifier_pages
        .iter()
        .map(|page| AccountMeta::new(*page, false))
        .collect();
    let config_policy = find_config_policy_v2();
    let account_metas = FinalizeBundleVerificationV2Accounts {
        coordinator: &AccountMeta::new(coordinator, true),
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        winner_node: &AccountMeta::new(winner_node, false),
        requester_refund_recipient: &AccountMeta::new(requester_refund_recipient, false),
        instructions_sysvar: &AccountMeta::new_readonly(
            solana_sdk::sysvar::instructions::ID,
            false,
        ),
        bundle_verifier_pages: &page_accounts,
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id,
        data: FinalizeBundleVerificationV2Args {
            verification_hash,
            accepted_output_tokens,
            verdict,
            quorum_verifier_bitmap,
            _reserved: [0; 6],
        }
        .to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn claim_winner_lstake_v2(
    bundle_escrow: Pubkey,
    winner_vote_account: Pubkey,
    vote_authority: Pubkey,
) -> Instruction {
    let config_policy = find_config_policy_v2();
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
        program_id,
        data: ClaimWinnerLstakeV2Args {}.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn claim_verifier_lstake_v2(
    bundle_escrow: Pubkey,
    verifier_vote_account: Pubkey,
    vote_authority: Pubkey,
    bundle_verifier_pages: &[Pubkey],
) -> Instruction {
    let page_accounts: Vec<_> = bundle_verifier_pages
        .iter()
        .map(|page| AccountMeta::new(*page, false))
        .collect();
    let config_policy = find_config_policy_v2();
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
        program_id,
        data: ClaimVerifierLstakeV2Args {}.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

pub fn expire_bundle_escrow_v2(
    bundle_escrow: Pubkey,
    requester_refund_recipient: Pubkey,
) -> Instruction {
    let config_policy = find_config_policy_v2();
    let account_metas = ExpireBundleEscrowV2Accounts {
        bundle_escrow: &AccountMeta::new(bundle_escrow, false),
        requester_refund_recipient: &AccountMeta::new(requester_refund_recipient, false),
        config_policy: &AccountMeta::new(config_policy, false),
    };

    Instruction {
        program_id,
        data: ExpireBundleEscrowV2Args {}.to_bytes(),
        accounts: account_metas.iter_owned().collect::<Vec<_>>(),
    }
}

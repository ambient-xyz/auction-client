use super::*;
use ambient_auction_api::{
    AuctionInstruction, BUNDLE_ESCROW_V2_SEED, BUNDLE_VERIFIER_PAGE_V2_SEED,
    BundleVerifierPageV2Entry, CONFIG_POLICY_V2_SEED, CONFIG_SEED, ConfigPolicyV2,
    ConfigPolicyV2Flag, ConfigPolicyV2Flags, InitBundleVerifierPageV2Args,
    InitConfigPolicyV2Args, InstructionAccounts, OpenBundleEscrowV2Args, PlaceBidArgs,
    PostBundleResultV2Args, RequestTier, RequestTierConfigV2, RevealBidArgs,
    SetConfigPolicyV2Args, SubmitJobOutputArgs, VerificationVerdictV2,
};
use solana_sdk::{
    instruction::Instruction,
    pubkey::{MAX_SEED_LEN, Pubkey},
};
use std::net::{IpAddr, Ipv4Addr};

fn sample_endpoint() -> (IpAddr, u16) {
    (IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9000)
}

fn sample_reveal_bid_args() -> RevealBidArgs {
    RevealBidArgs {
        price_per_output_token: 77,
        price_hash_seed: [11; 32],
    }
}

fn sample_page_entry() -> BundleVerifierPageV2Entry {
    BundleVerifierPageV2Entry {
        job_id: [7; 32].into(),
        posted_output_tokens: 42,
        accepted_output_tokens: 24,
        assigned_verifiers_token_ranges: [1, 2, 3, 4, 5, 6],
        verifier_reward_tokens: [7, 8, 9],
        verdict: VerificationVerdictV2::Verified,
        verifier_claimed_bitmap: 0b011,
        _reserved: [0; 6],
    }
}

fn instruction_pubkeys(instruction: &Instruction) -> Vec<Pubkey> {
    instruction
        .accounts
        .iter()
        .map(|meta| meta.pubkey)
        .collect()
}

fn owned_account_pubkeys(accounts: impl IntoIterator<Item = Pubkey>) -> Vec<Pubkey> {
    accounts.into_iter().collect()
}

fn find_config_policy_for_program(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED, CONFIG_POLICY_V2_SEED], &program_id).0
}

fn find_bundle_escrow_for_program(
    program_id: Pubkey,
    payer: Pubkey,
    bundle_hash: [u8; 32],
    bundle_version: u32,
) -> Pubkey {
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

fn find_bundle_verifier_page_for_program(
    program_id: Pubkey,
    bundle_escrow: Pubkey,
    page_index: u16,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            BUNDLE_VERIFIER_PAGE_V2_SEED,
            bundle_escrow.as_ref(),
            page_index.to_le_bytes().as_ref(),
        ],
        &program_id,
    )
    .0
}

#[test]
fn flexible_key_inputs_resolve_to_same_pubkeys() {
    let bundle = Pubkey::new_unique();
    let bundle_bytes = bundle.to_bytes();
    assert_eq!(find_auction(bundle), find_auction(bundle_bytes));
    assert_eq!(find_auction(bundle), find_auction(&bundle_bytes));

    let bidder = Pubkey::new_unique();
    let auction = find_auction(bundle);
    let bidder_bytes = bidder.to_bytes();
    assert_eq!(find_bid(auction, bidder), find_bid(auction, bidder_bytes));
    assert_eq!(find_bid(auction, bidder), find_bid(auction, &bidder_bytes));

    let payer = Pubkey::new_unique();
    let payer_bytes = payer.to_bytes();
    let bundle_hash = [3; 32];
    assert_eq!(
        find_bundle_escrow_v2(crate::ID, payer, bundle_hash, 2),
        find_bundle_escrow_v2(crate::ID, &payer_bytes, bundle_hash, 2)
    );

    let bundle_escrow = find_bundle_escrow_v2(crate::ID, payer, bundle_hash, 2);
    let bundle_escrow_bytes = bundle_escrow.to_bytes();
    assert_eq!(
        find_bundle_verifier_page_v2(crate::ID, bundle_escrow, 3),
        find_bundle_verifier_page_v2(crate::ID, &bundle_escrow_bytes, 3)
    );
}

#[test]
fn init_bundle_verifier_page_v2_uses_canonical_page_pda_and_encoded_args() {
    let forced_program_id = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let bundle_escrow = Pubkey::new_unique();
    let page_index = 7;
    let lamports = 12_345;
    let expected_page =
        find_bundle_verifier_page_for_program(forced_program_id, bundle_escrow, page_index);

    let instruction = init_bundle_verifier_page_v2(
        forced_program_id,
        payer,
        bundle_escrow,
        page_index,
        lamports,
    );
    let args = InitBundleVerifierPageV2Args::try_from(&instruction.data[1..]).unwrap();

    assert_eq!(instruction.program_id, forced_program_id);
    assert_eq!(instruction.accounts.len(), 4);
    assert_eq!(instruction.accounts[0].pubkey, payer);
    assert!(instruction.accounts[0].is_signer);
    assert!(instruction.accounts[0].is_writable);
    assert_eq!(instruction.accounts[1].pubkey, bundle_escrow);
    assert!(!instruction.accounts[1].is_writable);
    assert_eq!(instruction.accounts[2].pubkey, expected_page);
    assert!(instruction.accounts[2].is_writable);
    assert_eq!(
        instruction.accounts[3].pubkey,
        Pubkey::new_from_array(solana_system_interface::program::ID.to_bytes())
    );
    assert_eq!(args.bundle_verifier_page_lamports, lamports);
    assert_eq!(args.page_index, page_index);
}

#[test]
fn init_bundle_plan_matches_builder_and_find_helpers() {
    let payer = Pubkey::new_unique();
    let (planned_instruction, account_keys) =
        init_bundle_plan(payer, RequestTier::Standard, RequestTier::Pro, 11, 12);
    let instruction = init_bundle(payer, RequestTier::Standard, RequestTier::Pro, 11, 12);

    assert_eq!(planned_instruction, instruction);
    assert_eq!(account_keys.payer, payer);
    assert_eq!(
        account_keys.bundle,
        find_root_bundle(RequestTier::Standard, RequestTier::Pro)
    );
    assert_eq!(
        account_keys.registry,
        find_bundle_registry(RequestTier::Standard, RequestTier::Pro)
    );
    assert_eq!(
        account_keys.system_program,
        Pubkey::new_from_array(solana_system_interface::program::ID.to_bytes())
    );
    assert_eq!(instruction.accounts[1].pubkey, account_keys.bundle);
    assert_eq!(instruction.accounts[2].pubkey, account_keys.registry);
    assert_eq!(
        owned_account_pubkeys(account_keys.as_accounts().iter_owned()),
        instruction_pubkeys(&instruction)
    );
}

#[test]
fn request_job_plan_matches_builder_and_find_helpers() {
    let authority = Pubkey::new_unique();
    let bundle_key = find_root_bundle(RequestTier::Eco, RequestTier::Small);
    let seed = [5; MAX_SEED_LEN];
    let input_data_account = Some(Pubkey::new_unique());

    let (planned_instruction, account_keys) = request_job_plan(
        authority,
        [1; 32],
        Some([2; 16]),
        seed,
        100,
        200,
        300,
        400,
        bundle_key,
        500,
        RequestTier::Eco,
        RequestTier::Small,
        input_data_account,
        None,
    );
    let instruction = request_job(
        authority,
        [1; 32],
        Some([2; 16]),
        seed,
        100,
        200,
        300,
        400,
        bundle_key,
        500,
        RequestTier::Eco,
        RequestTier::Small,
        input_data_account,
        None,
    );

    assert_eq!(planned_instruction, instruction);
    assert_eq!(account_keys.payer, authority);
    assert_eq!(
        account_keys.job_request,
        find_job_request(authority, RequestTier::Eco, RequestTier::Small, seed)
    );
    assert_eq!(
        account_keys.registry,
        find_bundle_registry(RequestTier::Eco, RequestTier::Small)
    );
    assert_eq!(account_keys.input_data, input_data_account.unwrap());
    assert_eq!(
        account_keys.system_program,
        Pubkey::new_from_array(solana_system_interface::program::ID.to_bytes())
    );
    assert_eq!(account_keys.bundle_auction_account_pairs[0], bundle_key);
    assert_eq!(
        account_keys.bundle_auction_account_pairs[1],
        find_auction(bundle_key)
    );
    assert_eq!(
        account_keys.bundle_auction_account_pairs[2],
        find_child_bundle(bundle_key)
    );
    assert_eq!(
        account_keys.bundle_auction_account_pairs[3],
        find_auction(account_keys.bundle_auction_account_pairs[2])
    );
    assert_eq!(instruction.accounts[1].pubkey, account_keys.job_request);
    assert_eq!(instruction.accounts[2].pubkey, account_keys.registry);
    #[cfg(not(feature = "global-config"))]
    {
        assert_eq!(
            instruction.accounts[5].pubkey,
            account_keys.bundle_auction_account_pairs[0]
        );
        assert_eq!(
            instruction.accounts[6].pubkey,
            account_keys.bundle_auction_account_pairs[1]
        );
        assert_eq!(
            instruction.accounts[7].pubkey,
            account_keys.bundle_auction_account_pairs[2]
        );
        assert_eq!(
            instruction.accounts[8].pubkey,
            account_keys.bundle_auction_account_pairs[3]
        );
    }
    #[cfg(feature = "global-config")]
    {
        assert_eq!(
            instruction.accounts[6].pubkey,
            account_keys.bundle_auction_account_pairs[0]
        );
        assert_eq!(
            instruction.accounts[7].pubkey,
            account_keys.bundle_auction_account_pairs[1]
        );
        assert_eq!(
            instruction.accounts[8].pubkey,
            account_keys.bundle_auction_account_pairs[2]
        );
        assert_eq!(
            instruction.accounts[9].pubkey,
            account_keys.bundle_auction_account_pairs[3]
        );
    }
    assert_eq!(
        instruction.accounts.last().unwrap().pubkey,
        account_keys.last_bundle
    );
    assert_eq!(
        owned_account_pubkeys(account_keys.as_accounts().iter_owned()),
        instruction_pubkeys(&instruction)
    );
}

#[test]
fn place_bid_plan_matches_builder_and_find_helpers() {
    let authority = Pubkey::new_unique();
    let auction = Pubkey::new_unique();
    let endpoint = sample_endpoint();
    let (planned_instruction, account_keys) = place_bid_plan(
        &authority,
        auction.to_bytes(),
        99,
        [8; 32],
        endpoint,
        Some([9; 32]),
    );
    let instruction = place_bid(authority, auction, 99, [8; 32], endpoint, Some([9; 32]));
    let planned_args = PlaceBidArgs::try_from(&planned_instruction.data[1..]).unwrap();
    let instruction_args = PlaceBidArgs::try_from(&instruction.data[1..]).unwrap();

    assert_eq!(planned_instruction.accounts, instruction.accounts);
    assert_eq!(planned_args.price_hash, instruction_args.price_hash);
    assert_eq!(planned_args.authority, instruction_args.authority);
    assert_eq!(std::net::IpAddr::from(planned_args.ip), endpoint.0);
    assert_eq!(std::net::IpAddr::from(instruction_args.ip), endpoint.0);
    assert_eq!(planned_args.port, endpoint.1);
    assert_eq!(instruction_args.port, endpoint.1);
    assert_eq!(
        planned_args.encryption_node_public_key,
        instruction_args.encryption_node_public_key
    );
    assert_eq!(account_keys.payer, authority);
    assert_eq!(account_keys.bid, find_bid(auction, authority));
    assert_eq!(account_keys.auction, auction);
    assert_eq!(instruction.accounts[1].pubkey, account_keys.bid);
    assert_eq!(
        owned_account_pubkeys(account_keys.as_accounts().iter_owned()),
        instruction_pubkeys(&instruction)
    );
}

#[test]
fn reveal_bid_plan_matches_builder_and_find_helpers() {
    let bidder = Pubkey::new_unique();
    let bundle = Pubkey::new_unique();
    let auction = find_auction(bundle);
    let vote_account = Pubkey::new_unique();
    let vote_authority = Pubkey::new_unique();
    let args = sample_reveal_bid_args();

    let (planned_instruction, account_keys) = reveal_bid_plan(
        bidder.to_bytes(),
        &auction,
        bundle,
        &vote_account,
        vote_authority.to_bytes(),
        args,
    );
    let instruction = reveal_bid(bidder, auction, bundle, vote_account, vote_authority, args);

    assert_eq!(planned_instruction, instruction);
    assert_eq!(account_keys.bid_authority, bidder);
    assert_eq!(account_keys.bid, find_bid(auction, bidder));
    assert_eq!(account_keys.auction, auction);
    assert_eq!(account_keys.bundle, bundle);
    assert_eq!(account_keys.vote_account, vote_account);
    assert_eq!(account_keys.vote_authority, vote_authority);
    assert_eq!(instruction.accounts[1].pubkey, account_keys.bid);
    assert_eq!(
        owned_account_pubkeys(account_keys.as_accounts().iter_owned()),
        instruction_pubkeys(&instruction)
    );
}

#[test]
fn submit_job_plan_matches_builder_and_find_helpers() {
    let authority = Pubkey::new_unique();
    let bundle = Pubkey::new_unique();
    let job_request = Pubkey::new_unique();
    let output_data_account = Some(Pubkey::new_unique());
    let args = SubmitJobOutputArgs {
        output_token_count: 1,
        input_token_count: 2,
        merkle_root: [3; 32],
        output_hash: [4; 32],
        merkle_root_iv: [5; 16],
        output_hash_iv: [6; 16],
        encryption_node_publickey: [7; 32],
    };

    let (planned_instruction, account_keys) = submit_job_plan(
        &authority,
        bundle.to_bytes(),
        &job_request,
        args,
        output_data_account,
    );
    let instruction = submit_job(authority, bundle, job_request, args, output_data_account);

    assert_eq!(planned_instruction, instruction);
    assert_eq!(account_keys.bid_authority, authority);
    assert_eq!(account_keys.bundle, bundle);
    assert_eq!(account_keys.job_request, job_request);
    assert_eq!(account_keys.auction, find_auction(bundle));
    assert_eq!(account_keys.bid, find_bid(account_keys.auction, authority));
    assert_eq!(
        account_keys.output_data_account,
        output_data_account.unwrap()
    );
    assert_eq!(instruction.accounts[3].pubkey, account_keys.bid);
    assert_eq!(instruction.accounts[4].pubkey, account_keys.auction);
    assert_eq!(
        owned_account_pubkeys(account_keys.as_accounts().iter_owned()),
        instruction_pubkeys(&instruction)
    );
}

#[test]
fn open_bundle_escrow_v2_plan_matches_builder_and_find_helpers() {
    let payer = Pubkey::new_unique();
    let coordinator = Pubkey::new_unique();
    let requester_refund_recipient = Pubkey::new_unique();
    let bundle_hash = [4; 32];

    let (planned_instruction, account_keys) = open_bundle_escrow_v2_plan(
        crate::ID,
        payer.to_bytes(),
        2,
        RequestTier::Pro,
        bundle_hash,
        &coordinator,
        requester_refund_recipient.to_bytes(),
        100,
        200,
        300,
    );
    let instruction = open_bundle_escrow_v2(
        crate::ID,
        payer,
        2,
        RequestTier::Pro,
        bundle_hash,
        coordinator,
        requester_refund_recipient,
        100,
        200,
        300,
    );

    assert_eq!(planned_instruction, instruction);
    assert_eq!(account_keys.payer, payer);
    assert_eq!(
        account_keys.bundle_escrow,
        find_bundle_escrow_v2(crate::ID, payer, bundle_hash, 2)
    );
    assert_eq!(account_keys.config_policy, find_config_policy_v2(crate::ID));
    assert_eq!(instruction.accounts[1].pubkey, account_keys.bundle_escrow);
    assert_eq!(instruction.accounts[2].pubkey, account_keys.config_policy);
    assert_eq!(
        owned_account_pubkeys(account_keys.as_accounts().iter_owned()),
        instruction_pubkeys(&instruction)
    );

    let args = OpenBundleEscrowV2Args::try_from(&instruction.data[1..]).unwrap();
    assert_eq!(args.reward_tier, u64::from(RequestTier::Pro));
    assert_eq!(args.total_input_tokens, 100);
    assert_eq!(args.max_output_tokens, 200);
    assert_eq!(args.escrow_lamports, 300);
}

#[test]
fn post_bundle_result_v2_keeps_page_account_and_encoded_entries() {
    let authority = Pubkey::new_unique();
    let bundle_escrow = Pubkey::new_unique();
    let bundle_verifier_page = Pubkey::new_unique();
    let entry = sample_page_entry();

    let instruction = post_bundle_result_v2(
        crate::ID,
        authority,
        bundle_escrow,
        bundle_verifier_page,
        [8; 32],
        55,
        3,
        &[entry],
    );
    let args = PostBundleResultV2Args::try_from(&instruction.data[1..]).unwrap();

    assert_eq!(instruction.accounts.len(), 4);
    assert_eq!(instruction.accounts[0].pubkey, authority);
    assert_eq!(instruction.accounts[1].pubkey, bundle_escrow);
    assert_eq!(
        instruction.accounts[2].pubkey,
        find_config_policy_v2(crate::ID)
    );
    assert_eq!(instruction.accounts[3].pubkey, bundle_verifier_page);
    assert_eq!(args.result_hash, [8; 32]);
    assert_eq!(args.posted_output_tokens, 55);
    assert_eq!(args.page_index, 3);
    assert_eq!(args.page_entry_count, 1);
    assert_eq!(args.page_entries[0], entry);
}

#[test]
fn post_bundle_result_v2_legacy_omits_page_account_and_uses_zero_page_fields() {
    let authority = Pubkey::new_unique();
    let bundle_escrow = Pubkey::new_unique();

    let instruction =
        post_bundle_result_v2_legacy(crate::ID, authority, bundle_escrow, [9; 32], 66);
    let args = PostBundleResultV2Args::try_from(&instruction.data[1..]).unwrap();

    assert_eq!(instruction.accounts.len(), 3);
    assert_eq!(instruction.accounts[0].pubkey, authority);
    assert_eq!(instruction.accounts[1].pubkey, bundle_escrow);
    assert_eq!(
        instruction.accounts[2].pubkey,
        find_config_policy_v2(crate::ID)
    );
    assert_eq!(args.result_hash, [9; 32]);
    assert_eq!(args.posted_output_tokens, 66);
    assert_eq!(args.page_index, 0);
    assert_eq!(args.page_entry_count, 0);
    assert_eq!(
        args.page_entries,
        [BundleVerifierPageV2Entry::default();
            ambient_auction_api::BUNDLE_VERIFIER_PAGE_V2_MAX_ENTRIES]
    );
}

#[test]
fn v2_key_helpers_accept_explicit_program_id() {
    let forced_program_id = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let bundle_hash = [4; 32];

    let expected_config_policy = find_config_policy_for_program(forced_program_id);
    let expected_bundle_escrow =
        find_bundle_escrow_for_program(forced_program_id, payer, bundle_hash, 2);

    assert_eq!(
        find_config_policy_v2(forced_program_id),
        expected_config_policy
    );
    assert_eq!(
        find_bundle_escrow_v2(forced_program_id, payer, bundle_hash, 2),
        expected_bundle_escrow
    );
    assert_ne!(find_config_policy_v2(crate::ID), expected_config_policy);
    assert_ne!(
        find_bundle_escrow_v2(crate::ID, payer, bundle_hash, 2),
        expected_bundle_escrow
    );
}

#[test]
fn open_bundle_escrow_v2_plan_supports_explicit_program_id() {
    let forced_program_id = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let coordinator = Pubkey::new_unique();
    let requester_refund_recipient = Pubkey::new_unique();
    let bundle_hash = [4; 32];

    let (planned_instruction, account_keys) = open_bundle_escrow_v2_plan(
        forced_program_id,
        payer.to_bytes(),
        2,
        RequestTier::Pro,
        bundle_hash,
        &coordinator,
        requester_refund_recipient.to_bytes(),
        100,
        200,
        300,
    );
    let instruction = open_bundle_escrow_v2(
        forced_program_id,
        payer,
        2,
        RequestTier::Pro,
        bundle_hash,
        coordinator,
        requester_refund_recipient,
        100,
        200,
        300,
    );
    let expected_bundle_escrow =
        find_bundle_escrow_for_program(forced_program_id, payer, bundle_hash, 2);
    let expected_config_policy = find_config_policy_for_program(forced_program_id);

    assert_eq!(planned_instruction, instruction);
    assert_eq!(instruction.program_id, forced_program_id);
    assert_eq!(account_keys.bundle_escrow, expected_bundle_escrow);
    assert_eq!(account_keys.config_policy, expected_config_policy);
    assert_eq!(instruction.accounts[1].pubkey, expected_bundle_escrow);
    assert_eq!(instruction.accounts[2].pubkey, expected_config_policy);
}

#[test]
fn v2_instruction_builders_accept_explicit_program_id() {
    let forced_program_id = Pubkey::new_unique();
    let expected_config_policy = find_config_policy_for_program(forced_program_id);
    let payer = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let service_authority = Pubkey::new_unique();
    let bundle_escrow = Pubkey::new_unique();
    let winner_vote_account = Pubkey::new_unique();
    let winner_node = Pubkey::new_unique();
    let refund_recipient = Pubkey::new_unique();
    let verifier_page = Pubkey::new_unique();
    let verifier_vote_account = Pubkey::new_unique();
    let vote_authority = Pubkey::new_unique();

    let init_config_policy = init_config_policy_v2(
        forced_program_id,
        payer,
        11,
        authority,
        service_authority,
        ConfigPolicyV2::default(),
    );
    assert_eq!(init_config_policy.program_id, forced_program_id);
    assert_eq!(init_config_policy.accounts[0].pubkey, payer);
    assert_eq!(
        init_config_policy.accounts[1].pubkey,
        expected_config_policy
    );
    assert_eq!(
        init_config_policy.data[0],
        AuctionInstruction::InitConfigPolicyV2 as u8
    );
    assert_eq!(
        init_config_policy.data.len(),
        1 + std::mem::size_of::<InitConfigPolicyV2Args>()
    );

    let set_config_policy = set_config_policy_v2_flags(
        forced_program_id,
        authority,
        ConfigPolicyV2Flags::from_flag(ConfigPolicyV2Flag::AllowServiceCommitOverride),
    );
    assert_eq!(set_config_policy.program_id, forced_program_id);
    assert_eq!(set_config_policy.accounts[1].pubkey, expected_config_policy);
    assert_eq!(
        set_config_policy.data[0],
        AuctionInstruction::SetConfigPolicyV2 as u8
    );

    let commit = commit_auction_settlement_v2(
        forced_program_id,
        authority,
        bundle_escrow,
        winner_vote_account,
        [1; 32],
        winner_node,
        17,
    );
    assert_eq!(commit.program_id, forced_program_id);
    assert_eq!(commit.accounts[2].pubkey, expected_config_policy);

    let post = post_bundle_result_v2(
        forced_program_id,
        authority,
        bundle_escrow,
        verifier_page,
        [2; 32],
        19,
        3,
        &[sample_page_entry()],
    );
    assert_eq!(post.program_id, forced_program_id);
    assert_eq!(post.accounts[2].pubkey, expected_config_policy);

    let legacy_post =
        post_bundle_result_v2_legacy(forced_program_id, authority, bundle_escrow, [3; 32], 23);
    assert_eq!(legacy_post.program_id, forced_program_id);
    assert_eq!(legacy_post.accounts[2].pubkey, expected_config_policy);

    let finalize = finalize_bundle_verification_v2(
        forced_program_id,
        authority,
        bundle_escrow,
        winner_node,
        refund_recipient,
        [4; 32],
        29,
        17,
        VerificationVerdictV2::Verified,
        0b011,
        &[verifier_page],
    );
    assert_eq!(finalize.program_id, forced_program_id);
    assert_eq!(finalize.accounts[5].pubkey, expected_config_policy);

    let claim_winner = claim_winner_lstake_v2(
        forced_program_id,
        bundle_escrow,
        winner_vote_account,
        vote_authority,
    );
    assert_eq!(claim_winner.program_id, forced_program_id);
    assert_eq!(claim_winner.accounts[4].pubkey, expected_config_policy);

    let claim_verifier = claim_verifier_lstake_v2(
        forced_program_id,
        bundle_escrow,
        verifier_vote_account,
        vote_authority,
        &[verifier_page],
    );
    assert_eq!(claim_verifier.program_id, forced_program_id);
    assert_eq!(claim_verifier.accounts[4].pubkey, expected_config_policy);

    let expire = expire_bundle_escrow_v2(forced_program_id, bundle_escrow, refund_recipient);
    assert_eq!(expire.program_id, forced_program_id);
    assert_eq!(expire.accounts[2].pubkey, expected_config_policy);
}

#[test]
fn set_config_policy_v2_helpers_emit_packet_safe_patch_instructions() {
    let program_id = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let replacement_authority = Pubkey::new_unique();
    let expected_config_policy = find_config_policy_for_program(program_id);
    let tier_config = RequestTierConfigV2::from_request_tier(RequestTier::Small);

    let instructions = [
        set_config_policy_v2_flags(
            program_id,
            authority,
            ConfigPolicyV2Flags::from_flag(ConfigPolicyV2Flag::AllowServiceCommitOverride),
        ),
        set_config_policy_v2_admin_authority(program_id, authority, 0, replacement_authority),
        set_config_policy_v2_service_authority(program_id, authority, 0, replacement_authority),
        set_config_policy_v2_verifier_settings(program_id, authority, 2, 1),
        set_config_policy_v2_tier_config(program_id, authority, RequestTier::Small, tier_config),
        set_config_policy_v2_max_auction_credits_per_update(program_id, authority, 10),
    ];

    for instruction in instructions {
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(
            instruction.data[0],
            AuctionInstruction::SetConfigPolicyV2 as u8
        );
        assert_eq!(
            instruction.data.len(),
            1 + std::mem::size_of::<SetConfigPolicyV2Args>()
        );
        assert!(instruction.data.len() < 256);
        assert_eq!(instruction.accounts[0].pubkey, authority);
        assert!(instruction.accounts[0].is_signer);
        assert_eq!(instruction.accounts[1].pubkey, expected_config_policy);
    }
}

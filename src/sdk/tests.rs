use super::*;
use ambient_auction_api::{
    BundleVerifierPageV2Entry, InstructionAccounts, PlaceBidArgs, PostBundleResultV2Args,
    RequestTier, RevealBidArgs, SubmitJobOutputArgs, VerificationVerdictV2,
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
        find_bundle_escrow_v2(payer, bundle_hash, 2),
        find_bundle_escrow_v2(&payer_bytes, bundle_hash, 2)
    );
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
        payer.to_bytes(),
        2,
        RequestTier::Pro,
        bundle_hash,
        &coordinator,
        requester_refund_recipient.to_bytes(),
        100,
        200,
        300,
        400,
        500,
        600,
        700,
    );
    let instruction = open_bundle_escrow_v2(
        payer,
        2,
        RequestTier::Pro,
        bundle_hash,
        coordinator,
        requester_refund_recipient,
        100,
        200,
        300,
        400,
        500,
        600,
        700,
    );

    assert_eq!(planned_instruction, instruction);
    assert_eq!(account_keys.payer, payer);
    assert_eq!(
        account_keys.bundle_escrow,
        find_bundle_escrow_v2(payer, bundle_hash, 2)
    );
    assert_eq!(account_keys.config_policy, find_config_policy_v2());
    assert_eq!(instruction.accounts[1].pubkey, account_keys.bundle_escrow);
    assert_eq!(instruction.accounts[2].pubkey, account_keys.config_policy);
    assert_eq!(
        owned_account_pubkeys(account_keys.as_accounts().iter_owned()),
        instruction_pubkeys(&instruction)
    );
}

#[test]
fn post_bundle_result_v2_keeps_page_account_and_encoded_entries() {
    let authority = Pubkey::new_unique();
    let bundle_escrow = Pubkey::new_unique();
    let bundle_verifier_page = Pubkey::new_unique();
    let entry = sample_page_entry();

    let instruction = post_bundle_result_v2(
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
    assert_eq!(instruction.accounts[2].pubkey, find_config_policy_v2());
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

    let instruction = post_bundle_result_v2_legacy(authority, bundle_escrow, [9; 32], 66);
    let args = PostBundleResultV2Args::try_from(&instruction.data[1..]).unwrap();

    assert_eq!(instruction.accounts.len(), 3);
    assert_eq!(instruction.accounts[0].pubkey, authority);
    assert_eq!(instruction.accounts[1].pubkey, bundle_escrow);
    assert_eq!(instruction.accounts[2].pubkey, find_config_policy_v2());
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

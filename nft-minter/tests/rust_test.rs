pub mod constants;
pub mod nft_minter_interactor;

use constants::*;
use elrond_wasm::types::ManagedByteArray;
use elrond_wasm_debug::{managed_biguint, managed_buffer, rust_biguint, DebugApi};
use nft_minter::common_storage::{BrandInfo, MintPrice, COLLECTION_HASH_LEN};
use nft_minter::nft_module::NftModule;
use nft_minter::royalties::RoyaltiesModule;
use nft_minter_interactor::*;

#[test]
fn init_test() {
    let _ = NftMinterSetup::new(nft_minter::contract_obj);
}

#[test]
fn create_brands_test() {
    let mut nm_setup = NftMinterSetup::new(nft_minter::contract_obj);
    nm_setup.create_default_brands();

    // try create brand, same collection
    nm_setup
        .call_create_new_brand(
            FIRST_COLLECTION_HASH,
            THIRD_BRAND_ID,
            b"png",
            0,
            5,
            1,
            b"EGLD",
            1,
            b"",
            b"TICKER",
            &[],
        )
        .assert_user_error("Collection hash already exists");

    // try create brand, same brand ID
    nm_setup
        .call_create_new_brand(
            THIRD_COLLECTION_HASH,
            FIRST_BRAND_ID,
            b"png",
            0,
            5,
            1,
            b"EGLD",
            1,
            b"",
            b"TICKER",
            &[],
        )
        .assert_user_error("Brand already exists");

    // try create brand, unsupported media type
    nm_setup
        .call_create_new_brand(
            THIRD_COLLECTION_HASH,
            THIRD_BRAND_ID,
            b"exe",
            0,
            5,
            1,
            b"EGLD",
            1,
            b"",
            b"TICKER",
            &[],
        )
        .assert_user_error("Invalid media type");

    // get brand by id
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let result = sc.get_brand_info_view(managed_buffer!(FIRST_BRAND_ID));

            let expected_brand_id = managed_buffer!(FIRST_BRAND_ID);
            assert_eq!(result.brand_id, expected_brand_id);

            let expected_brand_info = BrandInfo::<DebugApi> {
                collection_hash: ManagedByteArray::<DebugApi, COLLECTION_HASH_LEN>::new_from_bytes(
                    FIRST_COLLECTION_HASH,
                ),
                token_display_name: managed_buffer!(FIRST_TOKEN_DISPLAY_NAME),
                media_type: managed_buffer!(FIRST_MEDIA_TYPE),
                royalties: managed_biguint!(0),
            };
            assert_eq!(result.brand_info, expected_brand_info);

            let expected_mint_price = MintPrice::<DebugApi> {
                start_timestamp: FIRST_MINT_START_TIMESTAMP,
                token_id: managed_token_id!(FIRST_MINT_PRICE_TOKEN_ID),
                amount: managed_biguint!(FIRST_MINT_PRICE_AMOUNT),
            };
            assert_eq!(result.mint_price, expected_mint_price);

            let expected_available_nfts = FIRST_MAX_NFTS;
            assert_eq!(result.available_nfts, expected_available_nfts);

            let expected_total_available_nfts = FIRST_MAX_NFTS;
            assert_eq!(result.total_nfts, expected_total_available_nfts);
        })
        .assert_ok();
}

#[test]
fn buy_random_nft_test() {
    let mut nm_setup = NftMinterSetup::new(nft_minter::contract_obj);
    nm_setup.create_default_brands();

    // try buy before start
    let first_user_addr = nm_setup.first_user_address.clone();
    nm_setup
        .call_buy_random_nft(
            &first_user_addr,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT,
            FIRST_BRAND_ID,
            1,
        )
        .assert_user_error("May not mint yet");

    nm_setup
        .b_mock
        .set_block_timestamp(FIRST_MINT_START_TIMESTAMP);

    // buy random nft ok
    let first_user_addr = nm_setup.first_user_address.clone();
    nm_setup
        .call_buy_random_nft(
            &first_user_addr,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT,
            FIRST_BRAND_ID,
            1,
        )
        .assert_ok();

    // user receives token with nonce 1, and ID 2
    let expected_attributes = nm_setup.build_nft_attributes_first_token(2);
    nm_setup.b_mock.check_nft_balance(
        &first_user_addr,
        FIRST_TOKEN_ID,
        1,
        &rust_biguint!(1),
        Some(&expected_attributes),
    );

    // check unique ID mapper internal consistency
    // ID 2 was removed, so pos 2 should have the last item, i.e. ID 5
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.available_ids(&managed_buffer!(FIRST_BRAND_ID));
            assert_eq!(mapper.len(), 4);
            assert_eq!(mapper.get(1), 1);
            assert_eq!(mapper.get(2), 5);
            assert_eq!(mapper.get(3), 3);
            assert_eq!(mapper.get(4), 4);
        })
        .assert_ok();

    // buy multiple NFTs - wrong payment amount
    let second_user_address = nm_setup.second_user_address.clone();
    nm_setup
        .call_buy_random_nft(
            &second_user_address,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT,
            FIRST_BRAND_ID,
            2,
        )
        .assert_user_error("Invalid payment");

    // try buy too many
    nm_setup
        .call_buy_random_nft(
            &second_user_address,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT * 5,
            FIRST_BRAND_ID,
            5,
        )
        .assert_user_error("Not enough NFTs available");

    // buy 2 ok
    nm_setup
        .call_buy_random_nft(
            &second_user_address,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT * 2,
            FIRST_BRAND_ID,
            2,
        )
        .assert_ok();

    // second user gets ID 3 and 1
    let expected_attributes_first = nm_setup.build_nft_attributes_first_token(3);
    let expected_attributes_second = nm_setup.build_nft_attributes_first_token(1);
    nm_setup.b_mock.check_nft_balance(
        &second_user_address,
        FIRST_TOKEN_ID,
        2,
        &rust_biguint!(1),
        Some(&expected_attributes_first),
    );
    nm_setup.b_mock.check_nft_balance(
        &second_user_address,
        FIRST_TOKEN_ID,
        3,
        &rust_biguint!(1),
        Some(&expected_attributes_second),
    );

    // check unique ID mapper internal consistency
    // ID 3 was removed, and then ID 1, so mapper would look like this
    // initially: 1 5 3 4
    // after first rand (3 removed): 1 5 4
    // after second rand (1 removed): 4 5
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.available_ids(&managed_buffer!(FIRST_BRAND_ID));
            assert_eq!(mapper.len(), 2);
            assert_eq!(mapper.get(1), 4);
            assert_eq!(mapper.get(2), 5);
        })
        .assert_ok();

    // claim user payments
    let owner_addr = nm_setup.owner_address.clone();
    nm_setup
        .b_mock
        .execute_tx(&owner_addr, &nm_setup.nm_wrapper, &rust_biguint!(0), |sc| {
            let result = sc.claim_mint_payments();
            let (egld_amt, other_payments) = result.into_tuple();

            assert_eq!(egld_amt, managed_biguint!(3 * FIRST_MINT_PRICE_AMOUNT));
            assert!(other_payments.is_empty());
        })
        .assert_ok();

    let owner_balance_before = OWNER_EGLD_BALANCE - 2 * ISSUE_COST;
    let expected_balance = owner_balance_before + 3 * FIRST_MINT_PRICE_AMOUNT;
    nm_setup
        .b_mock
        .check_egld_balance(&owner_addr, &rust_biguint!(expected_balance));
}

#[test]
fn giveaway_test() {
    let mut nm_setup = NftMinterSetup::new(nft_minter::contract_obj);
    nm_setup.create_default_brands();

    // giveaway single nft
    let first_user_addr = nm_setup.first_user_address.clone();
    nm_setup
        .call_giveaway(SECOND_BRAND_ID, [(first_user_addr.clone(), 1)].to_vec())
        .assert_ok();

    // user received nonce 1 and ID 7
    let mut attr = nm_setup.build_nft_attributes_second_token(7);
    nm_setup.b_mock.check_nft_balance(
        &first_user_addr,
        SECOND_TOKEN_ID,
        1,
        &rust_biguint!(1),
        Some(&attr),
    );

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.available_ids(&managed_buffer!(SECOND_BRAND_ID));
            assert_eq!(mapper.len(), 9);
            assert_eq!(mapper.get(1), 1);
            assert_eq!(mapper.get(2), 2);
            assert_eq!(mapper.get(3), 3);
            assert_eq!(mapper.get(4), 4);
            assert_eq!(mapper.get(5), 5);
            assert_eq!(mapper.get(6), 6);
            assert_eq!(mapper.get(7), 10); // this changed
            assert_eq!(mapper.get(8), 8);
            assert_eq!(mapper.get(9), 9);
        })
        .assert_ok();

    // giveaway two, single user
    nm_setup
        .call_giveaway(SECOND_BRAND_ID, [(first_user_addr.clone(), 2)].to_vec())
        .assert_ok();

    // received ID 4 and 5
    attr = nm_setup.build_nft_attributes_second_token(4);
    nm_setup.b_mock.check_nft_balance(
        &first_user_addr,
        SECOND_TOKEN_ID,
        2,
        &rust_biguint!(1),
        Some(&attr),
    );

    attr = nm_setup.build_nft_attributes_second_token(5);
    nm_setup.b_mock.check_nft_balance(
        &first_user_addr,
        SECOND_TOKEN_ID,
        3,
        &rust_biguint!(1),
        Some(&attr),
    );

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.available_ids(&managed_buffer!(SECOND_BRAND_ID));
            assert_eq!(mapper.len(), 7);
            assert_eq!(mapper.get(1), 1);
            assert_eq!(mapper.get(2), 2);
            assert_eq!(mapper.get(3), 3);
            assert_eq!(mapper.get(4), 9); // pos 4
            assert_eq!(mapper.get(5), 8); // and 5 changed
            assert_eq!(mapper.get(6), 6);
            assert_eq!(mapper.get(7), 10);
        })
        .assert_ok();

    // giveaway, multiple users
    let second_user_addr = nm_setup.second_user_address.clone();
    nm_setup
        .call_giveaway(
            SECOND_BRAND_ID,
            [(first_user_addr.clone(), 2), (second_user_addr.clone(), 1)].to_vec(),
        )
        .assert_ok();

    // user 1 received IDs 10 and 1
    attr = nm_setup.build_nft_attributes_second_token(10);
    nm_setup.b_mock.check_nft_balance(
        &first_user_addr,
        SECOND_TOKEN_ID,
        4,
        &rust_biguint!(1),
        Some(&attr),
    );

    attr = nm_setup.build_nft_attributes_second_token(1);
    nm_setup.b_mock.check_nft_balance(
        &first_user_addr,
        SECOND_TOKEN_ID,
        5,
        &rust_biguint!(1),
        Some(&attr),
    );

    // user 2 received ID
    attr = nm_setup.build_nft_attributes_second_token(6);
    nm_setup.b_mock.check_nft_balance(
        &second_user_addr,
        SECOND_TOKEN_ID,
        6,
        &rust_biguint!(1),
        Some(&attr),
    );

    // mapper progress:
    // 1 2 3 9 8 6 10
    // 1 2 3 9 8 6
    // 6 2 3 9 8
    // 8 2 3 9
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.available_ids(&managed_buffer!(SECOND_BRAND_ID));
            assert_eq!(mapper.len(), 4);
            assert_eq!(mapper.get(1), 8);
            assert_eq!(mapper.get(2), 2);
            assert_eq!(mapper.get(3), 3);
            assert_eq!(mapper.get(4), 9);
        })
        .assert_ok();
}

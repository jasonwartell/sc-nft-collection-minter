pub mod constants;
pub mod nft_minter_interactor;

use constants::*;
use elrond_wasm::storage::mappers::StorageTokenWrapper;
use elrond_wasm::types::{ManagedBuffer, ManagedByteArray, MultiValueEncoded, ManagedVec};
use elrond_wasm_debug::testing_framework::BlockchainStateWrapper;
use elrond_wasm_debug::{managed_address, managed_biguint, managed_buffer, rust_biguint, DebugApi};
use nft_minter::brand_creation::BrandCreationModule;
use nft_minter::common_storage::{BrandInfo, MintPrice, TimePeriod, CommonStorageModule};
use nft_minter::nft_attributes_builder::{NftAttributesBuilderModule, COLLECTION_HASH_LEN};
use nft_minter::nft_tier::NftTierModule;
use nft_minter::royalties::RoyaltiesModule;
use nft_minter::views::{TierInfoEntry, ViewsModule};
use nft_minter::NftMinter;
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
            1,
            2,
            b"EGLD",
            1,
            b"",
            b"TICKER",
            &[],
            FIRST_TIERS,
            FIRST_NFT_AMOUNTS,
            0,
        )
        .assert_user_error("Collection hash already exists");

    // try create brand, same brand ID
    nm_setup
        .call_create_new_brand(
            THIRD_COLLECTION_HASH,
            FIRST_BRAND_ID,
            b"png",
            0,
            1,
            2,
            b"EGLD",
            1,
            b"",
            b"TICKER",
            &[],
            FIRST_TIERS,
            FIRST_NFT_AMOUNTS,
            0,
        )
        .assert_user_error("Brand already exists");

    // try create brand, unsupported media type
    nm_setup
        .call_create_new_brand(
            THIRD_COLLECTION_HASH,
            THIRD_BRAND_ID,
            b"exe",
            0,
            1,
            2,
            b"EGLD",
            1,
            b"",
            b"TICKER",
            &[],
            FIRST_TIERS,
            FIRST_NFT_AMOUNTS,
            0,
        )
        .assert_user_error("Invalid media type");

    // get brand by id
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let result = sc.get_brand_info_view(managed_buffer!(FIRST_BRAND_ID));

            let expected_brand_id = managed_buffer!(FIRST_BRAND_ID);
            assert_eq!(result.brand_id, expected_brand_id);

            let expected_token_id = managed_token_id!(FIRST_TOKEN_ID);
            assert_eq!(result.nft_token_id, expected_token_id.unwrap_esdt());

            let expected_brand_info = BrandInfo::<DebugApi> {
                collection_hash: ManagedByteArray::<DebugApi, COLLECTION_HASH_LEN>::new_from_bytes(
                    FIRST_COLLECTION_HASH,
                ),
                token_display_name: managed_buffer!(FIRST_TOKEN_DISPLAY_NAME),
                media_type: managed_buffer!(FIRST_MEDIA_TYPE),
                royalties: managed_biguint!(0),
                mint_period: TimePeriod {
                    start: FIRST_MINT_START_TIMESTAMP,
                    end: FIRST_MINT_END_TIMESTAMP,
                },
                whitelist_expire_timestamp: 0,
            };
            assert_eq!(result.brand_info, expected_brand_info);

            let mut expected_tier_info = Vec::new();
            for (tier, nft_amount) in FIRST_TIERS.iter().zip(FIRST_NFT_AMOUNTS.iter()) {
                expected_tier_info.push(TierInfoEntry::<DebugApi> {
                    tier: managed_buffer!(tier.clone()),
                    available_nfts: *nft_amount,
                    total_nfts: *nft_amount,
                    mint_price: MintPrice::<DebugApi> {
                        token_id: managed_token_id!(FIRST_MINT_PRICE_TOKEN_ID),
                        amount: managed_biguint!(FIRST_MINT_PRICE_AMOUNT),
                    },
                });
            }
            assert_eq!(
                result.tier_info_entries.as_slice(),
                expected_tier_info.as_slice()
            );
        })
        .assert_ok();
}

#[test]
fn buy_random_nft_test() {
    let mut nm_setup = NftMinterSetup::new(nft_minter::contract_obj);
    nm_setup.create_default_brands();

    let first_tier = FIRST_TIERS[0];

    // verify no payments received
    let owner_addr = nm_setup.owner_address.clone();
    nm_setup
        .b_mock
        .execute_tx(&owner_addr, &nm_setup.nm_wrapper, &rust_biguint!(0), |sc| {
            let result = sc.claim_mint_payments();
            let (egld_amt, other_payments) = result.into_tuple();

            assert_eq!(egld_amt, managed_biguint!(0));
            assert!(other_payments.is_empty());
        })
        .assert_ok();
/*
let result = sc.claim_mint_payments();
            let (egld_amt, other_payments) = result.into_tuple();

            assert_eq!(egld_amt, managed_biguint!(3 * FIRST_MINT_PRICE_AMOUNT));
            assert!(other_payments.is_empty());
*/
    // try buy before start
    let first_user_addr = nm_setup.first_user_address.clone();
    nm_setup
        .call_buy_random_nft(
            &first_user_addr,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT,
            FIRST_BRAND_ID,
            first_tier,
            1,
        )
        .assert_user_error("May not mint yet");

    // verify no payments received
    let owner_addr = nm_setup.owner_address.clone();
    nm_setup
        .b_mock
        .execute_tx(&owner_addr, &nm_setup.nm_wrapper, &rust_biguint!(0), |sc| {
            let result = sc.claim_mint_payments();
            let (egld_amt, other_payments) = result.into_tuple();

            assert_eq!(egld_amt, managed_biguint!(0));
            assert!(other_payments.is_empty());
        })
        .assert_ok();
    
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
            first_tier,
            1,
        )
        .assert_ok();

    // claim payment from 1st nft purchase
    let owner_addr = nm_setup.owner_address.clone();
    nm_setup
        .b_mock
        .execute_tx(&owner_addr, &nm_setup.nm_wrapper, &rust_biguint!(0), |sc| {
            let result = sc.claim_mint_payments();
            let (egld_amt, other_payments) = result.into_tuple();

            assert_eq!(egld_amt, managed_biguint!(FIRST_MINT_PRICE_AMOUNT));
            assert!(other_payments.is_empty());
        })
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
            let mapper = sc.available_ids(
                &managed_buffer!(FIRST_BRAND_ID),
                &managed_buffer!(first_tier),
            );
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
            first_tier,
            2,
        )
        .assert_user_error("Invalid payment");

    // verify no payments received
    let owner_addr = nm_setup.owner_address.clone();
    nm_setup
        .b_mock
        .execute_tx(&owner_addr, &nm_setup.nm_wrapper, &rust_biguint!(0), |sc| {
            let result = sc.claim_mint_payments();
            let (egld_amt, other_payments) = result.into_tuple();

            assert_eq!(egld_amt, managed_biguint!(0));
            assert!(other_payments.is_empty());
        })
        .assert_ok();

    // try buy too many - over max limit
    nm_setup
        .call_buy_random_nft(
            &second_user_address,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT * 5,
            FIRST_BRAND_ID,
            first_tier,
            3,
        )
        .assert_user_error("Max NFTs per transaction limit exceeded");

    // verify no payments received
    let owner_addr = nm_setup.owner_address.clone();
    nm_setup
        .b_mock
        .execute_tx(&owner_addr, &nm_setup.nm_wrapper, &rust_biguint!(0), |sc| {
            let result = sc.claim_mint_payments();
            let (egld_amt, other_payments) = result.into_tuple();

            assert_eq!(egld_amt, managed_biguint!(0));
            assert!(other_payments.is_empty());
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_tx(
            &nm_setup.owner_address,
            &nm_setup.nm_wrapper,
            &rust_biguint!(0),
            |sc| {
                sc.set_max_nfts_per_transaction(1_000);
            },
        )
        .assert_ok();

    // try buy too many - not enough available
    nm_setup
        .call_buy_random_nft(
            &second_user_address,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT * 5,
            FIRST_BRAND_ID,
            first_tier,
            5,
        )
        .assert_user_error("Not enough NFTs available");

    // verify no payments received
    let owner_addr = nm_setup.owner_address.clone();
    nm_setup
        .b_mock
        .execute_tx(&owner_addr, &nm_setup.nm_wrapper, &rust_biguint!(0), |sc| {
            let result = sc.claim_mint_payments();
            let (egld_amt, other_payments) = result.into_tuple();

            assert_eq!(egld_amt, managed_biguint!(0));
            assert!(other_payments.is_empty());
        })
        .assert_ok();

    // buy 2 ok
    nm_setup
        .call_buy_random_nft(
            &second_user_address,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT * 2,
            FIRST_BRAND_ID,
            first_tier,
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
            let mapper = sc.available_ids(
                &managed_buffer!(FIRST_BRAND_ID),
                &managed_buffer!(first_tier),
            );
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

            assert_eq!(egld_amt, managed_biguint!(2 * FIRST_MINT_PRICE_AMOUNT));
            assert!(other_payments.is_empty());
        })
        .assert_ok();

    let owner_balance_before = OWNER_EGLD_BALANCE - 2 * ISSUE_COST;
    let expected_balance = owner_balance_before + 3 * FIRST_MINT_PRICE_AMOUNT;
    nm_setup
        .b_mock
        .check_egld_balance(&owner_addr, &rust_biguint!(expected_balance));

    // try buy after deadline
    nm_setup
        .b_mock
        .set_block_timestamp(FIRST_MINT_END_TIMESTAMP);

    nm_setup
        .call_buy_random_nft(
            &first_user_addr,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT,
            FIRST_BRAND_ID,
            first_tier,
            1,
        )
        .assert_user_error("May not mint after deadline");
}

#[test]
fn buy_whitelist_test() {
    let mut nm_setup = NftMinterSetup::new(nft_minter::contract_obj);
    let first_tier = FIRST_TIERS[0];
    let first_user_addr = nm_setup.first_user_address.clone();

    nm_setup.create_default_brands();
    nm_setup
        .b_mock
        .set_block_timestamp(FIRST_MINT_START_TIMESTAMP);

    nm_setup
        .b_mock
        .execute_tx(
            &nm_setup.owner_address.clone(),
            &nm_setup.nm_wrapper,
            &rust_biguint!(0),
            |sc| {
                sc.set_mint_whitelist_expire_timestamp(
                    managed_buffer!(FIRST_BRAND_ID),
                    FIRST_MINT_START_TIMESTAMP + 1,
                );
            },
        )
        .assert_ok();

    // try buy, not in whitelist
    nm_setup
        .call_buy_random_nft(
            &first_user_addr,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT,
            FIRST_BRAND_ID,
            first_tier,
            1,
        )
        .assert_user_error("Not in whitelist");

    nm_setup
        .b_mock
        .execute_tx(
            &nm_setup.owner_address.clone(),
            &nm_setup.nm_wrapper,
            &rust_biguint!(0),
            |sc| {
                let mut args = MultiValueEncoded::new();
                args.push(managed_address!(&first_user_addr));
                sc.add_to_whitelist(managed_buffer!(FIRST_BRAND_ID), args);
            },
        )
        .assert_ok();

    // buy ok

    nm_setup
        .call_buy_random_nft(
            &first_user_addr,
            FIRST_MINT_PRICE_TOKEN_ID,
            FIRST_MINT_PRICE_AMOUNT,
            FIRST_BRAND_ID,
            first_tier,
            1,
        )
        .assert_ok();
}

#[test]
fn giveaway_test() {
    let mut nm_setup = NftMinterSetup::new(nft_minter::contract_obj);
    nm_setup.create_default_brands();

    let first_tier = SECOND_TIERS[0];

    // giveaway single nft
    let first_user_addr = nm_setup.first_user_address.clone();
    nm_setup
        .call_giveaway(
            SECOND_BRAND_ID,
            first_tier,
            [(first_user_addr.clone(), 1)].to_vec(),
        )
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
            let mapper = sc.available_ids(
                &managed_buffer!(SECOND_BRAND_ID),
                &managed_buffer!(first_tier),
            );
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
        .call_giveaway(
            SECOND_BRAND_ID,
            first_tier,
            [(first_user_addr.clone(), 2)].to_vec(),
        )
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
            let mapper = sc.available_ids(
                &managed_buffer!(SECOND_BRAND_ID),
                &managed_buffer!(first_tier),
            );
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
            first_tier,
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
            let mapper = sc.available_ids(
                &managed_buffer!(SECOND_BRAND_ID),
                &managed_buffer!(first_tier),
            );
            assert_eq!(mapper.len(), 4);
            assert_eq!(mapper.get(1), 8);
            assert_eq!(mapper.get(2), 2);
            assert_eq!(mapper.get(3), 3);
            assert_eq!(mapper.get(4), 9);
        })
        .assert_ok();
}

#[test]
fn formatters_test() {
    let mut nm_setup = NftMinterSetup::new(nft_minter::contract_obj);
    nm_setup.create_default_brands();

    // test NFT attributes
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let attributes = sc.build_nft_attributes(
                &ManagedByteArray::new_from_bytes(FIRST_COLLECTION_HASH),
                &managed_buffer!(FIRST_BRAND_ID),
                2,
            );

            let expected_attributes =
                "metadata:FirstCollection_______________________________/2.json;tags:funny,sad,memes";
            assert_eq!(managed_buffer_to_string(&attributes), expected_attributes.to_string());
        })
        .assert_ok();

    // test generated URIs
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let coll_hash = ManagedByteArray::new_from_bytes(FIRST_COLLECTION_HASH);
            let nft_main_file_uri =
                sc.build_nft_main_file_uri(&coll_hash, 2, &managed_buffer!(b"jpg"));
            let expected_main_file_uri =
                "https://ipfs.io/ipfs/FirstCollection_______________________________/2.jpg";
            assert_eq!(
                managed_buffer_to_string(&nft_main_file_uri),
                expected_main_file_uri.to_string()
            );

            let nft_json_file_uri = sc.build_nft_json_file_uri(&coll_hash, 2);
            let expected_nft_json_uri =
                "https://ipfs.io/ipfs/FirstCollection_______________________________/2.json";
            assert_eq!(
                managed_buffer_to_string(&nft_json_file_uri),
                expected_nft_json_uri.to_string()
            );

            let collection_json_uri = sc.build_collection_json_file_uri(&coll_hash);
            let expected_collection_json_uri = "https://ipfs.io/ipfs/FirstCollection_______________________________/collection.json";
            assert_eq!(
                managed_buffer_to_string(&collection_json_uri),
                expected_collection_json_uri.to_string()
            );
        })
        .assert_ok();
}

#[test]
fn custom_test() {
    let mut nm_setup = NftMinterSetup::new(nft_minter::contract_obj);
    let mut b_mock = BlockchainStateWrapper::new();
    let owner_address = b_mock.create_user_account(&rust_biguint!(OWNER_EGLD_BALANCE));
    
    // check contract storage after init()
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.collections_category();
            assert_eq!(mapper.get(), managed_buffer!(CATEGORY));
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.royalties_claim_address();
            assert_eq!(mapper.get(), managed_address!(&owner_address));
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.mint_payments_claim_address();
            assert_eq!(mapper.get(), managed_address!(&owner_address));
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.max_nfts_per_transaction();
            assert_eq!(mapper.get(), 2);
        })
        .assert_ok();
    
    nm_setup.create_custom_brand();

    // check contract storage after brand creation()
    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.available_ids(
                &managed_buffer!(CUSTOM_BRAND_ID),
                &managed_buffer!(CUSTOM_TIERS[0]),
            );
            assert_eq!(mapper.len(), 3);
            assert_eq!(mapper.get(1), 1);
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.nft_token(
                &managed_buffer!(CUSTOM_BRAND_ID),
            );
            assert_eq!(mapper.get_token_id(), (CUSTOM_TOKEN_ID).into());
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.nft_tiers_for_brand(
                &managed_buffer!(CUSTOM_BRAND_ID),
            );
            assert_eq!(mapper.contains(&managed_buffer!(CUSTOM_TIERS[0])), true);
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.total_nfts(
                &managed_buffer!(CUSTOM_BRAND_ID),
                &managed_buffer!(CUSTOM_TIERS[0])
            );
            assert_eq!(mapper.get(), 3);
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.nft_id_offset_for_tier(
                &managed_buffer!(CUSTOM_BRAND_ID),
                &managed_buffer!(CUSTOM_TIERS[0])
            );
            assert_eq!(mapper.get(), 0);
        })
        .assert_ok();
 
   

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.price_for_tier(
                &managed_buffer!(CUSTOM_BRAND_ID),
                &managed_buffer!(CUSTOM_TIERS[0])
            );

            let custom_mint_price = MintPrice::<DebugApi> {
                token_id: managed_token_id!(CUSTOM_MINT_PRICE_TOKEN_ID),
                amount: managed_biguint!(CUSTOM_MINT_PRICE_AMOUNT),
            };

            assert_eq!(mapper.get(), custom_mint_price);
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.registered_brands();
            assert_eq!(mapper.contains(&managed_buffer!(CUSTOM_BRAND_ID)), true);
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.registered_collection_hashes();

            let collection_hash = 
                ManagedByteArray::<DebugApi, COLLECTION_HASH_LEN>::new_from_bytes(
                    CUSTOM_COLLECTION_HASH
                );
            assert_eq!(mapper.contains(&collection_hash), true);
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let mapper = sc.tags_for_brand(
                &managed_buffer!(CUSTOM_BRAND_ID),
            );

            let mut custom_tag = ManagedVec::new();
            custom_tag.push(managed_buffer!(CUSTOM_TAGS[0]));
            assert_eq!(mapper.get(), custom_tag);
        })
        .assert_ok();

    nm_setup
        .b_mock
        .execute_query(&nm_setup.nm_wrapper, |sc| {
            let result = sc.get_brand_info_view(managed_buffer!(CUSTOM_BRAND_ID));

            let expected_brand_id = managed_buffer!(CUSTOM_BRAND_ID);
            assert_eq!(result.brand_id, expected_brand_id);

            let expected_token_id = managed_token_id!(CUSTOM_TOKEN_ID);
            assert_eq!(result.nft_token_id, expected_token_id.unwrap_esdt());

            let expected_brand_info = BrandInfo::<DebugApi> {
                collection_hash: ManagedByteArray::<DebugApi, COLLECTION_HASH_LEN>::new_from_bytes(
                    CUSTOM_COLLECTION_HASH,
                ),
                token_display_name: managed_buffer!(CUSTOM_TOKEN_DISPLAY_NAME),
                media_type: managed_buffer!(CUSTOM_MEDIA_TYPE),
                royalties: managed_biguint!(CUSTOM_ROYALTIES),
                mint_period: TimePeriod {
                    start: CUSTOM_MINT_START_TIMESTAMP,
                    end: CUSTOM_MINT_END_TIMESTAMP,
                },
                whitelist_expire_timestamp: CUSTOM_WHITELIST_EXPIRE_TIMESTAMP,
            };
            assert_eq!(result.brand_info, expected_brand_info);

            let mut expected_tier_info = Vec::new();
            for (tier, nft_amount) in CUSTOM_TIERS.iter().zip(CUSTOM_NFT_AMOUNTS.iter()) {
                expected_tier_info.push(TierInfoEntry::<DebugApi> {
                    tier: managed_buffer!(tier.clone()),
                    available_nfts: *nft_amount,
                    total_nfts: *nft_amount,
                    mint_price: MintPrice::<DebugApi> {
                        token_id: managed_token_id!(CUSTOM_MINT_PRICE_TOKEN_ID),
                        amount: managed_biguint!(CUSTOM_MINT_PRICE_AMOUNT),
                    },
                });
            }
            assert_eq!(
                result.tier_info_entries.as_slice(),
                expected_tier_info.as_slice()
            );
        })
        .assert_ok();

}

fn managed_buffer_to_string(buffer: &ManagedBuffer<DebugApi>) -> String {
    String::from_utf8(buffer.to_boxed_bytes().into_vec()).unwrap()
}

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use crate::{
    common_storage::{BrandId, BrandInfo, CollectionHash, MintPrice, Tag},
    unique_id_mapper::{UniqueId, UniqueIdMapper},
};

const NFT_AMOUNT: u32 = 1;
const NFT_ISSUE_COST: u64 = 50_000_000_000_000_000; // 0.05 EGLD
const ROYALTIES_MAX: u32 = 10_000; // 100%
const VEC_MAPPER_FIRST_ITEM_INDEX: usize = 1;

const MAX_BRAND_ID_LEN: usize = 50;
static INVALID_BRAND_ID_ERR_MSG: &[u8] = b"Invalid Brand ID";

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct BrandInfoViewResultType<M: ManagedTypeApi> {
    pub brand_id: BrandId<M>,
    pub brand_info: BrandInfo<M>,
    pub mint_price: MintPrice<M>,
    pub available_nfts: usize,
    pub total_nfts: usize,
}

#[derive(TopEncode, TopDecode)]
pub struct TempCallbackStorageInfo<M: ManagedTypeApi> {
    pub brand_info: BrandInfo<M>,
    pub price_for_brand: MintPrice<M>,
    pub max_nfts: usize,
    pub tags: ManagedVec<M, Tag<M>>,
}

#[elrond_wasm::module]
pub trait NftModule:
    crate::common_storage::CommonStorageModule
    + crate::admin_whitelist::AdminWhitelistModule
    + crate::nft_attributes_builder::NftAttributesBuilderModule
    + crate::royalties::RoyaltiesModule
{
    #[payable("EGLD")]
    #[endpoint(issueTokenForBrand)]
    fn issue_token_for_brand(
        &self,
        collection_hash: CollectionHash<Self::Api>,
        brand_id: BrandId<Self::Api>,
        media_type: ManagedBuffer,
        royalties: BigUint,
        max_nfts: usize,
        mint_start_timestamp: u64,
        mint_price_token_id: TokenIdentifier,
        mint_price_amount: BigUint,
        token_display_name: ManagedBuffer,
        token_ticker: ManagedBuffer,
        #[var_args] tags: MultiValueEncoded<Tag<Self::Api>>,
    ) {
        self.require_caller_is_admin();

        let id_len = brand_id.len();
        require!(
            id_len > 0 && id_len <= MAX_BRAND_ID_LEN,
            INVALID_BRAND_ID_ERR_MSG
        );

        let payment_amount = self.call_value().egld_value();
        require!(
            payment_amount == NFT_ISSUE_COST,
            "Invalid payment amount. Issue costs exactly 0.05 EGLD"
        );

        require!(
            self.is_supported_media_type(&media_type),
            "Invalid media type"
        );
        require!(royalties <= ROYALTIES_MAX, "Royalties cannot be over 100%");
        require!(max_nfts > 0, "Cannot create brand with max 0 items");
        require!(
            mint_price_token_id.is_egld() || mint_price_token_id.is_valid_esdt_identifier(),
            "Invalid price token"
        );

        let is_new_collection = self
            .registered_collection_hashes()
            .insert(collection_hash.clone());
        require!(is_new_collection, "Collection hash already exists");

        let is_new_brand = self.registered_brands().insert(brand_id.clone());
        require!(is_new_brand, "Brand already exists");

        let brand_info = BrandInfo {
            collection_hash: collection_hash.clone(),
            token_display_name: token_display_name.clone(),
            media_type,
            royalties,
        };
        let price_for_brand = MintPrice {
            start_timestamp: mint_start_timestamp,
            token_id: mint_price_token_id,
            amount: mint_price_amount,
        };
        self.temporary_callback_storage(&brand_id)
            .set(&TempCallbackStorageInfo {
                brand_info,
                price_for_brand,
                max_nfts,
                tags: tags.to_vec(),
            });

        self.nft_token(&brand_id).issue(
            EsdtTokenType::NonFungible,
            payment_amount,
            token_display_name,
            token_ticker,
            0,
            Some(self.callbacks().issue_callback(collection_hash, brand_id)),
        );
    }

    #[callback]
    fn issue_callback(
        &self,
        collection_hash: CollectionHash<Self::Api>,
        brand_id: BrandId<Self::Api>,
        #[call_result] result: ManagedAsyncCallResult<TokenIdentifier>,
    ) {
        match result {
            ManagedAsyncCallResult::Ok(token_id) => {
                let cb_info: TempCallbackStorageInfo<Self::Api> =
                    self.temporary_callback_storage(&brand_id).get();

                self.nft_token(&brand_id).set_token_id(&token_id);
                self.brand_info(&brand_id).set(&cb_info.brand_info);
                self.price_for_brand(&brand_id)
                    .set(&cb_info.price_for_brand);
                self.total_nfts(&brand_id).set(cb_info.max_nfts);
                self.available_ids(&brand_id)
                    .set_initial_len(cb_info.max_nfts);

                if !cb_info.tags.is_empty() {
                    self.tags_for_brand(&brand_id).set(&cb_info.tags);
                }
            }
            ManagedAsyncCallResult::Err(_) => {
                let _ = self.registered_brands().swap_remove(&brand_id);
                let _ = self
                    .registered_collection_hashes()
                    .swap_remove(&collection_hash);
            }
        }

        self.temporary_callback_storage(&brand_id).clear();
    }

    #[endpoint(setLocalRoles)]
    fn set_local_roles(&self, brand_id: BrandId<Self::Api>) {
        self.nft_token(&brand_id)
            .set_local_roles(&[EsdtLocalRole::NftCreate], None);
    }

    #[payable("*")]
    #[endpoint(buyRandomNft)]
    fn buy_random_nft(
        &self,
        brand_id: BrandId<Self::Api>,
        #[var_args] opt_nfts_to_buy: OptionalValue<usize>,
    ) {
        require!(
            self.registered_brands().contains(&brand_id),
            INVALID_BRAND_ID_ERR_MSG
        );

        let nfts_to_buy = match opt_nfts_to_buy {
            OptionalValue::Some(val) => {
                if val == 0 {
                    return;
                }

                val
            }
            OptionalValue::None => NFT_AMOUNT as usize,
        };

        let price_for_brand: MintPrice<Self::Api> = self.price_for_brand(&brand_id).get();
        let payment: EsdtTokenPayment<Self::Api> = self.call_value().payment();
        let total_required_amount = &price_for_brand.amount * (nfts_to_buy as u32);
        require!(
            payment.token_identifier == price_for_brand.token_id
                && payment.amount == total_required_amount,
            "Invalid payment"
        );

        let current_timestamp = self.blockchain().get_block_timestamp();
        require!(
            current_timestamp >= price_for_brand.start_timestamp,
            "May not mint yet"
        );

        self.add_mint_payment(payment.token_identifier, payment.amount);

        let caller = self.blockchain().get_caller();
        let brand_info: BrandInfo<Self::Api> = self.brand_info(&brand_id).get();
        self.mint_and_send_random_nft(&caller, &brand_id, &brand_info, nfts_to_buy);
    }

    #[endpoint(giveawayNfts)]
    fn giveaway_nfts(
        &self,
        brand_id: BrandId<Self::Api>,
        #[var_args] dest_amount_pairs: MultiValueEncoded<MultiValue2<ManagedAddress, usize>>,
    ) {
        self.require_caller_is_admin();

        require!(
            self.registered_brands().contains(&brand_id),
            INVALID_BRAND_ID_ERR_MSG
        );

        let brand_info = self.brand_info(&brand_id).get();
        for pair in dest_amount_pairs {
            let (dest_address, nfts_to_send) = pair.into_tuple();
            if nfts_to_send > 0 {
                self.mint_and_send_random_nft(&dest_address, &brand_id, &brand_info, nfts_to_send);
            }
        }
    }

    fn mint_and_send_random_nft(
        &self,
        to: &ManagedAddress,
        brand_id: &BrandId<Self::Api>,
        brand_info: &BrandInfo<Self::Api>,
        nfts_to_send: usize,
    ) {
        let total_available_nfts = self.available_ids(brand_id).len();
        require!(
            nfts_to_send <= total_available_nfts,
            "Not enough NFTs available"
        );

        let nft_token_id = self.nft_token(brand_id).get_token_id();
        let mut nft_output_payments = ManagedVec::new();
        for _ in 0..nfts_to_send {
            let nft_id = self.get_next_random_id(brand_id);
            let nft_uri = self.build_nft_main_file_uri(
                &brand_info.collection_hash,
                nft_id,
                &brand_info.media_type,
            );
            let nft_json = self.build_nft_json_file_uri(&brand_info.collection_hash, nft_id);
            let collection_json = self.build_collection_json_file_uri(&brand_info.collection_hash);

            let mut uris = ManagedVec::new();
            uris.push(nft_uri);
            uris.push(nft_json);
            uris.push(collection_json);

            let attributes =
                self.build_nft_attributes(&brand_info.collection_hash, brand_id, nft_id);
            let nft_amount = BigUint::from(NFT_AMOUNT);
            let nft_nonce = self.send().esdt_nft_create(
                &nft_token_id,
                &nft_amount,
                &brand_info.token_display_name,
                &brand_info.royalties,
                &ManagedBuffer::new(),
                &attributes,
                &uris,
            );

            nft_output_payments.push(EsdtTokenPayment::new(
                nft_token_id.clone(),
                nft_nonce,
                nft_amount,
            ));
        }

        self.send().direct_multi(to, &nft_output_payments, &[]);
    }

    fn get_next_random_id(&self, brand_id: &BrandId<Self::Api>) -> UniqueId {
        let mut id_mapper = self.available_ids(brand_id);
        let last_id_index = id_mapper.len();
        require!(last_id_index > 0, "No more NFTs available for brand");

        let rand_index = self.get_random_usize(VEC_MAPPER_FIRST_ITEM_INDEX, last_id_index + 1);
        id_mapper.get_and_swap_remove(rand_index)
    }

    /// range is [min, max)
    fn get_random_usize(&self, min: usize, max: usize) -> usize {
        let mut rand_source = RandomnessSource::<Self::Api>::new();
        rand_source.next_usize_in_range(min, max)
    }

    #[view(getBrandInfo)]
    fn get_brand_info_view(
        &self,
        brand_id: BrandId<Self::Api>,
    ) -> BrandInfoViewResultType<Self::Api> {
        require!(
            self.registered_brands().contains(&brand_id),
            INVALID_BRAND_ID_ERR_MSG
        );

        let brand_info = self.brand_info(&brand_id).get();
        let mint_price = self.price_for_brand(&brand_id).get();
        let available_nfts = self.available_ids(&brand_id).len();
        let total_nfts = self.total_nfts(&brand_id).get();

        BrandInfoViewResultType {
            brand_id,
            brand_info,
            mint_price,
            available_nfts,
            total_nfts,
        }
    }

    #[view(getAllBrandsInfo)]
    fn get_all_brands_info(&self) -> MultiValueEncoded<BrandInfoViewResultType<Self::Api>> {
        let mut result = MultiValueEncoded::new();
        for brand_id in self.registered_brands().iter() {
            let brand_info_entry = self.get_brand_info_view(brand_id);
            result.push(brand_info_entry);
        }

        result
    }

    #[view(getNftTokenIdForBrand)]
    #[storage_mapper("nftTokenId")]
    fn nft_token(&self, brand_id: &BrandId<Self::Api>) -> NonFungibleTokenMapper<Self::Api>;

    #[storage_mapper("totalNfts")]
    fn total_nfts(&self, brand_id: &BrandId<Self::Api>) -> SingleValueMapper<usize>;

    #[storage_mapper("availableIds")]
    fn available_ids(&self, brand_id: &BrandId<Self::Api>) -> UniqueIdMapper<Self::Api>;

    #[storage_mapper("temporaryCallbackStorage")]
    fn temporary_callback_storage(
        &self,
        brand_id: &BrandId<Self::Api>,
    ) -> SingleValueMapper<TempCallbackStorageInfo<Self::Api>>;
}

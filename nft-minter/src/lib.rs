#![no_std]

elrond_wasm::imports!();

pub mod admin_whitelist;
pub mod common_storage;
pub mod nft_attributes_builder;
pub mod nft_marketplace_interactor;
pub mod nft_module;
pub mod royalties;
pub mod unique_id_mapper;

use common_storage::CollectionId;

#[elrond_wasm::contract]
pub trait NftMinter:
    common_storage::CommonStorageModule
    + admin_whitelist::AdminWhitelistModule
    + nft_module::NftModule
    + nft_attributes_builder::NftAttributesBuilderModule
    + royalties::RoyaltiesModule
    + nft_marketplace_interactor::NftMarketplaceInteractorModule
{
    #[init]
    fn init(
        &self,
        parent_collection_id: CollectionId<Self::Api>,
        royalties_claim_address: ManagedAddress,
        mint_payments_claim_address: ManagedAddress,
    ) {
        require!(!parent_collection_id.is_empty(), "Invalid collection ID");

        self.parent_collection_id().set(&parent_collection_id);
        self.royalties_claim_address().set(&royalties_claim_address);
        self.mint_payments_claim_address()
            .set(&mint_payments_claim_address);
    }
}

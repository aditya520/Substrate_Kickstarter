use parity_codec::{Decode, Encode};
use runtime_primitives::traits::{As, Hash};
use support::{
	decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap, StorageValue,
};
use system::ensure_signed;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Campaign<Hash, Balance> {
	id: Hash,
	value: Balance,
	approvalcount: u64,
	minimumcontribution: u64,
	completed: bool,
}

pub trait Trait: balances::Trait {}

decl_storage! {
	trait Store for Module<T: Trait> as KickstartModule {
		Campaigns get(campaign): map T::Hash => Campaign<T::Hash, T::Balance>;
		CampaignOwner get (owner_of_campaign): map T::Hash => Option<T::AccountId>;
		OwnedCampaign get(campaign_of_owner): map T::AccountId => T::Hash;

		Nonce: u64;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn create_campaign(origin) -> Result {
			let sender = ensure_signed(origin)?;

			let nonce = <Nonce<T>>::get();
			let random_hash = (<system::Module<T>>::random_seed(), &sender, nonce)
				.using_encoded(<T as system::Trait>::Hashing::hash);

			ensure!(!<CampaignOwner<T>>::exists(random_hash), "Campaign already exists");

			let new_campaign = Campaign {
				id: <T as system::Trait>::Hashing::hash_of(&0),
				value: <T::Balance as As<u64>>::sa(0),
				minimumcontribution: 0,
				approvalcount: 0,
				completed: false,
			};

			<Campaigns<T>>::insert(random_hash,new_campaign);
			<CampaignOwner<T>>::insert(random_hash,&sender);
			<OwnedCampaign<T>>::insert(&sender,random_hash);

			<Nonce<T>>::mutate(|n| *n += 1);

			Ok(())
		}
	}
}

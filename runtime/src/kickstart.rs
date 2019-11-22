use parity_codec::{Decode, Encode};
use runtime_primitives::traits::{As, Hash};
use support::{decl_event, decl_module, decl_storage, dispatch::Result, StorageValue, StorageMap};
use system::ensure_signed;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Campaign<Hash, Balance> {
	id: Hash,
	value: Balance,
	approvalcount: u64,
	completed: bool,
}

pub trait Trait: balances::Trait {}

decl_storage! {
	trait Store for Module<T: Trait> as KickstartModule {
		CampaignOwner get(campaign_of_owner): map T::AccountId => Campaign<T::Hash, T::Balance>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn create_campaign(origin) -> Result {
			let sender = ensure_signed(origin)?;

			let new_campaign = Campaign {
				id: <T as system::Trait>::Hashing::hash_of(&0),
				value: <T::Balance as As<u64>>::sa(0),
				approvalcount: 0,
				completed: false,
			};
			<CampaignOwner<T>>::insert(&sender,new_campaign);

			Ok(())
		}
	}
}

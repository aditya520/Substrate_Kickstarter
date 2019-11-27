use parity_codec::{Decode, Encode};
use runtime_primitives::traits::{As, Hash};
use support::{
	decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap, StorageValue, traits::Currency,
};
use system::ensure_signed;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Campaign<Hash, Balance, AccountId> {
	id: Hash,
	owner: AccountId,
	targetprice: Balance,
	balance: Balance,
	approvalcount: u64,
	minimumcontribution: Balance,
	completed: bool,
}

pub trait Trait: balances::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_event!(
    pub enum Event<T>
    where
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash
    {
        Created(AccountId, Hash),
    }
);

decl_storage! {
	trait Store for Module<T: Trait> as KickstartModule {
		Campaigns get(campaign): map T::Hash => Campaign<T::Hash, T::Balance, T::AccountId>;
		CampaignOwner get (owner_of_campaign): map T::Hash => Option<T::AccountId>;
		OwnedCampaign get(campaign_of_owner): map T::AccountId => T::Hash;
		// CampaignApprovals get(no_approvals_campaign): map T::Hash => u64;
		// AllCampaignCount get(campaign_count): u64;
		Nonce: u64;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		fn deposit_event<T>() = default;

		fn create_campaign(origin, targetprice: T::Balance,minimumcontribution: T::Balance) -> Result {
			let sender = ensure_signed(origin)?;

			let nonce = <Nonce<T>>::get();
			let random_hash = (<system::Module<T>>::random_seed(), &sender, nonce)
				.using_encoded(<T as system::Trait>::Hashing::hash);

			ensure!(!<CampaignOwner<T>>::exists(random_hash), "Campaign already exists");

			let new_campaign = Campaign {
				id: random_hash,
				owner: sender.clone(),
				targetprice: targetprice,
				balance: <T::Balance as As<u64>>::sa(0),
				minimumcontribution: minimumcontribution,
				approvalcount: 0,
				completed: false,
			};

			<Campaigns<T>>::insert(random_hash,new_campaign);
			<CampaignOwner<T>>::insert(random_hash,&sender);
			<OwnedCampaign<T>>::insert(&sender,random_hash);

			<Nonce<T>>::mutate(|n| *n += 1);

			Self::deposit_event(RawEvent::Created(sender, random_hash));

			Ok(())
		}

		fn contribute_campaign(origin, contribution: T::Balance, contribute_to: T::Hash){
			let sender = ensure_signed(origin)?;

			ensure!(<Campaigns<T>>::exists(contribute_to), "This Campaign does not exist");
			let owner = Self::owner_of_campaign(contribute_to).ok_or("No owner for this Campaign")?;
			ensure!(owner != sender, "You cannot contribute to your own Campaign");
			
			let mut campaign = Self::campaign(contribute_to);
			let minimumcontribution = campaign.minimumcontribution;
			
			ensure!(contribution >= minimumcontribution, "You have to pay atleast the minimum amount");
			<balances::Module<T> as Currency<_>>::transfer(&sender, &owner, contribution)?;
			campaign.balance += contribution;
			<Campaigns<T>>::insert(campaign.id,campaign);

		}
	}
}

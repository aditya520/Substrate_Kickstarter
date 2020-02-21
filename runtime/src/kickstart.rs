use parity_codec::{Decode, Encode};
use rstd::prelude::*;
use runtime_primitives::traits::{As, Hash, Zero};
use support::{
	decl_event, decl_module, decl_storage,
	dispatch::Result,
	ensure,
	traits::{Currency, ReservableCurrency},
	StorageMap, StorageValue,
};
use system::ensure_signed;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Campaign<Hash, AccountId, Balance, BlockNumber> {
	campaign_id: Hash,
	campaign_manager: AccountId,
	campaign_name: Vec<u8>,
	campaign_target_money: Balance,
	campaign_expiry: BlockNumber,
	// status 
	// 0- Still raising money 
	// 1- Succeeded 
	// 2- Failed
	campaign_status: u64,
}

pub trait Trait: balances::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

const MAX_CAMPAIGNS_PER_BLOCK: usize = 3;

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		<T as system::Trait>::Hash,
		<T as balances::Trait>::Balance,
		<T as system::Trait>::BlockNumber
	{
		CreateCampaign(AccountId, Hash, Balance, Balance, BlockNumber),
		Invest(Hash, AccountId, Balance),
		CampaignFinalized(Hash, Balance, BlockNumber, bool),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as KickstartModule {
		Campaigns get(campaign): map T::Hash => Campaign<T::Hash, T::AccountId, T::Balance, T::BlockNumber>;
		CampaignOwner get(owner_of_campaign): map T::Hash => Option<T::AccountId>;
		// to be added in the genesis configuration
		CampaignPeriodLimit get(campaign_period_limit) config(): T::BlockNumber = T::BlockNumber::sa(864000);

		CampaignsByBlockNumber get(campaign_expire_at): map T::BlockNumber => Vec<T::Hash>;
		
		AllCampaignCount get(campaign_count): u64;
		AllCampaignArray get(campaign_by_index): map u64 => T::Hash;
		AllCampaignIndex: map T::Hash => u64;

		OwnedCampaignArray get(campaign_of_owner_by_index): map (T::AccountId, u64) => T::Hash;
		OwnedCampaignCount get(owned_campaign_count): map T::AccountId => u64;
		OwnedCampaignIndex: map (T::AccountId, T::Hash) => u64;

		InvestedCampaignsArray get(invested_campaign_by_index): map (T::AccountId, u64) => T::Hash;
		InvestedCampaignsCount get(invested_campaign_count): map T::AccountId => u64;
		InvestedCampaignsIndex: map (T::AccountId, T::Hash) => u64;

		InvestAmount get(invest_amount_of): map (T::Hash, T::AccountId) => T::Balance;
		InvestAccounts get(invest_accounts): map T::Hash => Vec<T::AccountId>;
		InvestAccountsCount get(invest_accounts_count): map T::Hash => u64;

		// The total amount of money the Campaign has got
		CampaignSupportedAmount get(total_amount_of_campaign): map T::Hash => T::Balance;

		CampaignStatus get(campaign_status): map T::Hash => u64;

		Nonce: u64;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		fn create_funding(origin, project_name: Vec<u8>, target_money: T::Balance, support_money: T::Balance, expiry: T::BlockNumber) -> Result {
			let sender = ensure_signed(origin)?;
			
			let nonce = <Nonce<T>>::get();
			let campaign_id = (<system::Module<T>>::random_seed(), &sender, nonce)
				.using_encoded(<T as system::Trait>::Hashing::hash);
			ensure!(!<CampaignOwner<T>>::exists(&campaign_id), "Campaign already exists");
			
			// ensure support_money <= target_money
			ensure!(support_money <= target_money, "You already have enough money");
			
			let new_campaign = Campaign{
				campaign_id: campaign_id.clone(),
				campaign_manager: sender.clone(),
				campaign_name: project_name,
				campaign_target_money: target_money,
				campaign_expiry: expiry,
				campaign_status: 0,				//Still raising money
			};

			// ensuring validation of the expiry
			ensure!(expiry > <system::Module<T>>::block_number(), "The expiry has to be greater than the current block number");
			ensure!(expiry <= <system::Module<T>>::block_number() + Self::campaign_period_limit(), "The expiry has be lower than the limit block number");

			// ensuring maximum number of campaign in a block
			let campaigns = Self::campaign_expire_at(expiry);
			ensure!(campaigns.len() < MAX_CAMPAIGNS_PER_BLOCK, "Maximum number of campaigns is reached for the target block, move to next block");

			Self::mint(sender.clone(), campaign_id.clone(), expiry.clone(), support_money.clone(), new_campaign)?;

			// deposit the event
			Self::deposit_event(RawEvent::CreateCampaign(sender, campaign_id, target_money, support_money, expiry));
			Ok(())
		}
	}
}
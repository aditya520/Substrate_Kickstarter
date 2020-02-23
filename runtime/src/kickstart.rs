use parity_codec::{Decode, Encode};
use rstd::prelude::*;
use runtime_primitives::traits::{As, Hash};
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

		/// invest a project
		fn invest(origin, campaign_id: T::Hash, invest_amount: T::Balance) -> Result {
			let sender = ensure_signed(origin)?;

			let owner = Self::owner_of_campaign(campaign_id).ok_or("Campaign has no owner")?;
			ensure!(owner != sender, "You can't invest for your own project");

			// The investor had not invested the project before
			if !<InvestAmount<T>>::exists((campaign_id.clone(), sender.clone())){
				Self::not_invest_before(sender.clone(), campaign_id.clone(), invest_amount.clone())?;
			}else{
				Self::invest_before(sender.clone(), campaign_id.clone(), invest_amount.clone())?;
			}

			Self::deposit_event(RawEvent::Invest(campaign_id, sender, invest_amount));

			Ok(())
		}

		fn on_finalize() {
		// get all the Campaign present in the block
			let block_number = <system::Module<T>>::block_number();
			let campaign_hash = Self::campaign_expire_at(block_number);

			for campaign_id in &campaign_hash{
				let mut campaign = Self::campaign(campaign_id);
				let amount_of_investment = Self::total_amount_of_campaign(campaign_id);
				if amount_of_investment >= campaign.campaign_target_money{
					// Make the status success
					campaign.campaign_status = 1;
					<Campaigns<T>>::insert(campaign_id.clone(), campaign);
					// Get the owner of the funding
					let _owner = Self::owner_of_campaign(campaign_id);
					match _owner {
						Some(owner) => {
							// Get all the investors
							let investors = Self::invest_accounts(campaign_id);
							let mut no_error = true;
							// Iterate every investor, unreserve the money that he/she had invested and transfer it to owner
							'inner: for investor in &investors{
								let invest_balance = Self::invest_amount_of((*campaign_id, investor.clone()));
								let _ = <balances::Module<T>>::unreserve(&investor, invest_balance.clone());
								// If the investor is owner, just unreserve the money
								if investor == &owner{ continue;}
								let _currency_transfer = <balances::Module<T> as Currency<_>>::transfer(&investor, &owner, invest_balance);
								match _currency_transfer {
									Err(_e) => {
										no_error = false;
										break 'inner;
									},
									Ok(_v) => {}
								}
							}
							if no_error {
								let _ = <balances::Module<T>>::reserve(&owner, amount_of_investment);
								// deposit the event
								Self::deposit_event(RawEvent::CampaignFinalized(*campaign_id, amount_of_investment, block_number, true));
							}
						},
						None => continue,
					}
				}else{ // refund all of the money
					// Make the status fail
					campaign.campaign_status = 2;
					<Campaigns<T>>::insert(campaign_id.clone(), campaign);
					let campaign_accounts = Self::invest_accounts(campaign_id);
					for account in campaign_accounts {
						let invest_balance = Self::invest_amount_of((*campaign_id, account.clone()));
						let _ = <balances::Module<T>>::unreserve(&account, invest_balance);
					}
					// deposit the event
					Self::deposit_event(RawEvent::CampaignFinalized(*campaign_id, amount_of_investment, block_number, false));
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	fn mint(
		sender: T::AccountId,
		campaign_id: T::Hash,
		expiry: T::BlockNumber,
		support_money: T::Balance,
		new_campaign: Campaign<T::Hash, T::AccountId, T::Balance, T::BlockNumber>,
	) -> Result {
		// updating the global states
		<Campaigns<T>>::insert(campaign_id.clone(), new_campaign.clone());
		<CampaignOwner<T>>::insert(campaign_id.clone(), sender.clone());

		<CampaignsByBlockNumber<T>>::mutate(expiry, |campaigns| campaigns.push(campaign_id.clone()));
		//Verify first Execute Last
		let campaign_count = Self::campaign_count();
		let new_campaign_count = campaign_count
			.checked_add(1)
			.ok_or("Overflow adding a new Campaign")?;

		<AllCampaignArray<T>>::insert(&campaign_count, campaign_id.clone());
		<AllCampaignCount<T>>::put(new_campaign_count);
		<AllCampaignIndex<T>>::insert(campaign_id.clone(), campaign_count);

		let owned_campaign_count = Self::owned_campaign_count(&sender);
		let new_owned_campaign_count = owned_campaign_count
			.checked_add(1)
			.ok_or("Overflow adding a new Campaign")?;

		<OwnedCampaignArray<T>>::insert(
			(sender.clone(), owned_campaign_count.clone()),
			campaign_id.clone(),
		);
		<OwnedCampaignCount<T>>::insert(&sender, new_owned_campaign_count);
		<OwnedCampaignIndex<T>>::insert((sender.clone(), campaign_id.clone()), owned_campaign_count);

		if support_money > T::Balance::sa(0) {
			Self::not_invest_before(sender.clone(), campaign_id.clone(), support_money.clone())?;
		}
		// add the nonce
		<Nonce<T>>::mutate(|n| *n += 1);

		Ok(())
	}

	//The investor had invested the project before
	fn invest_before(
		sender: T::AccountId,
		campaign_id: T::Hash,
		invest_amount: T::Balance,
	) -> Result {
		ensure!(<Campaigns<T>>::exists(campaign_id),"The Campaign exist does not exist");
		// ensure the investor has enough money
		ensure!(
			<balances::Module<T>>::free_balance(sender.clone()) >= invest_amount,
			"You don't have enough free balance to invest on this campaign"
		);

		let campaign = Self::campaign(&campaign_id);
		ensure!(
			<system::Module<T>>::block_number() < campaign.campaign_expiry,
			"This Campaign expired."
		);

		// reserve the amount of money
		<balances::Module<T>>::reserve(&sender, invest_amount)?;

		let amount_of_investor_on_campaign =
			Self::invest_amount_of((campaign_id.clone(), sender.clone()));
		let new_amount_of_investor_on_campaign =
			amount_of_investor_on_campaign + invest_amount.clone();

		<InvestAmount<T>>::insert(
			(campaign_id, sender),
			new_amount_of_investor_on_campaign.clone(),
		);

		// get the total amount of the project and add invest_amount
		let amount_of_campaign = Self::total_amount_of_campaign(&campaign_id);
		let new_amount_of_campaign = amount_of_campaign + invest_amount;

		// change the total amount of the project has collected
		<CampaignSupportedAmount<T>>::insert(&campaign_id, new_amount_of_campaign);

		Ok(())
	}

	// The investor doesn't invest the project before
	fn not_invest_before(
		sender: T::AccountId,
		campaign_id: T::Hash,
		invest_amount: T::Balance,
	) -> Result {
		ensure!(<Campaigns<T>>::exists(campaign_id),"The campaign does not exist");
		// ensure that the investor has enough money
		ensure!(
			<balances::Module<T>>::free_balance(sender.clone()) >= invest_amount,
			"You don't have enough free balance for investing for this campaign"
		);

		// get the number of projects that the investor had invested and add it
		let invested_campaign_count = Self::invested_campaign_count(&sender);
		let new_invested_campaign_count = invested_campaign_count
			.checked_add(1)
			.ok_or("Overflow adding a new invested Campaign")?;

		let campaign = Self::campaign(&campaign_id);
		ensure!(<system::Module<T>>::block_number() < campaign.campaign_expiry,"This campaign is expired.");

		// reserve the amount of money
		<balances::Module<T>>::reserve(&sender, invest_amount)?;

		<InvestAmount<T>>::insert((campaign_id.clone(), sender.clone()), invest_amount.clone());
		<InvestAccounts<T>>::mutate(&campaign_id, |accounts| accounts.push(sender.clone()));

		// add total support count
		let investor_count = <InvestAccountsCount<T>>::get(&campaign_id);
		let new_investor_count = investor_count
			.checked_add(1)
			.ok_or("Overflow adding the total number of investors of a campaign")?;
		<InvestAccountsCount<T>>::insert(campaign_id.clone(), new_investor_count);

		// change the state of invest related fields
		<InvestedCampaignsArray<T>>::insert(
			(sender.clone(), invested_campaign_count),
			campaign_id.clone(),
		);
		<InvestedCampaignsCount<T>>::insert(&sender, new_invested_campaign_count);
		<InvestedCampaignsIndex<T>>::insert(
			(sender.clone(), campaign_id.clone()),
			invested_campaign_count,
		);

		// get the total amount of the project and add invest_amount
		let amount_of_campaign = Self::total_amount_of_campaign(&campaign_id);
		let new_amount_of_campaign = amount_of_campaign + invest_amount;


		<CampaignSupportedAmount<T>>::insert(&campaign_id, new_amount_of_campaign);

		Ok(())
	}
}

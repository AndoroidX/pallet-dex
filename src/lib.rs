#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::traits::StaticLookup;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod test;

pub use pallet::*;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::dispatch::HasCompact;
    use frame_support::{pallet_prelude::*, Blake2_128Concat};
    use frame_system::pallet_prelude::*;
    use pallet_multi_token::multi_token::MultiTokenTrait;
    use sp_runtime::traits::{AtLeast32BitUnsigned, Zero};
    use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, IntegerSquareRoot};

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        // A number-like type which is used to store balances, shares of the pool and fees
        // Assumed to be the same one as the pallet_multi_token uses
        type Balance: Member
            + Parameter
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + MaxEncodedLen
            + TypeInfo;

        // A number-like type which is used to store id of an asset
        // Assumed to be the same one as the pallet_multi_token uses
        type AssetId: Member
            + Parameter
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + HasCompact
            + MaybeSerializeDeserialize
            + MaxEncodedLen
            + TypeInfo
            + Zero;

        // A share that is assigned to a pool creator
        #[pallet::constant]
        type DefaultShare: Get<Self::Balance>;

        // A constant of hundred percent mark (for fees)
        #[pallet::constant]
        type HundredPercent: Get<Self::Balance>;

        // A constant of hundred percent minus a fee mark
        #[pallet::constant]
        type HundredPercentMinusFee: Get<Self::Balance>;

        type MultiToken: MultiTokenTrait<Self, Self::AssetId, Self::Balance>;
    }

    #[pallet::storage]
    #[pallet::getter(fn get_pool)]
    pub type Pools<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,                         // Pool address
        (T::AssetId, T::AssetId, T::Balance), // Pair of assets in the pool & Pool constant
    >;

    #[pallet::storage]
    #[pallet::getter(fn get_pool_share)]
    pub type PoolShares<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // Pool address
        Blake2_128Concat,
        T::AccountId, // Liquidity provider address
        T::Balance,   // Share in the pool
    >;

    #[pallet::storage]
    #[pallet::getter(fn get_total_pool_shares)]
    pub type TotalPoolShares<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance>;

    #[pallet::event]
    #[pallet::generate_deposit(pub fn deposit_event)]
    pub enum Event<T: Config> {
        PoolCreated {
            creator: T::AccountId,
            pool_account: T::AccountId,
            first_asset: T::AssetId,
            second_asset: T::AssetId,
        },
        Swapped {
            operator: T::AccountId,
            pool_account: T::AccountId,
            first_asset: T::AssetId,
            first_asset_amount: T::Balance,
            second_asset: T::AssetId,
            second_asset_amount: T::Balance,
        },
        Deposited {
            operator: T::AccountId,
            pool_account: T::AccountId,
            first_asset: T::AssetId,
            first_asset_amount: T::Balance,
            second_asset: T::AssetId,
            second_asset_amount: T::Balance,
        },
        Withdrawed {
            operator: T::AccountId,
            pool_account: T::AccountId,
            first_asset: T::AssetId,
            first_asset_amount: T::Balance,
            second_asset: T::AssetId,
            second_asset_amount: T::Balance,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        // An arithmetic overflow
        Overflow,
        // Depositing 0 amount for init, swap or deposit functions
        DepositingZeroAmount,
        // Trying to withdraw 0 amount from the pool
        WithdrawingZeroAmount,
        // Trying to initialize pool with an address that already exists
        PoolAlreadyExists,
        // Trying to interact with a pool that does not exist
        NoSuchPool,
        // There is not enough balance to perform operation
        NotEnoughBalance,
        // Trying to deposit/withdraw wrong asset into the pool
        NoSuchTokenInPool,
        // The pool is dead, no assets in the pool
        EmptyPool,
        // Initialization of the pool with both assets being same
        SameAssetPool,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(1000)]
        pub fn init(
            origin: OriginFor<T>,
            pool_address: AccountIdLookupOf<T>,
            first_token_id: T::AssetId,
            first_token_amount: T::Balance,
            second_token_id: T::AssetId,
            second_token_amount: T::Balance,
        ) -> DispatchResult {
            let creator = ensure_signed(origin)?;
            let pool = T::Lookup::lookup(pool_address)?;

            ensure!(
                !first_token_amount.is_zero() && !second_token_amount.is_zero(),
                Error::<T>::DepositingZeroAmount
            );
            ensure!(Self::get_pool(&pool) == None, Error::<T>::PoolAlreadyExists);
            ensure!(first_token_id != second_token_id, Error::<T>::SameAssetPool);
            Self::check_balance(&first_token_id, &creator, first_token_amount)?;
            Self::check_balance(&second_token_id, &creator, second_token_amount)?;

            let pool_constant = first_token_amount
                .checked_mul(&second_token_amount)
                .ok_or(Error::<T>::Overflow)?;

            T::MultiToken::safe_transfer(
                creator.clone(),
                creator.clone(),
                pool.clone(),
                first_token_id,
                first_token_amount,
            )?;

            T::MultiToken::safe_transfer(
                creator.clone(),
                creator.clone(),
                pool.clone(),
                second_token_id,
                second_token_amount,
            )?;

            Pools::<T>::insert(&pool, (first_token_id, second_token_id, pool_constant));
            PoolShares::<T>::insert(&pool, &creator, T::DefaultShare::get());
            TotalPoolShares::<T>::insert(&pool, T::DefaultShare::get());

            Ok(())
        }

        #[pallet::weight(1000)]
        pub fn swap_token(
            origin: OriginFor<T>,
            pool_address: AccountIdLookupOf<T>,
            token_id: T::AssetId,
            amount: T::Balance,
        ) -> DispatchResult {
            let operator = ensure_signed(origin)?;
            let pool = T::Lookup::lookup(pool_address)?;

            Self::swap(operator, pool, token_id, amount)?;
            Ok(())
        }

        #[pallet::weight(1000)]
        pub fn deposit(
            origin: OriginFor<T>,
            pool_address: AccountIdLookupOf<T>,
            token_id: T::AssetId,
            amount: T::Balance,
        ) -> DispatchResult {
            let operator = ensure_signed(origin)?;
            let pool = T::Lookup::lookup(pool_address)?;

            Self::dep(operator, pool, token_id, amount)
        }

        #[pallet::weight(1000)]
        pub fn withdraw(
            origin: OriginFor<T>,
            pool_address: AccountIdLookupOf<T>,
            token_id: T::AssetId,
            amount: T::Balance,
        ) -> DispatchResult {
            let operator = ensure_signed(origin)?;
            let pool = T::Lookup::lookup(pool_address)?;

            Self::with(operator, pool, token_id, amount)?;
            Ok(())
        }

        #[pallet::weight(1000)]
        pub fn deposit_one_asset(
            origin: OriginFor<T>,
            pool_address: AccountIdLookupOf<T>,
            token_id: T::AssetId,
            amount: T::Balance,
        ) -> DispatchResult {
            let operator = ensure_signed(origin)?;
            let pool = T::Lookup::lookup(pool_address)?;

            ensure!(!amount.is_zero(), Error::<T>::DepositingZeroAmount);
            ensure!(Self::get_pool(&pool) != None, Error::<T>::NoSuchPool);
            Self::check_balance(&token_id, &operator, amount)?;

            // We have already checked that pool exists, unwrap is safe
            let (first_asset_id, second_asset_id, _) = Self::get_pool(&pool).unwrap();
            let corresponding_token_id = if token_id == first_asset_id {
                second_asset_id
            } else if token_id == second_asset_id {
                first_asset_id
            } else {
                ensure!(false, Error::<T>::NoSuchTokenInPool);
                first_asset_id
            };

            // x
            let pool_origin_token_balance =
                T::MultiToken::get_balance(&token_id, &pool).ok_or(Error::<T>::EmptyPool)?;
            // y
            let pool_dest_token_balance =
                T::MultiToken::get_balance(&corresponding_token_id, &pool)
                    .ok_or(Error::<T>::EmptyPool)?;

            ensure!(
                !pool_origin_token_balance.is_zero() && !pool_dest_token_balance.is_zero(),
                Error::<T>::EmptyPool
            );

            // Calculation of the amount that needs to be swapped
            let partial_result = pool_origin_token_balance
                .checked_mul(&pool_origin_token_balance)
                .ok_or(Error::<T>::Overflow)?
                .checked_add(
                    &pool_origin_token_balance
                        .checked_mul(&amount)
                        .ok_or(Error::<T>::Overflow)?,
                )
                .ok_or(Error::<T>::Overflow)?;
            let sqrt = partial_result.integer_sqrt();
            let to_swap_amount = sqrt
                .checked_sub(&pool_origin_token_balance)
                .ok_or(Error::<T>::Overflow)?;

            let received_after_swap =
                Self::swap(operator.clone(), pool.clone(), token_id, to_swap_amount)?;

            Self::dep(operator, pool, corresponding_token_id, received_after_swap)?;

            Ok(())
        }

        #[pallet::weight(1000)]
        pub fn withdraw_one_asset(
            origin: OriginFor<T>,
            pool_address: AccountIdLookupOf<T>,
            token_id: T::AssetId,
            amount: T::Balance,
        ) -> DispatchResult {
            let operator = ensure_signed(origin)?;
            let pool = T::Lookup::lookup(pool_address)?;

            ensure!(!amount.is_zero(), Error::<T>::DepositingZeroAmount);
            ensure!(Self::get_pool(&pool) != None, Error::<T>::NoSuchPool);

            // We have already checked that pool exists, unwrap is safe
            let (first_asset_id, second_asset_id, _) = Self::get_pool(&pool).unwrap();
            let corresponding_token_id = if token_id == first_asset_id {
                second_asset_id
            } else if token_id == second_asset_id {
                first_asset_id
            } else {
                ensure!(false, Error::<T>::NoSuchTokenInPool);
                first_asset_id
            };

            // x
            let pool_origin_token_balance =
                T::MultiToken::get_balance(&token_id, &pool).ok_or(Error::<T>::EmptyPool)?;
            // y
            let pool_dest_token_balance =
                T::MultiToken::get_balance(&corresponding_token_id, &pool)
                    .ok_or(Error::<T>::EmptyPool)?;

            ensure!(
                !pool_origin_token_balance.is_zero() && !pool_dest_token_balance.is_zero(),
                Error::<T>::EmptyPool
            );
            ensure!(pool_origin_token_balance >= amount, Error::<T>::Overflow);

            let partial_result = pool_origin_token_balance
                .checked_mul(&pool_origin_token_balance)
                .ok_or(Error::<T>::Overflow)?
                .checked_sub(
                    &pool_origin_token_balance
                        .checked_mul(&amount)
                        .ok_or(Error::<T>::Overflow)?,
                )
                .ok_or(Error::<T>::Overflow)?;
            let sqrt = partial_result.integer_sqrt();
            let to_withdraw_amount = pool_origin_token_balance
                .checked_sub(&sqrt)
                .ok_or(Error::<T>::Overflow)?;

            let withdrawn_of_corresponding_token =
                Self::with(operator.clone(), pool.clone(), token_id, to_withdraw_amount)?;

            Self::swap(
                operator,
                pool,
                corresponding_token_id,
                withdrawn_of_corresponding_token,
            )?;

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        // Checks if there is enoguh tokens on users balance
        fn check_balance(
            id: &T::AssetId,
            account: &T::AccountId,
            needed_balace: T::Balance,
        ) -> Result<(), Error<T>> {
            match T::MultiToken::get_balance(id, account) {
                Some(balance) => {
                    ensure!(needed_balace <= balance, Error::<T>::NotEnoughBalance);
                }
                None => {
                    return Err(Error::<T>::NotEnoughBalance);
                }
            }
            Ok(())
        }

        fn swap(
            operator: T::AccountId,
            pool: T::AccountId,
            token_id: T::AssetId,
            amount: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(!amount.is_zero(), Error::<T>::DepositingZeroAmount);
            ensure!(Self::get_pool(&pool) != None, Error::<T>::NoSuchPool);
            Self::check_balance(&token_id, &operator, amount)?;

            // We have already checked that pool exists, unwrap is safe
            let (first_asset_id, second_asset_id, constant) = Self::get_pool(&pool).unwrap();
            let corresponding_token_id = if token_id == first_asset_id {
                second_asset_id
            } else if token_id == second_asset_id {
                first_asset_id
            } else {
                ensure!(false, Error::<T>::NoSuchTokenInPool);
                first_asset_id
            };

            let pool_origin_token_balance =
                T::MultiToken::get_balance(&token_id, &pool).ok_or(Error::<T>::EmptyPool)?;
            let pool_dest_token_balance =
                T::MultiToken::get_balance(&corresponding_token_id, &pool)
                    .ok_or(Error::<T>::EmptyPool)?;

            ensure!(
                !pool_origin_token_balance.is_zero() && !pool_dest_token_balance.is_zero(),
                Error::<T>::EmptyPool
            );

            let partial_calculation = constant
                .checked_div(
                    &pool_origin_token_balance
                        .checked_add(&amount)
                        .ok_or(Error::<T>::Overflow)?,
                )
                .ok_or(Error::<T>::Overflow)?;
            let swap_token_result = pool_dest_token_balance
                .checked_sub(&partial_calculation)
                .ok_or(Error::<T>::Overflow)?
                .checked_mul(&T::HundredPercentMinusFee::get())
                .ok_or(Error::<T>::Overflow)?
                .checked_div(&T::HundredPercent::get())
                .ok_or(Error::<T>::Overflow)?;

            T::MultiToken::safe_transfer(
                operator.clone(),
                operator.clone(),
                pool.clone(),
                token_id,
                amount,
            )?;

            T::MultiToken::safe_transfer(
                pool.clone(),
                pool.clone(),
                operator.clone(),
                corresponding_token_id,
                swap_token_result,
            )?;

            // Since we took the fee, we need to alter the pool constant
            Self::fetch_pool_constant(&pool, &token_id, &corresponding_token_id)?;

            Self::deposit_event(Event::<T>::Swapped {
                operator,
                pool_account: pool,
                first_asset: first_asset_id,
                first_asset_amount: amount,
                second_asset: corresponding_token_id,
                second_asset_amount: swap_token_result,
            });

            Ok(swap_token_result)
        }

        fn dep(
            operator: T::AccountId,
            pool: T::AccountId,
            token_id: T::AssetId,
            amount: T::Balance,
        ) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::DepositingZeroAmount);
            ensure!(Self::get_pool(&pool) != None, Error::<T>::NoSuchPool);
            Self::check_balance(&token_id, &operator, amount)?;

            let (first_asset_id, second_asset_id, _) = Self::get_pool(&pool).unwrap();
            let corresponding_token_id = if token_id == first_asset_id {
                second_asset_id
            } else if token_id == second_asset_id {
                first_asset_id
            } else {
                ensure!(false, Error::<T>::NoSuchTokenInPool);
                first_asset_id
            };

            let pool_origin_token_balance =
                T::MultiToken::get_balance(&token_id, &pool).ok_or(Error::<T>::EmptyPool)?;
            let pool_dest_token_balance =
                T::MultiToken::get_balance(&corresponding_token_id, &pool)
                    .ok_or(Error::<T>::EmptyPool)?;
            ensure!(
                !pool_origin_token_balance.is_zero() && !pool_dest_token_balance.is_zero(),
                Error::<T>::EmptyPool
            );

            let corresponding_token_amount = amount
                .checked_mul(&pool_dest_token_balance)
                .ok_or(Error::<T>::Overflow)?
                .checked_div(&pool_origin_token_balance)
                .ok_or(Error::<T>::Overflow)?;
            Self::check_balance(
                &corresponding_token_id,
                &operator,
                corresponding_token_amount,
            )?;

            let current_full_share =
                TotalPoolShares::<T>::get(&pool).ok_or(Error::<T>::NoSuchPool)?;
            ensure!(!current_full_share.is_zero(), Error::<T>::NoSuchPool);
            let operator_pool_share = match PoolShares::<T>::get(&pool, &operator) {
                Some(share) => share,
                None => Zero::zero(),
            };
            let add_operator_pool_share = amount
                .checked_mul(&current_full_share)
                .ok_or(Error::<T>::Overflow)?
                .checked_div(&pool_origin_token_balance)
                .ok_or(Error::<T>::Overflow)?;
            let new_full_share = current_full_share
                .checked_add(&add_operator_pool_share)
                .ok_or(Error::<T>::Overflow)?;
            let new_operator_pool_share = operator_pool_share
                .checked_add(&add_operator_pool_share)
                .ok_or(Error::<T>::Overflow)?;

            T::MultiToken::safe_transfer(
                operator.clone(),
                operator.clone(),
                pool.clone(),
                token_id,
                amount,
            )?;

            T::MultiToken::safe_transfer(
                operator.clone(),
                operator.clone(),
                pool.clone(),
                corresponding_token_id,
                corresponding_token_amount,
            )?;

            Self::fetch_pool_constant(&pool, &token_id, &corresponding_token_id)?;

            TotalPoolShares::<T>::set(&pool, Some(new_full_share));
            PoolShares::<T>::set(&pool, &operator, Some(new_operator_pool_share));

            Self::deposit_event(Event::<T>::Deposited {
                operator,
                pool_account: pool,
                first_asset: token_id,
                first_asset_amount: amount,
                second_asset: corresponding_token_id,
                second_asset_amount: corresponding_token_amount,
            });

            Ok(())
        }

        // Returns the amount of corresponding tokens that has been withdrawn
        fn with(
            operator: T::AccountId,
            pool: T::AccountId,
            token_id: T::AssetId,
            amount: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(!amount.is_zero(), Error::<T>::WithdrawingZeroAmount);
            ensure!(Self::get_pool(&pool) != None, Error::<T>::NoSuchPool);

            let (first_asset_id, second_asset_id, _) = Self::get_pool(&pool).unwrap();
            let corresponding_token_id = if token_id == first_asset_id {
                second_asset_id
            } else if token_id == second_asset_id {
                first_asset_id
            } else {
                ensure!(false, Error::<T>::NoSuchTokenInPool);
                first_asset_id
            };

            let pool_origin_token_balance =
                T::MultiToken::get_balance(&token_id, &pool).ok_or(Error::<T>::EmptyPool)?;
            let pool_dest_token_balance =
                T::MultiToken::get_balance(&corresponding_token_id, &pool)
                    .ok_or(Error::<T>::EmptyPool)?;
            ensure!(
                !pool_origin_token_balance.is_zero() && !pool_dest_token_balance.is_zero(),
                Error::<T>::EmptyPool
            );

            let corresponding_token_amount = amount
                .checked_mul(&pool_dest_token_balance)
                .ok_or(Error::<T>::Overflow)?
                .checked_div(&pool_origin_token_balance)
                .ok_or(Error::<T>::Overflow)?;

            let current_full_share =
                TotalPoolShares::<T>::get(&pool).ok_or(Error::<T>::NoSuchPool)?;
            ensure!(!current_full_share.is_zero(), Error::<T>::NoSuchPool);
            let operator_pool_share = match PoolShares::<T>::get(&pool, &operator) {
                Some(share) => share,
                None => Zero::zero(),
            };
            let sub_operator_pool_share = amount
                .checked_mul(&current_full_share)
                .ok_or(Error::<T>::Overflow)?
                .checked_div(&pool_origin_token_balance)
                .ok_or(Error::<T>::Overflow)?;
            let new_full_share = current_full_share
                .checked_sub(&sub_operator_pool_share)
                .ok_or(Error::<T>::Overflow)?;
            let new_operator_pool_share = operator_pool_share
                .checked_sub(&sub_operator_pool_share)
                .ok_or(Error::<T>::Overflow)?;

            TotalPoolShares::<T>::set(&pool, Some(new_full_share));
            PoolShares::<T>::set(&pool, &operator, Some(new_operator_pool_share));

            T::MultiToken::safe_transfer(
                pool.clone(),
                pool.clone(),
                operator.clone(),
                token_id,
                amount,
            )?;

            T::MultiToken::safe_transfer(
                pool.clone(),
                pool.clone(),
                operator.clone(),
                corresponding_token_id,
                corresponding_token_amount,
            )?;

            Self::fetch_pool_constant(&pool, &token_id, &corresponding_token_id)?;

            Self::deposit_event(Event::<T>::Withdrawed {
                operator,
                pool_account: pool,
                first_asset: token_id,
                first_asset_amount: amount,
                second_asset: corresponding_token_id,
                second_asset_amount: corresponding_token_amount,
            });

            Ok(corresponding_token_amount)
        }

        fn fetch_pool_constant(
            pool: &T::AccountId,
            token_id: &T::AssetId,
            corresponding_token_id: &T::AssetId,
        ) -> DispatchResult {
            let (first_asset_id, second_asset_id, _) =
                Self::get_pool(pool).ok_or(Error::<T>::NoSuchPool)?;
            let pool_origin_token_balance =
                T::MultiToken::get_balance(token_id, pool).ok_or(Error::<T>::EmptyPool)?;
            let pool_dest_token_balance = T::MultiToken::get_balance(corresponding_token_id, pool)
                .ok_or(Error::<T>::EmptyPool)?;
            let new_constant = pool_origin_token_balance
                .checked_mul(&pool_dest_token_balance)
                .ok_or(Error::<T>::Overflow)?;
            Pools::<T>::set(pool, Some((first_asset_id, second_asset_id, new_constant)));
            Ok(())
        }
    }
}

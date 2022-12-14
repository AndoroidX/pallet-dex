use crate::{mock::*, Error};

use frame_benchmarking::frame_support::assert_noop;
use frame_support::assert_ok;
use pallet_multi_token::multi_token::MultiTokenTrait;

#[test]
fn init_pool() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));

        assert_eq!(Dex::get_pool(314159265), Some((0, 1, 2500)));
        assert_eq!(Dex::get_pool_share(314159265, 1), Some(10000));
    });
}

#[test]
fn swap_tokens() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100));
        assert_ok!(MultiTokenPallet::transfer(Origin::signed(1), 1, 2, 0, 10));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_ok!(Dex::swap_token(Origin::signed(2), 314159265, 0, 10));

        assert_eq!(MultiTokenPallet::get_balance(&0, &2), Some(0));
        // One token was used as slippage, another one as fee
        assert_eq!(MultiTokenPallet::get_balance(&1, &2), Some(8));
    });
}

#[test]
fn depositing_liquidity() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100));
        assert_ok!(MultiTokenPallet::transfer(Origin::signed(1), 1, 2, 0, 10));
        assert_ok!(MultiTokenPallet::transfer(Origin::signed(1), 1, 2, 1, 10));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_ok!(Dex::deposit(Origin::signed(2), 314159265, 0, 10));

        assert_eq!(MultiTokenPallet::get_balance(&0, &2), Some(0));
        assert_eq!(MultiTokenPallet::get_balance(&1, &2), Some(0));
        assert_eq!(Dex::get_pool_share(314159265, 2), Some(2000));
        assert_eq!(Dex::get_total_pool_shares(314159265), Some(12000));
    });
}

#[test]
fn withdrawing_liquidity() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_ok!(Dex::withdraw(Origin::signed(1), 314159265, 0, 10));

        assert_eq!(MultiTokenPallet::get_balance(&0, &1), Some(60));
        assert_eq!(MultiTokenPallet::get_balance(&1, &1), Some(60));
        assert_eq!(Dex::get_pool_share(314159265, 1), Some(8000));
        assert_eq!(Dex::get_total_pool_shares(314159265), Some(8000));

        assert_ok!(Dex::withdraw(Origin::signed(1), 314159265, 0, 40));

        assert_eq!(MultiTokenPallet::get_balance(&0, &1), Some(100));
        assert_eq!(MultiTokenPallet::get_balance(&1, &1), Some(100));
        assert_eq!(Dex::get_pool_share(314159265, 1), Some(0));
        assert_eq!(Dex::get_total_pool_shares(314159265), Some(0));
    });
}

#[test]
#[should_panic]
fn init_pool_with_same_assets() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 0, 50));
    });
}

#[test]
fn abuse_without_tokens() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 11000));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 10100));
        assert_ok!(MultiTokenPallet::transfer(
            Origin::signed(1),
            1,
            2,
            0,
            10000
        ));
        assert_ok!(MultiTokenPallet::transfer(
            Origin::signed(1),
            1,
            2,
            1,
            10000
        ));
        assert_noop!(
            Dex::init(Origin::signed(1), 314159265, 0, 500, 1, 500),
            Error::<Test>::NotEnoughBalance
        );
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_noop!(
            Dex::swap_token(Origin::signed(1), 314159265, 1, 500),
            Error::<Test>::NotEnoughBalance
        );
        assert_noop!(
            Dex::deposit(Origin::signed(1), 314159265, 1, 500),
            Error::<Test>::NotEnoughBalance
        );
        assert_ok!(Dex::deposit(Origin::signed(2), 314159265, 0, 10000));
        assert_noop!(
            Dex::withdraw(Origin::signed(1), 314159265, 1, 500),
            Error::<Test>::Overflow
        );
        assert_noop!(
            Dex::withdraw(Origin::signed(1), 314159265, 1, 51),
            Error::<Test>::Overflow
        );
        assert_ok!(Dex::withdraw(Origin::signed(1), 314159265, 1, 50));
    });
}

#[test]
fn using_uninitialized_pool() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Dex::swap_token(Origin::signed(2), 314159265, 0, 10),
            Error::<Test>::NoSuchPool
        );
    });
}

#[test]
fn zero_amounts() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100));
        assert_noop!(
            Dex::init(Origin::signed(1), 314159265, 0, 0, 1, 50),
            Error::<Test>::DepositingZeroAmount
        );
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_noop!(
            Dex::swap_token(Origin::signed(1), 314159265, 1, 0),
            Error::<Test>::DepositingZeroAmount
        );
        assert_noop!(
            Dex::deposit(Origin::signed(1), 314159265, 1, 0),
            Error::<Test>::DepositingZeroAmount
        );
        assert_noop!(
            Dex::withdraw(Origin::signed(1), 314159265, 1, 0),
            Error::<Test>::WithdrawingZeroAmount
        );
    });
}

#[test]
fn creating_existing_pool() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_noop!(
            Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50),
            Error::<Test>::PoolAlreadyExists
        );
    });
}

#[test]
fn depositing_token_that_is_not_in_pool() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 2, 100));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_noop!(
            Dex::swap_token(Origin::signed(1), 314159265, 2, 50),
            Error::<Test>::NoSuchTokenInPool
        );
        assert_noop!(
            Dex::deposit(Origin::signed(1), 314159265, 2, 50),
            Error::<Test>::NoSuchTokenInPool
        );
        assert_noop!(
            Dex::withdraw(Origin::signed(1), 314159265, 2, 50),
            Error::<Test>::NoSuchTokenInPool
        );
    });
}

#[test]
fn depositing_assets_into_dead_pool() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_ok!(Dex::withdraw(Origin::signed(1), 314159265, 0, 50));
        assert_noop!(
            Dex::deposit(Origin::signed(1), 314159265, 0, 50),
            Error::<Test>::EmptyPool
        );
    });
}

#[test]
fn withdrawing_more_liquidity_than_in_the_pool() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 1000));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 1000));
        assert_ok!(MultiTokenPallet::transfer(Origin::signed(1), 1, 2, 0, 900));
        assert_ok!(MultiTokenPallet::transfer(Origin::signed(1), 1, 2, 1, 900));
        assert_ok!(Dex::init(Origin::signed(1), 314159265, 0, 50, 1, 50));
        assert_ok!(Dex::deposit(Origin::signed(2), 314159265, 0, 900));
        assert_noop!(
            Dex::withdraw(Origin::signed(1), 314159265, 0, 500),
            Error::<Test>::Overflow
        );
    });
}

#[test]
fn deposit_one_asset() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100000000));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100000000));
        assert_ok!(MultiTokenPallet::transfer(
            Origin::signed(1),
            1,
            2,
            0,
            10000000
        ));
        assert_ok!(MultiTokenPallet::transfer(
            Origin::signed(1),
            1,
            2,
            1,
            10000000
        ));
        assert_ok!(Dex::init(
            Origin::signed(1),
            314159265,
            0,
            50000,
            1,
            50000000
        ));
        assert_ok!(Dex::deposit_one_asset(
            Origin::signed(2),
            314159265,
            0,
            10000000
        ));

        // Note, even though the balance should be 0, it is not because there is a swap fee
        // This amount would become negligible as swap fee aproaches 0
        assert_eq!(MultiTokenPallet::get_balance(&0, &2), Some(382199));
        assert_eq!(MultiTokenPallet::get_balance(&1, &2), Some(10000000));
        println!("{}", MultiTokenPallet::get_balance(&0, &314159265).unwrap());
        println!("{}", MultiTokenPallet::get_balance(&1, &314159265).unwrap());
    });
}

#[test]
fn withdrawing_one_asset() {
    new_test_ext().execute_with(|| {
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 0, 100000000));
        assert_ok!(MultiTokenPallet::create(Origin::signed(1)));
        assert_ok!(MultiTokenPallet::mint(Origin::signed(1), 1, 100000000));
        assert_ok!(MultiTokenPallet::transfer(
            Origin::signed(1),
            1,
            2,
            0,
            10000000
        ));
        assert_ok!(MultiTokenPallet::transfer(
            Origin::signed(1),
            1,
            2,
            1,
            10000000
        ));
        assert_ok!(Dex::init(
            Origin::signed(1),
            314159265,
            0,
            50000000,
            1,
            50000000
        ));
        assert_ok!(Dex::deposit(Origin::signed(2), 314159265, 0, 10000000));
        assert_ok!(Dex::withdraw_one_asset(
            Origin::signed(2),
            314159265,
            0,
            1000000
        ));

        println!("{}", MultiTokenPallet::get_balance(&0, &2).unwrap());
        println!("{}", MultiTokenPallet::get_balance(&1, &2).unwrap());

        // Note, even though we made a request of withdrawal for 1000000, we receive 0.15% less
        // This happens because of the swap fee
        // As swap fee aproaches 0, the balance would aproach requested amonut
        assert_eq!(MultiTokenPallet::get_balance(&0, &2), Some(998507));
        assert_eq!(MultiTokenPallet::get_balance(&1, &2), Some(0));
    });
}

// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    account::{self, Account},
    executor::FakeExecutor,
    gas_costs::TXN_RESERVED,
    keygen::KeyGen,
    transaction_status_eq,
};
use libra_types::{
    account_config,
    transaction::TransactionStatus,
    vm_status::{AbortLocation, StatusCode, VMStatus},
};
use transaction_builder::*;

#[test]
fn tiered_mint_designated_dealer() {
    let mut executor = FakeExecutor::from_genesis_file();
    let blessed = Account::new_blessed_tc();
    let mut keygen = KeyGen::from_seed([9u8; 32]);
    let (_, pubkey) = keygen.generate_keypair();

    // account to represent designated dealer
    let dd = Account::new();
    executor.execute_and_apply(blessed.signed_script_txn(
        encode_create_designated_dealer_script(
            account_config::coin1_tag(),
            0,
            *dd.address(),
            dd.auth_key_prefix(),
            vec![],
            vec![],
            pubkey.to_bytes().to_vec(),
            false, // add_all_currencies
        ),
        0,
    ));
    let mint_amount_one = 1_000;
    let tier_index = 0;
    executor.execute_and_apply(blessed.signed_script_txn(
        encode_tiered_mint_script(
            account_config::coin1_tag(),
            1,
            *dd.address(),
            mint_amount_one,
            tier_index,
        ),
        1,
    ));
    let dd_post_mint = executor
        .read_account_resource(&dd)
        .expect("receiver must exist");
    let dd_balance = executor
        .read_balance_resource(&dd, account::coin1_currency_code())
        .expect("receiver balance must exist");
    assert_eq!(mint_amount_one, dd_balance.coin());
    assert_eq!(0, dd_post_mint.sequence_number());

    // --------------
    let mint_amount_two = 5_000_000;
    let tier_index = 3;
    executor.execute_and_apply(blessed.signed_script_txn(
        encode_tiered_mint_script(
            account_config::coin1_tag(),
            2,
            *dd.address(),
            mint_amount_two,
            tier_index,
        ),
        2,
    ));
    let dd_balance = executor
        .read_balance_resource(&dd, account::coin1_currency_code())
        .expect("receiver balance must exist");
    assert_eq!(mint_amount_one + mint_amount_two, dd_balance.coin());

    // -------------- invalid tier index
    let tier_index = 4;
    let output = &executor.execute_transaction(blessed.signed_script_txn(
        encode_tiered_mint_script(
            account_config::coin1_tag(),
            3,
            *dd.address(),
            mint_amount_one,
            tier_index,
        ),
        3,
    ));
    // TODO(tmn) provide a real abort location
    assert!(transaction_status_eq(
        &output.status(),
        &TransactionStatus::Keep(VMStatus::MoveAbort(AbortLocation::Script, 3)),
    ));
}

#[test]
fn mint_to_existing_not_dd() {
    // create a FakeExecutor with a genesis from file
    // We can't run mint test on terraform genesis as we don't have the private key to sign the
    // mint transaction.
    let mut executor = FakeExecutor::from_genesis_file();
    let tc = Account::new_blessed_tc();
    let libra_root = Account::new_libra_root();

    // create and publish a sender with 1_000_000 coins
    let receiver = Account::new();

    executor.execute_and_apply(libra_root.signed_script_txn(
        encode_create_testing_account_script(
            account_config::coin1_tag(),
            *receiver.address(),
            receiver.auth_key_prefix(),
            false,
        ),
        1,
    ));

    let mint_amount = 1_000;
    let output = executor.execute_transaction(tc.signed_script_txn(
        encode_tiered_mint_script(
            account_config::coin1_tag(),
            0,
            *receiver.address(),
            mint_amount,
            4,
        ),
        0,
    ));
    assert_eq!(
        output.status().vm_status().status_code(),
        StatusCode::ABORTED
    );
    assert_eq!(output.status().vm_status().move_abort_code(), Some(5));
}

#[test]
fn mint_to_new_account() {
    // create a FakeExecutor with a genesis from file
    // We can't run mint test on terraform genesis as we don't have the private key to sign the
    // mint transaction.

    let executor = FakeExecutor::from_genesis_file();
    let tc = Account::new_blessed_tc();

    // create and publish a sender with TXN_RESERVED coins
    let new_account = Account::new();

    let mint_amount = TXN_RESERVED;
    let output = executor.execute_transaction(tc.signed_script_txn(
        encode_tiered_mint_script(
            account_config::coin1_tag(),
            0,
            *new_account.address(),
            mint_amount,
            4,
        ),
        0,
    ));

    assert_eq!(
        output.status().vm_status().status_code(),
        StatusCode::ABORTED
    );
    assert_eq!(output.status().vm_status().move_abort_code(), Some(5));
}

#[test]
fn tiered_update_exchange_rate() {
    let mut executor = FakeExecutor::from_genesis_file();
    let blessed = Account::new_blessed_tc();

    // set coin1 rate to 1.23 COIN1
    executor.execute_and_apply(blessed.signed_script_txn(
        encode_update_exchange_rate_script(account_config::coin1_tag(), 0, 123, 100),
        0,
    ));
    let post_update = executor
        .read_account_resource(&blessed)
        .expect("blessed executed txn");
    assert_eq!(1, post_update.sequence_number());
}

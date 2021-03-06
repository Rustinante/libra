// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ledger_store::{EpochEndingLedgerInfoIter, LedgerStore},
    state_store::StateStore,
    transaction_store::TransactionStore,
};
use anyhow::{ensure, Result};
use itertools::zip_eq;
use jellyfish_merkle::iterator::JellyfishMerkleIterator;
use libra_crypto::hash::HashValue;
use libra_types::{
    account_state_blob::AccountStateBlob,
    ledger_info::LedgerInfoWithSignatures,
    proof::{SparseMerkleRangeProof, TransactionAccumulatorRangeProof, TransactionInfoWithProof},
    transaction::{Transaction, TransactionInfo, Version},
};
use std::sync::Arc;

/// `BackupHandler` provides functionalities for LibraDB data backup.
#[derive(Clone)]
pub struct BackupHandler {
    ledger_store: Arc<LedgerStore>,
    transaction_store: Arc<TransactionStore>,
    state_store: Arc<StateStore>,
}

impl BackupHandler {
    pub(crate) fn new(
        ledger_store: Arc<LedgerStore>,
        transaction_store: Arc<TransactionStore>,
        state_store: Arc<StateStore>,
    ) -> Self {
        Self {
            ledger_store,
            transaction_store,
            state_store,
        }
    }

    /// Gets an iterator that yields a range of transactions.
    pub fn get_transaction_iter<'a>(
        &'a self,
        start_version: Version,
        num_transactions: usize,
    ) -> Result<impl Iterator<Item = Result<(Transaction, TransactionInfo)>> + 'a> {
        let txn_iter = self
            .transaction_store
            .get_transaction_iter(start_version, num_transactions)?;
        let txn_info_iter = self
            .ledger_store
            .get_transaction_info_iter(start_version, num_transactions)?;
        let zipped = zip_eq(txn_iter, txn_info_iter)
            .map(|(txn_res, txn_info_res)| Ok((txn_res?, txn_info_res?)));
        Ok(zipped)
    }

    /// Gets the proof for a transaction chunk.
    /// N.B. the `LedgerInfo` returned will always be in the same epoch of the `last_version`.
    pub fn get_transaction_range_proof(
        &self,
        first_version: Version,
        last_version: Version,
    ) -> Result<(TransactionAccumulatorRangeProof, LedgerInfoWithSignatures)> {
        ensure!(
            last_version >= first_version,
            "Bad transaction range: [{}, {}]",
            first_version,
            last_version
        );
        let num_transactions = last_version - first_version + 1;
        let epoch = self.ledger_store.get_epoch(last_version)?;
        let ledger_info = self.ledger_store.get_latest_ledger_info_in_epoch(epoch)?;
        let accumulator_proof = self.ledger_store.get_transaction_range_proof(
            Some(first_version),
            num_transactions,
            ledger_info.ledger_info().version(),
        )?;
        Ok((accumulator_proof, ledger_info))
    }

    /// Gets an iterator which can yield all accounts in the state tree.
    pub fn get_account_iter(
        &self,
        version: Version,
    ) -> Result<Box<dyn Iterator<Item = Result<(HashValue, AccountStateBlob)>> + Send + Sync>> {
        let iterator = JellyfishMerkleIterator::new(
            Arc::clone(&self.state_store),
            version,
            HashValue::zero(),
        )?;
        Ok(Box::new(iterator))
    }

    /// Gets the proof that proves a range of accounts.
    pub fn get_account_state_range_proof(
        &self,
        rightmost_key: HashValue,
        version: Version,
    ) -> Result<SparseMerkleRangeProof> {
        self.state_store
            .get_account_state_range_proof(rightmost_key, version)
    }

    /// Gets the latest version and state root hash.
    pub fn get_latest_state_root(&self) -> Result<(Version, HashValue)> {
        let (version, txn_info) = self.ledger_store.get_latest_transaction_info()?;
        Ok((version, txn_info.state_root_hash()))
    }

    /// Gets the proof of the state root at specified version.
    /// N.B. the `LedgerInfo` returned will always be in the same epoch of the version.
    pub fn get_state_root_proof(
        &self,
        version: Version,
    ) -> Result<(TransactionInfoWithProof, LedgerInfoWithSignatures)> {
        let epoch = self.ledger_store.get_epoch(version)?;
        let ledger_info = self.ledger_store.get_latest_ledger_info_in_epoch(epoch)?;
        let txn_info = self
            .ledger_store
            .get_transaction_info_with_proof(version, ledger_info.ledger_info().version())?;

        Ok((txn_info, ledger_info))
    }

    pub fn get_epoch_ending_ledger_info_iter(
        &self,
        start_epoch: u64,
        end_epoch: u64,
    ) -> Result<EpochEndingLedgerInfoIter> {
        self.ledger_store
            .get_epoch_ending_ledger_info_iter(start_epoch, end_epoch)
    }
}

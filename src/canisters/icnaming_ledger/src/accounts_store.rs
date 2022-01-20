use crate::metrics_encoder::MetricsEncoder;
use crate::state::StableState;
use crate::STATE;
use candid::CandidType;
use dfn_candid::Candid;
use ic_base_types::{CanisterId, PrincipalId};
use ic_nns_constants::GOVERNANCE_CANISTER_ID;
use itertools::Itertools;
use ledger_canister::{
    AccountIdentifier, BlockHeight, ICPTs, Memo, Subaccount, TimeStamp,
    Transfer::{self, Burn, Mint, Send},
};
use on_wire::{FromWire, IntoWire};
use serde::Deserialize;
use std::cmp::{min, Ordering};
use std::collections::{HashMap, VecDeque};
use std::ops::RangeTo;
use std::time::{Duration, SystemTime};

type TransactionIndex = u64;

#[derive(Default)]
pub struct AccountsStore {
    transactions: VecDeque<Transaction>,
    block_height_synced_up_to: Option<BlockHeight>,

    last_ledger_sync_timestamp_nanos: u64,
}

#[derive(CandidType, Deserialize)]
struct Transaction {
    transaction_index: TransactionIndex,
    block_height: BlockHeight,
    timestamp: TimeStamp,
    memo: Memo,
    transfer: Transfer,
    transaction_type: Option<TransactionType>,
}

#[derive(Copy, Clone, CandidType, Deserialize, Debug, Eq, PartialEq)]
enum TransactionType {
    Send,
}

impl AccountsStore {
    pub fn append_transaction(
        &mut self,
        transfer: Transfer,
        memo: Memo,
        block_height: BlockHeight,
        timestamp: TimeStamp,
    ) -> Result<bool, String> {
        if let Some(block_height_synced_up_to) = self.get_block_height_synced_up_to() {
            let expected_block_height = block_height_synced_up_to + 1;
            if block_height != block_height_synced_up_to + 1 {
                return Err(format!(
                    "Expected block height {}. Got block height {}",
                    expected_block_height, block_height
                ));
            }
        }

        let transaction_index = self.get_next_transaction_index();
        let mut should_store_transaction = false;
        let mut transaction_type: Option<TransactionType> = None;

        todo!();
        // match transfer {
        //     Burn { from, amount: _ } => {
        //         if self.try_add_transaction_to_account(from, transaction_index) {
        //             should_store_transaction = true;
        //             transaction_type = Some(TransactionType::Burn);
        //         }
        //     }
        //     Mint { to, amount: _ } => {
        //         if self.try_add_transaction_to_account(to, transaction_index) {
        //             should_store_transaction = true;
        //             transaction_type = Some(TransactionType::Mint);
        //         }
        //     }
        //     Send {
        //         from,
        //         to,
        //         amount,
        //         fee: _,
        //     } => {
        //         if self.try_add_transaction_to_account(to, transaction_index) {
        //             self.try_add_transaction_to_account(from, transaction_index);
        //             should_store_transaction = true;
        //             transaction_type = Some(TransactionType::Send);
        //         } else if self.try_add_transaction_to_account(from, transaction_index) {
        //             should_store_transaction = true;
        //             if let Some(principal) = self.try_get_principal(&from) {
        //                 let canister_ids: Vec<CanisterId> = self
        //                     .get_canisters(principal)
        //                     .iter()
        //                     .map(|c| c.canister_id)
        //                     .collect();
        //                 transaction_type = Some(self.get_transaction_type(
        //                     from,
        //                     to,
        //                     amount,
        //                     memo,
        //                     &principal,
        //                     &canister_ids,
        //                 ));
        //                 self.process_transaction_type(
        //                     transaction_type.unwrap(),
        //                     principal,
        //                     from,
        //                     to,
        //                     memo,
        //                     amount,
        //                     block_height,
        //                 );
        //             }
        //         } else if let Some(neuron_details) = self.neuron_accounts.get(&to) {
        //             // Handle the case where people top up their neuron from an external account
        //             self.multi_part_transactions_processor.push(
        //                 neuron_details.principal,
        //                 block_height,
        //                 MultiPartTransactionToBeProcessed::TopUpNeuron(
        //                     neuron_details.principal,
        //                     neuron_details.memo,
        //                 ),
        //             );
        //         }
        //     }
        // }

        if should_store_transaction {
            self.transactions.push_back(Transaction::new(
                transaction_index,
                block_height,
                timestamp,
                memo,
                transfer,
                transaction_type,
            ));
        }

        self.block_height_synced_up_to = Some(block_height);

        Ok(should_store_transaction)
    }

    pub fn mark_ledger_sync_complete(&mut self) {
        self.last_ledger_sync_timestamp_nanos = dfn_core::api::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
    }

    pub fn init_block_height_synced_up_to(&mut self, block_height: BlockHeight) {
        if self.block_height_synced_up_to.is_some() {
            panic!("This can only be called to initialize the 'block_height_synced_up_to' value");
        }

        self.block_height_synced_up_to = Some(block_height);
    }

    pub fn get_next_transaction_index(&self) -> TransactionIndex {
        match self.transactions.back() {
            Some(t) => t.transaction_index + 1,
            None => 0,
        }
    }

    pub fn get_block_height_synced_up_to(&self) -> Option<BlockHeight> {
        self.block_height_synced_up_to
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_transactions_count(&self) -> u32 {
        self.transactions.len() as u32
    }

    pub fn get_stats(&self) -> Stats {
        let earliest_transaction = self.transactions.front();
        let latest_transaction = self.transactions.back();
        let timestamp_now_nanos = dfn_core::api::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let duration_since_last_sync =
            Duration::from_nanos(timestamp_now_nanos - self.last_ledger_sync_timestamp_nanos);

        Stats {
            transactions_count: self.transactions.len() as u64,
            block_height_synced_up_to: self.block_height_synced_up_to,
            earliest_transaction_timestamp_nanos: earliest_transaction
                .map_or(0, |t| t.timestamp.timestamp_nanos),
            earliest_transaction_block_height: earliest_transaction.map_or(0, |t| t.block_height),
            latest_transaction_timestamp_nanos: latest_transaction
                .map_or(0, |t| t.timestamp.timestamp_nanos),
            latest_transaction_block_height: latest_transaction.map_or(0, |t| t.block_height),
            seconds_since_last_ledger_sync: duration_since_last_sync.as_secs(),
        }
    }

    fn get_transaction(&self, transaction_index: TransactionIndex) -> Option<&Transaction> {
        match self.transactions.front() {
            Some(t) => {
                if t.transaction_index > transaction_index {
                    None
                } else {
                    let offset = t.transaction_index;
                    self.transactions.get((transaction_index - offset) as usize)
                }
            }
            None => None,
        }
    }

    fn get_transaction_mut(
        &mut self,
        transaction_index: TransactionIndex,
    ) -> Option<&mut Transaction> {
        match self.transactions.front() {
            Some(t) => {
                if t.transaction_index > transaction_index {
                    None
                } else {
                    let offset = t.transaction_index;
                    self.transactions
                        .get_mut((transaction_index - offset) as usize)
                }
            }
            None => None,
        }
    }

    fn get_transaction_index(&self, block_height: BlockHeight) -> Option<TransactionIndex> {
        // The binary search methods are taken from here (they will be in stable rust shortly) -
        // https://github.com/vojtechkral/rust/blob/c7a787a3276cadad7ee51577f65158b4888c058c/library/alloc/src/collections/vec_deque.rs#L2515
        fn binary_search_by_key<T, B, F>(
            vec_deque: &VecDeque<T>,
            b: &B,
            mut f: F,
        ) -> Result<usize, usize>
        where
            F: FnMut(&T) -> B,
            B: Ord,
        {
            binary_search_by(vec_deque, |k| f(k).cmp(b))
        }

        fn binary_search_by<T, F>(vec_deque: &VecDeque<T>, mut f: F) -> Result<usize, usize>
        where
            F: FnMut(&T) -> Ordering,
        {
            let (front, back) = vec_deque.as_slices();

            let search_back = matches!(
                back.first().map(|elem| f(elem)),
                Some(Ordering::Less) | Some(Ordering::Equal)
            );
            if search_back {
                back.binary_search_by(f)
                    .map(|idx| idx + front.len())
                    .map_err(|idx| idx + front.len())
            } else {
                front.binary_search_by(f)
            }
        }

        if let Some(latest_transaction) = self.transactions.back() {
            let max_block_height = latest_transaction.block_height;
            if block_height <= max_block_height {
                // binary_search_by_key is not yet in stable rust (https://github.com/rust-lang/rust/issues/78021)
                // TODO uncomment the line below once binary_search_by_key is in stable rust
                // self.transactions.binary_search_by_key(&block_height, |t| t.block_height).ok().map(|i| i as u64)
                return binary_search_by_key(&self.transactions, &block_height, |t| t.block_height)
                    .ok()
                    .map(|i| i as u64);
            }
        }
        None
    }
}

impl StableState for AccountsStore {
    fn encode(&self) -> Vec<u8> {
        Candid((
            &self.transactions,
            &self.block_height_synced_up_to,
            &self.last_ledger_sync_timestamp_nanos,
        ))
        .into_bytes()
        .unwrap()
    }

    fn decode(bytes: Vec<u8>) -> Result<Self, String> {
        #[allow(clippy::type_complexity)]
        let (transactions, block_height_synced_up_to, last_ledger_sync_timestamp_nanos): (
            VecDeque<Transaction>,
            Option<BlockHeight>,
            u64,
        ) = Candid::from_bytes(bytes).map(|c| c.0)?;

        Ok(AccountsStore {
            transactions,
            block_height_synced_up_to,
            last_ledger_sync_timestamp_nanos,
        })
    }
}

impl Transaction {
    pub fn new(
        transaction_index: TransactionIndex,
        block_height: BlockHeight,
        timestamp: TimeStamp,
        memo: Memo,
        transfer: Transfer,
        transaction_type: Option<TransactionType>,
    ) -> Transaction {
        Transaction {
            transaction_index,
            block_height,
            timestamp,
            memo,
            transfer,
            transaction_type,
        }
    }
}

/// This will sort the canisters such that those with names specified will appear first and will be
/// sorted by their names. Then those without names will appear last, sorted by their canister Ids.
fn sort_canisters(canisters: &mut Vec<NamedCanister>) {
    canisters.sort_unstable_by_key(|c| {
        if c.name.is_empty() {
            (true, c.canister_id.to_string())
        } else {
            (false, c.name.clone())
        }
    });
}

#[derive(CandidType)]
pub enum TransferResult {
    Burn {
        amount: ICPTs,
    },
    Mint {
        amount: ICPTs,
    },
    Send {
        to: AccountIdentifier,
        amount: ICPTs,
        fee: ICPTs,
    },
    Receive {
        from: AccountIdentifier,
        amount: ICPTs,
        fee: ICPTs,
    },
}

pub fn encode_metrics(w: &mut MetricsEncoder<Vec<u8>>) -> std::io::Result<()> {
    STATE.with(|s| {
        let stats = s.accounts_store.borrow().get_stats();
        w.encode_gauge(
            "transactions_count",
            stats.transactions_count as f64,
            "Number of transactions processed by the canister.",
        )?;
        w.encode_gauge(
            "seconds_since_last_ledger_sync",
            stats.seconds_since_last_ledger_sync as f64,
            "Number of seconds since last ledger sync.",
        )?;
        Ok(())
    })
}

#[derive(CandidType, Deserialize)]
pub struct Stats {
    transactions_count: u64,
    block_height_synced_up_to: Option<u64>,
    earliest_transaction_timestamp_nanos: u64,
    earliest_transaction_block_height: BlockHeight,
    latest_transaction_timestamp_nanos: u64,
    latest_transaction_block_height: BlockHeight,
    seconds_since_last_ledger_sync: u64,
}

#[cfg(test)]
mod tests;

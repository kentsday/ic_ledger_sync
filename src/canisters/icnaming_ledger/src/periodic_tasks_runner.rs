use crate::canisters::ledger;
use crate::ledger_sync;
use crate::state::STATE;
use dfn_core::api::{CanisterId, PrincipalId};
use ic_nns_constants::CYCLES_MINTING_CANISTER_ID;
use ledger_canister::{
    AccountBalanceArgs, AccountIdentifier, BlockHeight, CyclesResponse, ICPTs, Memo,
    NotifyCanisterArgs, SendArgs, Subaccount, TRANSACTION_FEE,
};

const PRUNE_TRANSACTIONS_COUNT: u32 = 1000;

pub async fn run_periodic_tasks() {
    ledger_sync::sync_transactions().await;

    if should_prune_transactions() {
        // TODO clean some data
    }
}

fn should_prune_transactions() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        const MEMORY_LIMIT_BYTES: u32 = 1024 * 1024 * 1024; // 1GB
        let memory_usage_bytes = (core::arch::wasm32::memory_size(0) * 65536) as u32;
        memory_usage_bytes > MEMORY_LIMIT_BYTES
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        const TRANSACTIONS_COUNT_LIMIT: u32 = 1_000_000;
        let transactions_count = STATE.with(|s| s.accounts_store.borrow().get_transactions_count());
        transactions_count > TRANSACTIONS_COUNT_LIMIT
    }
}

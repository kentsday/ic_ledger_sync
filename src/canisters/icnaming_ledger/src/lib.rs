use crate::accounts_store::Stats;
use crate::periodic_tasks_runner::run_periodic_tasks;
use crate::state::{StableState, State, STATE};
use candid::CandidType;
use dfn_candid::{candid, candid_one};
use dfn_core::{api::trap_with, over, stable};
use ledger_canister::{AccountIdentifier, BlockHeight};

mod accounts_store;
mod assets;
mod canisters;
mod ledger_sync;
mod metrics_encoder;
mod periodic_tasks_runner;
mod state;

#[export_name = "canister_init"]
fn main() {}

#[export_name = "canister_pre_upgrade"]
fn pre_upgrade() {
    STATE.with(|s| {
        let bytes = s.encode();
        stable::set(&bytes);
    });
}

#[export_name = "canister_post_upgrade"]
fn post_upgrade() {
    STATE.with(|s| {
        let bytes = stable::get();
        let new_state = State::decode(bytes).expect("Decoding stable memory failed");

        s.replace(new_state)
    });
}

#[export_name = "canister_query http_request"]
pub fn http_request() {
    over(candid_one, assets::http_request);
}

/// Returns stats about the canister.
///
/// These stats include things such as the number of accounts registered, the memory usage, the
/// number of neurons created, etc.
#[export_name = "canister_query get_stats"]
pub fn get_stats() {
    over(candid, |()| get_stats_impl());
}

fn get_stats_impl() -> Stats {
    STATE.with(|s| s.accounts_store.borrow().get_stats())
}

/// Executes on every block height and is used to run background processes.
///
/// These background processes include:
/// - Sync transactions from the ledger
/// - Process any queued 'multi-part' actions (eg. staking a neuron or topping up a canister)
/// - Prune old transactions if memory usage is too high
#[export_name = "canister_heartbeat"]
pub fn canister_heartbeat() {
    let future = run_periodic_tasks();

    dfn_core::api::futures::spawn(future);
}

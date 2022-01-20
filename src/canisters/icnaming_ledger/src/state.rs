use crate::accounts_store::AccountsStore;
use dfn_candid::Candid;
use on_wire::{FromWire, IntoWire};
use std::cell::RefCell;

#[derive(Default)]
pub struct State {
    // NOTE: When adding new persistent fields here, ensure that these fields
    // are being persisted in the `replace` method below.
    pub accounts_store: RefCell<AccountsStore>,
}

impl State {
    pub fn replace(&self, new_state: State) {
        self.accounts_store.replace(new_state.accounts_store.take());
    }
}

pub trait StableState: Sized {
    fn encode(&self) -> Vec<u8>;
    fn decode(bytes: Vec<u8>) -> Result<Self, String>;
}

thread_local! {
    pub static STATE: State = State::default();
}

impl StableState for State {
    fn encode(&self) -> Vec<u8> {
        Candid((self.accounts_store.borrow().encode(),))
            .into_bytes()
            .unwrap()
    }

    fn decode(bytes: Vec<u8>) -> Result<Self, String> {
        let (account_store_bytes,) = Candid::from_bytes(bytes).map(|c| c.0)?;

        Ok(State {
            accounts_store: RefCell::new(AccountsStore::decode(account_store_bytes)?),
        })
    }
}

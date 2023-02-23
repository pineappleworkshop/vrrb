use std::{collections::HashMap, env, fs};

use primitives::Address;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use vrrb_core::{account::Account, keypair::Keypair};
use vrrbdb::{VrrbDb, VrrbDbConfig};

mod common;
use common::generate_random_address;

use crate::common::generate_random_string;

#[test]
fn accounts_can_be_added() {
    let temp_dir_path = env::temp_dir();
    let state_backup_path = temp_dir_path.join(format!("{}", generate_random_string()));

    let mut db = VrrbDb::new(VrrbDbConfig {
        path: state_backup_path,
        state_store_path: None,
        transaction_store_path: None,
        event_store_path: None,
    });

    let addr1 = generate_random_address();
    let addr2 = generate_random_address();
    let addr3 = generate_random_address();
    let addr4 = generate_random_address();
    let addr5 = generate_random_address();

    db.insert_account(
        addr1,
        Account {
            hash: String::from(""),
            nonce: 0,
            credits: 0,
            debits: 0,
            storage: None,
            code: None,
            pubkey: vec![],
            digests: HashMap::new(),
        },
    )
    .unwrap();

    db.insert_account(
        addr2,
        Account {
            hash: String::from(""),
            nonce: 0,
            credits: 0,
            debits: 0,
            storage: None,
            code: None,
            pubkey: vec![],
            digests: HashMap::new(),
        },
    )
    .unwrap();

    let entries = db.state_store_factory().handle().entries();

    assert_eq!(entries.len(), 2);

    db.extend_accounts(vec![
        (
            addr3,
            Account {
                hash: String::from(""),
                nonce: 0,
                credits: 0,
                debits: 0,
                storage: None,
                code: None,
                pubkey: vec![],
                digests: HashMap::new(),
            },
        ),
        (
            addr4,
            Account {
                hash: String::from(""),
                nonce: 0,
                credits: 0,
                debits: 0,
                storage: None,
                code: None,
                pubkey: vec![],
                digests: HashMap::new(),
            },
        ),
        (
            addr5,
            Account {
                hash: String::from(""),
                nonce: 0,
                credits: 0,
                debits: 0,
                storage: None,
                code: None,
                pubkey: vec![],
                digests: HashMap::new(),
            },
        ),
    ]);

    let entries = db.state_store_factory().handle().entries();

    assert_eq!(entries.len(), 5);
}
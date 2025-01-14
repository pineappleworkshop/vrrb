/// Some basic utility functions (a decay calculator for the now deprecated
/// monetary policy and a restore db function that can take in a path
/// to a db file and restore a PickleDB)
//TODO: Replace PickleDB with LR DB
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};

pub fn decay_calculator(initial: u128, epochs: u128) -> f64 {
    let b: f64 = 1.0f64 / initial as f64;
    let ln_b = b.log10();
    (ln_b / epochs as f64) * -1.0
}

pub fn restore_db(path: &str) -> PickleDb {
    match PickleDb::load(
        path,
        PickleDbDumpPolicy::DumpUponRequest,
        SerializationMethod::Bin,
    ) {
        Ok(nst) => nst,
        Err(_) => PickleDb::new(
            path,
            PickleDbDumpPolicy::DumpUponRequest,
            SerializationMethod::Bin,
        ),
    }
}

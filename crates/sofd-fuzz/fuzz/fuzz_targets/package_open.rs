#![no_main]

//! The ZIP container reader may return `Ok` or `Err` for any input but must
//! never panic, and anything it accepts must be walkable entry by entry without
//! a second panic. This is the first thing a downloaded document reaches.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut package) = sofd_package::Package::from_bytes(data) else {
        return;
    };
    // Reading every entry it claims to hold must also stay panic-free: a
    // corrupted central directory could otherwise hand back a name whose data
    // runs past the buffer.
    let names: Vec<String> = package.entries().to_vec();
    for name in names {
        let _ = package.read(&name);
        let _ = package.read_string(&name);
    }
    let _ = package.entry_point();
    let _ = package.document_dirs();
});

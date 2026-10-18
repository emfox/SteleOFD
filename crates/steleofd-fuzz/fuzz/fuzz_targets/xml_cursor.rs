#![no_main]

//! `Cursor::next_event` must answer every byte sequence with `Ok` or `Err`, and
//! the walk must always terminate. The loop bound turns a hang into a visible
//! fuzz failure instead of a stuck process.

use libfuzzer_sys::fuzz_target;
use steleofd_xml::Cursor;

fuzz_target!(|data: &[u8]| {
    // Invalid UTF-8 is not the cursor's concern, so it is normalised the same
    // way the reader normalises an entry's bytes.
    let text = String::from_utf8_lossy(data);
    let mut cursor = Cursor::new(&text);
    for _ in 0..4096 {
        match cursor.next_event() {
            Ok(steleofd_xml::cursor::Event::Eof) => return,
            Ok(steleofd_xml::cursor::Event::Start(node)) => {
                let _ = node.attributes().get("ID");
                let _ = node.local();
            }
            Ok(_) => {}
            Err(_) => return,
        }
    }
    panic!("cursor did not reach EOF within 4096 events");
});

//! Write the golden book to a path given as the first argument, so the file a
//! third-party renderer is handed stays in step with the fixtures.
fn main() {
    let path = std::env::args().nth(1).expect("output path");
    std::fs::write(&path, sofd_testkit::golden_book()).expect("write the book");
}

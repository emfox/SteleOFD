//! The `Fonts` table is the only place that says which file a document font
//! id means. These tests pin that pairing down: the id a text run names must
//! reach the file the declaration names, whatever order the two lists come in.

use sofd_package::{Package, build_archive};
use sofd_parser::fonts::load_fonts;

/// Bytes that stand in for an encoded font. What they are matters less than
/// that the ones read back are the ones that were stored.
fn face_bytes(tag: u8) -> Vec<u8> {
    vec![0x00, 0x01, 0x00, 0x00, tag]
}

/// The smallest archive `load_fonts` reads: an entry point, a document and
/// one `Res` table naming the font files given.
fn archive(res: &str, files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let ofd = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0"><DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody></OFD>"#;
    let document = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document><CommonData><PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea></CommonData></Document>"#;
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("OFD.xml", ofd.as_bytes()),
        ("Doc_0/Document.xml", document.as_bytes()),
        ("Doc_0/PublicRes.xml", res.as_bytes()),
    ];
    for (name, bytes) in files {
        entries.push((name, bytes.as_slice()));
    }
    build_archive(&entries).expect("the archive builds")
}

fn table(bytes: &[u8]) -> sofd_parser::fonts::FontTable {
    let mut package = Package::from_bytes(bytes).expect("package");
    load_fonts(&mut package, "Doc_0").expect("the font table loads")
}

/// A `Res` table whose font ids descend while their file names ascend, so a
/// pairing that sorts one list and zips it against the other is caught.
fn two_font_table() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><Fonts>
  <Font ID="9" FontName="Nine"><FontFile>aaa-nine.ttf</FontFile></Font>
  <Font ID="7" FontName="Seven"><FontFile>zzz-seven.ttf</FontFile></Font>
</Fonts></Res>"#
        .to_string()
}

#[test]
fn a_declared_font_resolves_to_the_bytes_of_the_file_it_names() {
    let bytes = archive(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><Fonts>
  <Font ID="7" FontName="Orbitron"><FontFile>Orbitron-Bold.ttf</FontFile></Font>
</Fonts></Res>"#,
        &[("Doc_0/Res/Orbitron-Bold.ttf", face_bytes(9))],
    );
    let table = table(&bytes);
    let font = table.get(&7).expect("font 7 is in the table");
    assert_eq!(font.path, "Doc_0/Res/Orbitron-Bold.ttf");
    assert_eq!(font.family, "Orbitron");
    assert_eq!(
        &*font.data,
        &face_bytes(9),
        "the declared file's bytes travel intact"
    );
}

#[test]
fn each_id_keeps_the_file_its_own_declaration_names() {
    let bytes = archive(
        &two_font_table(),
        &[
            ("Doc_0/Res/aaa-nine.ttf", face_bytes(1)),
            ("Doc_0/Res/zzz-seven.ttf", face_bytes(2)),
        ],
    );
    let table = table(&bytes);
    assert_eq!(table.len(), 2, "both declarations load");
    let nine = table.get(&9).expect("font 9 is in the table");
    assert_eq!(nine.path, "Doc_0/Res/aaa-nine.ttf");
    assert_eq!(nine.family, "Nine");
    assert_eq!(&*nine.data, &face_bytes(1));
    let seven = table.get(&7).expect("font 7 is in the table");
    assert_eq!(seven.path, "Doc_0/Res/zzz-seven.ttf");
    assert_eq!(seven.family, "Seven");
    assert_eq!(&*seven.data, &face_bytes(2));
}

#[test]
fn a_font_that_names_no_file_is_kept_as_an_unshipped_declaration() {
    // Dropping the declaration is what made a named face render blank with no
    // warning: nothing downstream knew the document had asked for it. It is
    // kept, marked as not embedded, with the family a host search needs.
    let bytes = archive(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><Fonts>
  <Font ID="7" FontName="Orbitron" FamilyName="Orbitron"/>
</Fonts></Res>"#,
        &[("Doc_0/Res/Orbitron-Bold.ttf", face_bytes(9))],
    );
    let table = table(&bytes);
    let seven = table.get(&7).expect("the declaration is kept");
    assert!(!seven.embedded, "and says it ships no file");
    assert!(seven.data.is_empty(), "so there are no bytes to hand over");
    assert_eq!(seven.family, "Orbitron");
}

#[test]
fn a_declared_file_the_package_does_not_hold_is_kept_unshipped() {
    let bytes = archive(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><Fonts>
  <Font ID="7" FontName="Orbitron" FamilyName="Orbitron"><FontFile>missing.ttf</FontFile></Font>
</Fonts></Res>"#,
        &[("Doc_0/Res/Orbitron-Bold.ttf", face_bytes(9))],
    );
    let table = table(&bytes);
    let seven = table.get(&7).expect("the declaration is kept");
    assert!(!seven.embedded, "a dangling file name resolves to nothing");
    assert!(seven.data.is_empty());
}

#[test]
fn the_base_loc_of_a_table_decides_the_directory_to_look_in() {
    let bytes = archive(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Fonts"><Fonts>
  <Font ID="7" FontName="Orbitron"><FontFile>Orbitron-Bold.ttf</FontFile></Font>
</Fonts></Res>"#,
        &[("Doc_0/Fonts/Orbitron-Bold.ttf", face_bytes(9))],
    );
    let table = table(&bytes);
    let font = table.get(&7).expect("font 7 is in the table");
    assert_eq!(font.path, "Doc_0/Fonts/Orbitron-Bold.ttf");
}

#[test]
fn the_document_table_and_the_public_table_both_contribute_their_fonts() {
    let ofd = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0"><DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody></OFD>"#;
    let document = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document><CommonData><PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea></CommonData></Document>"#;
    let document_res = r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><Fonts>
  <Font ID="3" FontName="Three"><FontFile>three.ttf</FontFile></Font>
</Fonts></Res>"#;
    let public_res = r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><Fonts>
  <Font ID="7" FontName="Seven"><FontFile>seven.ttf</FontFile></Font>
</Fonts></Res>"#;
    let bytes = build_archive(&[
        ("OFD.xml", ofd.as_bytes()),
        ("Doc_0/Document.xml", document.as_bytes()),
        ("Doc_0/DocumentRes.xml", document_res.as_bytes()),
        ("Doc_0/PublicRes.xml", public_res.as_bytes()),
        ("Doc_0/Res/three.ttf", &face_bytes(3)),
        ("Doc_0/Res/seven.ttf", &face_bytes(7)),
    ])
    .expect("the archive builds");
    let table = table(&bytes);
    assert_eq!(table.len(), 2, "both tables are read");
    assert_eq!(table.get(&3).expect("font 3").family, "Three");
    assert_eq!(table.get(&7).expect("font 7").family, "Seven");
}

#[test]
fn a_font_without_an_id_cannot_be_named_and_is_skipped() {
    let bytes = archive(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><Fonts>
  <Font FontName="Nameless"><FontFile>aaa.ttf</FontFile></Font>
  <Font ID="7" FontName="Seven"><FontFile>zzz-seven.ttf</FontFile></Font>
</Fonts></Res>"#,
        &[
            ("Doc_0/Res/aaa.ttf", face_bytes(1)),
            ("Doc_0/Res/zzz-seven.ttf", face_bytes(2)),
        ],
    );
    let table = table(&bytes);
    assert_eq!(table.len(), 1, "the nameless entry is skipped whole");
    assert_eq!(table.get(&7).expect("font 7").family, "Seven");
}

#[test]
fn a_document_with_no_res_table_loads_no_fonts() {
    let ofd = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0"><DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody></OFD>"#;
    let document = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document><CommonData><PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea></CommonData></Document>"#;
    let bytes = build_archive(&[
        ("OFD.xml", ofd.as_bytes()),
        ("Doc_0/Document.xml", document.as_bytes()),
    ])
    .expect("the archive builds");
    let table = table(&bytes);
    assert!(table.is_empty(), "an absent table is skipped, not an error");
}

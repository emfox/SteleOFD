//! The archival rewrite, from GB/T 42133-2022 clause 6.2.2. Every case here
//! takes a container apart, rewrites it and checks what came back.

use steleofd_archive::{archive, audit};
use steleofd_package::Package;

/// A one page document that still carries every optional structure clause
/// 6.2.2 wants gone, so a case can watch them come out.
fn carrying_optional() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "OFD.xml",
            r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>urn:test:1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Signatures>Doc_0/Signs/Signatures.xml</ofd:Signatures>
  </ofd:DocBody>
</ofd:OFD>"#,
        ),
        (
            "Doc_0/Document.xml",
            r#"<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>1</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
    <ofd:DocumentRes>Res/DocumentRes.xml</ofd:DocumentRes>
    <ofd:VPreferences><ofd:PageMode>None</ofd:PageMode></ofd:VPreferences>
    <ofd:Permissions><ofd:Edit>false</ofd:Edit></ofd:Permissions>
  </ofd:CommonData>
  <ofd:Pages>
    <ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>
  </ofd:Pages>
  <ofd:Attachments><ofd:Attachment ID="1" Name="note.txt" FileLoc="Attachments/note.txt"/></ofd:Attachments>
</ofd:Document>"#,
        ),
        (
            "Doc_0/Signs/Signatures.xml",
            r#"<ofd:Signatures xmlns:ofd="http://www.ofdspec.org/2016"/>"#,
        ),
        (
            "Doc_0/Res/DocumentRes.xml",
            r#"<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res"/>"#,
        ),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            r#"<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content>
    <ofd:Layer ID="1"><ofd:PathObject ID="2" Boundary="0 0 10 10"/></ofd:Layer>
  </ofd:Content>
</ofd:Page>"#,
        ),
        ("Doc_0/Attachments/note.txt", "a note"),
    ]
}

/// Rewrites a container and returns the result bytes plus the change record.
fn rewrite(entries: &[(&str, &str)]) -> (Vec<u8>, steleofd_archive::Archived) {
    let owned: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, body)| (*name, body.as_bytes()))
        .collect();
    archive(&owned).expect("rewrite")
}

/// Opens a container and reads one entry.
fn read_entry(bytes: &[u8], path: &str) -> String {
    let mut package = Package::from_bytes(bytes).expect("open");
    package.read_string(path).expect("entry")
}

#[test]
fn gb_t_42133_6_2_2_optional_structures_are_stripped() {
    let (bytes, change) = rewrite(&carrying_optional());
    let document = read_entry(&bytes, "Doc_0/Document.xml");
    assert!(
        !document.contains("VPreferences"),
        "view preferences must be gone, got {document}"
    );
    assert!(
        !document.contains("Permissions"),
        "permissions must be gone, got {document}"
    );
    assert!(
        !document.contains("Attachments"),
        "the attachment list must be gone, got {document}"
    );
    assert_eq!(change.stripped_elements, 3);
    assert!(
        change
            .stripped_documents
            .contains(&"Doc_0/Document.xml".to_string())
    );
}

#[test]
fn gb_t_42133_6_2_2_attachment_files_leave_with_their_list() {
    let (bytes, change) = rewrite(&carrying_optional());
    let package = Package::from_bytes(&bytes).expect("open");
    assert!(
        !package.contains("Doc_0/Attachments/note.txt"),
        "an attachment file with no list entry is unreachable and must go"
    );
    assert!(
        change
            .removed_entries
            .contains(&"Doc_0/Attachments/note.txt".to_string()),
        "the removal is reported, got {:?}",
        change.removed_entries
    );
}

#[test]
fn gb_t_42133_6_2_2_the_content_around_the_optional_elements_survives() {
    let (bytes, _change) = rewrite(&carrying_optional());
    let root = read_entry(&bytes, "OFD.xml");
    assert!(root.contains("DocID"), "the durable id must stay");
    let document = read_entry(&bytes, "Doc_0/Document.xml");
    assert!(document.contains("Content.xml"), "pages stay");
    assert!(document.contains("DocumentRes"), "resources stay");
}

#[test]
fn gb_t_42133_6_2_1_a_the_result_declares_the_archival_profile() {
    let (bytes, _change) = rewrite(&carrying_optional());
    let root = read_entry(&bytes, "OFD.xml");
    assert!(
        root.contains("DocType=\"OFD-A\""),
        "the rewrite declares the profile, got {root}"
    );
}

#[test]
fn the_rewritten_container_passes_the_archival_audit() {
    let (bytes, _change) = rewrite(&carrying_optional());
    let mut package = Package::from_bytes(&bytes).expect("open");
    let report = audit(&mut package).expect("audit");
    let errors: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.severity == steleofd_archive::Severity::Error)
        .map(|f| f.code)
        .collect();
    assert!(
        errors.is_empty(),
        "the rewrite must not leave errors, got {errors:?}"
    );
}

#[test]
fn a_clean_container_is_rewritten_only_where_the_profile_asks() {
    // A container that already carries nothing optional still gains its
    // profile declaration and keeps every other byte.
    let entries = vec![
        (
            "OFD.xml",
            r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0"><ofd:DocBody><ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot></ofd:DocBody></ofd:OFD>"#,
        ),
        ("Doc_0/Document.xml", "<ofd:Document/>"),
    ];
    let (bytes, change) = rewrite(&entries);
    assert_eq!(change.stripped_elements, 0, "nothing optional to strip");
    assert!(change.removed_entries.is_empty(), "no entry to remove");
    assert!(
        !change.is_noop(),
        "the profile declaration is still a change"
    );
    let root = read_entry(&bytes, "OFD.xml");
    assert!(
        root.contains("DocType=\"OFD-A\""),
        "the profile is declared"
    );
    assert_eq!(
        read_entry(&bytes, "Doc_0/Document.xml"),
        "<ofd:Document/>",
        "a non-optional document is untouched"
    );
}

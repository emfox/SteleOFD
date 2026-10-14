//! End-to-end archival audits over real containers.

use steleofd_archive::{Severity, audit};
use steleofd_package::{Package, build_archive};

/// Audits one container and returns the report.
fn audit_of(entries: &[(&str, &str)]) -> steleofd_archive::Report {
    let owned: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, body)| (*name, body.as_bytes()))
        .collect();
    let bytes = build_archive(&owned).expect("build container");
    let mut package = Package::from_bytes(&bytes).expect("open container");
    audit(&mut package).expect("audit")
}

/// Codes present in a report, sorted for stable assertions.
fn codes(report: &steleofd_archive::Report) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = report.findings.iter().map(|f| f.code).collect();
    found.sort_unstable();
    found
}

/// A minimal one page document that is fully self-contained.
fn healthy() -> Vec<(&'static str, &'static str)> {
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
  </ofd:CommonData>
  <ofd:Pages>
    <ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>
  </ofd:Pages>
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
  <ofd:PageRes>PageRes.xml</ofd:PageRes>
</ofd:Page>"#,
        ),
        (
            "Doc_0/Pages/Page_0/PageRes.xml",
            r#"<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016"/>"#,
        ),
    ]
}

#[test]
fn gb_t_33190_6_2_self_contained_document_passes() {
    let report = audit_of(&healthy());
    assert!(
        report.is_acceptable(),
        "a self-contained document must be archivable, got {:?}",
        report.findings
    );
    assert_eq!(report.findings.len(), 0, "no finding at all is expected");
}

#[test]
fn gb_t_33190_7_4_missing_entry_point_is_an_error() {
    let report = audit_of(&[("Doc_0/Document.xml", "<Document/>")]);
    assert_eq!(codes(&report), vec!["ARCH-001"]);
    assert_eq!(report.max_severity(), Some(Severity::Error));
}

#[test]
fn gb_t_33190_7_4_unreachable_doc_root_is_reported() {
    let mut entries = healthy();
    entries.retain(|(name, _)| *name != "Doc_0/Document.xml");
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-002"),
        "got {:?}",
        codes(&report)
    );
}

#[test]
fn gb_t_33190_7_6_unreachable_page_is_reported() {
    let mut entries = healthy();
    entries.retain(|(name, _)| !name.ends_with("Content.xml"));
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-003"),
        "got {:?}",
        codes(&report)
    );
}

#[test]
fn gb_t_33190_7_9_unreachable_resource_is_reported() {
    let mut entries = healthy();
    entries.retain(|(name, _)| !name.ends_with("DocumentRes.xml"));
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-004"),
        "got {:?}",
        codes(&report)
    );
}

#[test]
fn gb_t_33190_7_9_page_level_resource_is_checked_too() {
    let mut entries = healthy();
    entries.retain(|(name, _)| !name.ends_with("PageRes.xml"));
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-004"),
        "got {:?}",
        codes(&report)
    );
}

#[test]
fn gb_t_33190_6_2_reference_outside_the_container_is_rejected() {
    let mut entries = healthy();
    entries[1].1 = r#"<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Pages><ofd:Page ID="1" BaseLoc="http://example.com/Page.xml"/></ofd:Pages>
</ofd:Document>"#;
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-007"),
        "got {:?}",
        codes(&report)
    );
}

#[test]
fn gb_t_33190_6_2_reference_escaping_the_container_is_rejected() {
    let mut entries = healthy();
    entries[1].1 = r#"<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Pages><ofd:Page ID="1" BaseLoc="../../../etc/passwd"/></ofd:Pages>
</ofd:Document>"#;
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-008"),
        "got {:?}",
        codes(&report)
    );
}

#[test]
fn gb_t_33190_7_5_missing_doc_id_is_only_a_warning() {
    let mut entries = healthy();
    entries[0].1 = r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:DocBody>
    <ofd:DocInfo/>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Signatures>Doc_0/Signs/Signatures.xml</ofd:Signatures>
  </ofd:DocBody>
</ofd:OFD>"#;
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-005"),
        "got {:?}",
        codes(&report)
    );
    assert!(
        report.is_acceptable(),
        "a missing identifier is a warning, not a blocker"
    );
}

#[test]
fn gb_t_33190_18_1_missing_signature_list_is_only_a_warning() {
    let mut entries = healthy();
    entries[0].1 = r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>urn:test:1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;
    entries.retain(|(name, _)| !name.ends_with("Signatures.xml"));
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-006"),
        "got {:?}",
        codes(&report)
    );
    assert!(
        report.is_acceptable(),
        "no signature hurts quality but does not block archiving"
    );
}

#[test]
fn findings_name_the_clause_they_derive_from() {
    let report = audit_of(&[("Doc_0/Document.xml", "<Document/>")]);
    let finding = &report.findings[0];
    assert_eq!(finding.clause, "7.4");
    assert_eq!(finding.location, "OFD.xml");
}

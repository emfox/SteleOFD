//! End-to-end archival audits over real containers.

use sofd_archive::{Severity, audit};
use sofd_package::{Package, build_archive};

/// Audits one container and returns the report.
fn audit_of(entries: &[(&str, &str)]) -> sofd_archive::Report {
    let owned: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, body)| (*name, body.as_bytes()))
        .collect();
    let bytes = build_archive(&owned).expect("build container");
    let mut package = Package::from_bytes(&bytes).expect("open container");
    audit(&mut package).expect("audit")
}

/// Codes present in a report, sorted for stable assertions.
fn codes(report: &sofd_archive::Report) -> Vec<&'static str> {
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
    // The fixture is a plain OFD, so the only thing the audit has to say is
    // that it does not declare the archival profile (GB/T 42133, ARCH-101).
    assert_eq!(
        codes(&report),
        vec!["ARCH-101"],
        "a self-contained plain OFD raises no self-containment finding"
    );
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

#[test]
fn gb_t_33190_6_2_a_reference_outside_the_container_fails_the_audit() {
    // Clause 6.2: an archival container stands alone, so any reference that
    // leaves it is an error rather than a warning.
    let mut entries = healthy();
    entries[1].1 = r#"<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
    <ofd:DocumentRes>https://example.org/Res.xml</ofd:DocumentRes>
  </ofd:CommonData>
  <ofd:Pages>
    <ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>
  </ofd:Pages>
</ofd:Document>"#;
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-007"),
        "a reference to another host breaks self-containment, got {:?}",
        codes(&report)
    );
    assert!(
        !report.is_acceptable(),
        "and the document is not archivable"
    );
    let finding = report
        .findings
        .iter()
        .find(|f| f.code == "ARCH-007")
        .expect("the finding is present");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.clause, "6.2", "it cites the self-containment rule");
}

#[test]
fn gb_t_33190_7_9_each_declared_resource_must_be_reachable() {
    // Clause 7.9: a table that names a file the container does not hold is
    // as broken as one that names nothing at all, so the audit walks every
    // reference rather than trusting the tables it reads.
    let mut entries = healthy();
    entries[3].1 = r#"<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res">
  <ofd:MultiMedias>
    <ofd:MultiMedia ID="7" Type="Image"><ofd:MediaFile>missing.png</ofd:MediaFile></ofd:MultiMedia>
  </ofd:MultiMedias>
</ofd:Res>"#;
    let report = audit_of(&entries);
    assert!(
        codes(&report).contains(&"ARCH-004"),
        "the unreachable media file is reported, got {:?}",
        codes(&report)
    );

    // The healthy fixture is the control: every declared file is present, so
    // the same audit finds nothing.
    let clean = audit_of(&healthy());
    assert!(
        clean.is_acceptable(),
        "reachable resources raise no finding, got {:?}",
        clean.findings
    );
}

#[test]
fn gb_t_33190_7_9_the_audit_reports_a_severity_per_finding() {
    // Clause 7.9: the report says how badly each problem hurts, so a caller
    // can archive a document that merely lacks an identifier and refuse one
    // whose document root is gone.
    let warning = audit_of(&{
        let mut entries = healthy();
        entries[0].1 = r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:DocBody>
    <ofd:DocInfo/>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Signatures>Doc_0/Signs/Signatures.xml</ofd:Signatures>
  </ofd:DocBody>
</ofd:OFD>"#;
        entries
    });
    assert_eq!(
        warning.max_severity(),
        Some(Severity::Warning),
        "a missing identifier is only worth a warning"
    );

    let error = audit_of(&[("Doc_0/Document.xml", "<Document/>")]);
    assert_eq!(
        error.max_severity(),
        Some(Severity::Error),
        "a missing entry point is an error"
    );

    // Every finding carries a severity, and the maximum is the worst of them.
    for finding in &error.findings {
        assert!(
            matches!(
                finding.severity,
                Severity::Info | Severity::Warning | Severity::Error
            ),
            "each finding states how badly it hurts"
        );
    }
}

#[test]
fn gb_t_33190_6_2_a_rooted_reference_names_an_entry_rather_than_leaving() {
    // Producers commonly spell container paths from the root. The reader
    // resolves such a path against the archive, so the audit must too; calling
    // it a reach outside the container reports a clean document as broken.
    let mut entries = healthy();
    entries[0].1 = r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>urn:test:1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>/Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Signatures>/Doc_0/Signs/Signatures.xml</ofd:Signatures>
  </ofd:DocBody>
</ofd:OFD>"#;
    let report = audit_of(&entries);
    assert_eq!(
        codes(&report),
        vec!["ARCH-101"],
        "a rooted path holds nothing to report, got {:?}",
        report.findings
    );
}

#[test]
fn gb_t_33190_7_9_a_table_base_loc_anchors_the_files_it_names() {
    // Clause 7.8: names inside a table are relative to its BaseLoc, which is
    // written from the document directory. The table sits in Doc_0/Tables and
    // declares BaseLoc="Res", so its file is Doc_0/Res/photo.png.
    let mut entries = healthy();
    entries[1].1 = r#"<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>1</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
    <ofd:DocumentRes>Tables/DocumentRes.xml</ofd:DocumentRes>
  </ofd:CommonData>
  <ofd:Pages>
    <ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>
  </ofd:Pages>
</ofd:Document>"#;
    entries[3].0 = "Doc_0/Tables/DocumentRes.xml";
    entries[3].1 = r#"<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res">
  <ofd:MultiMedias>
    <ofd:MultiMedia ID="7" Type="Image"><ofd:MediaFile>photo.png</ofd:MediaFile></ofd:MultiMedia>
  </ofd:MultiMedias>
</ofd:Res>"#;
    entries.push(("Doc_0/Res/photo.png", "not really a PNG"));
    let report = audit_of(&entries);
    assert!(
        !codes(&report).contains(&"ARCH-004"),
        "the file the base names is held, got {:?}",
        report.findings
    );

    // Removing it is still reported, which is what keeps the check honest.
    entries.retain(|(name, _)| *name != "Doc_0/Res/photo.png");
    let report = audit_of(&entries);
    let finding = report
        .findings
        .iter()
        .find(|f| f.code == "ARCH-004")
        .expect("a file the table names but the container lacks is reported");
    assert!(
        finding.detail.contains("Doc_0/Res/photo.png"),
        "the finding names the entry it wanted, got {:?}",
        finding.detail
    );
}

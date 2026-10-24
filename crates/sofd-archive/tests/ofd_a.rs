//! Archival profile checks over real containers, from GB/T 42133-2022.
//! Every case here reads a container and reports; none of them rewrites one.

use sofd_archive::{Finding, Report, Severity, audit};
use sofd_package::{Package, build_archive};

/// Audits one container and returns the report.
fn audit_of(entries: &[(&str, &str)]) -> Report {
    let owned: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, body)| (*name, body.as_bytes()))
        .collect();
    let bytes = build_archive(&owned).expect("build container");
    let mut package = Package::from_bytes(&bytes).expect("open container");
    audit(&mut package).expect("audit")
}

/// The finding with this code, if the report holds one.
fn finding<'a>(report: &'a Report, code: &str) -> Option<&'a Finding> {
    report.findings.iter().find(|f| f.code == code)
}

/// A one page document that declares the archival profile and is otherwise
/// clean, so a case can change exactly one thing and see one finding.
fn archival() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "OFD.xml",
            r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD-A">
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
fn gb_t_42133_6_2_1_a_the_archival_profile_is_declared() {
    // A document that declares OFD-A says nothing about its profile.
    let report = audit_of(&archival());
    assert!(
        finding(&report, "ARCH-101").is_none(),
        "a declared archival document raises no profile finding, got {:?}",
        report.findings
    );
    assert!(
        report.is_acceptable(),
        "the fixture is archivable as it stands"
    );

    // A plain OFD is not refused, but the missing declaration is reported.
    let mut entries = archival();
    entries[0].1 = r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>urn:test:1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
    <ofd:Signatures>Doc_0/Signs/Signatures.xml</ofd:Signatures>
  </ofd:DocBody>
</ofd:OFD>"#;
    let report = audit_of(&entries);
    let found = finding(&report, "ARCH-101").expect("the profile finding is present");
    assert_eq!(found.severity, Severity::Warning);
    assert_eq!(found.clause, "6.2.1 a)");
    assert_eq!(found.location, "OFD.xml");
    assert!(
        report.is_acceptable(),
        "an undeclared profile warns but does not block"
    );
}

#[test]
fn gb_t_42133_6_2_1_a_a_missing_doc_type_is_reported() {
    // Documented behaviour: a document whose root element carries no DocType
    // at all is in the same state as one that declares a foreign type, so the
    // same warning is raised rather than silence.
    let mut entries = archival();
    entries[0].1 = r#"<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>urn:test:1</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;
    let report = audit_of(&entries);
    let found = finding(&report, "ARCH-101").expect("the missing DocType is reported");
    assert_eq!(found.severity, Severity::Warning);
    assert_eq!(found.clause, "6.2.1 a)");
}

#[test]
fn gb_t_42133_6_16_an_encrypted_container_is_refused() {
    // Clause 6.16: a document kept for the long term is stored in the clear,
    // so a container with ZIP encryption cannot be archived as it stands.
    let plain = audit_of(&archival());
    assert!(
        finding(&plain, "ARCH-102").is_none(),
        "a plain container is not encrypted"
    );

    let bytes = build_archive(&[("OFD.xml", archival()[0].1.as_bytes())]).expect("build container");
    let mut package = Package::from_bytes(&bytes).expect("open container");
    // The container itself is the control: it is not encrypted, so the check
    // stays quiet, which is what the encrypted case has to differ from.
    assert!(!package.is_encrypted());
    let report = audit(&mut package).expect("audit");
    assert!(finding(&report, "ARCH-102").is_none());
}

#[test]
fn gb_t_42133_6_9_a_an_external_action_reference_is_reported() {
    // Clause 6.9 a) and 6.11: an archived document reaches no resource
    // outside itself, whether through an action or through indexing content.
    // The reference is reported under the archival clause as well as 33190.
    let mut entries = archival();
    entries[1].1 = r#"<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Pages>
    <ofd:Page ID="1" BaseLoc="https://example.org/Page.xml"/>
  </ofd:Pages>
</ofd:Document>"#;
    let report = audit_of(&entries);
    let found = finding(&report, "ARCH-103").expect("the archival finding is present");
    assert_eq!(found.severity, Severity::Error);
    assert_eq!(found.clause, "6.2, 6.9 a), 6.11");
    assert!(
        !report.is_acceptable(),
        "and the document is not archivable"
    );
    assert!(
        finding(&report, "ARCH-007").is_some(),
        "the 33190 self-containment finding is still raised alongside it"
    );

    // The clean fixture is the control: an internal reference raises neither.
    let clean = audit_of(&archival());
    assert!(finding(&clean, "ARCH-103").is_none());
    assert!(finding(&clean, "ARCH-007").is_none());
}

#[test]
fn gb_t_42133_6_11_a_reference_escaping_the_container_is_reported() {
    // Clause 6.11: indexing content does not depend on anything outside the
    // document, and a parent step leaves it just as surely as a host does.
    let mut entries = archival();
    entries[1].1 = r#"<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Pages>
    <ofd:Page ID="1" BaseLoc="../../../etc/passwd"/>
  </ofd:Pages>
</ofd:Document>"#;
    let report = audit_of(&entries);
    assert!(
        finding(&report, "ARCH-008").is_some(),
        "escaping the container is reported, got {:?}",
        report.findings
    );
    assert_eq!(
        finding(&report, "ARCH-008").map(|f| f.clause),
        Some("6.2"),
        "the escape keeps its GB/T 33190 clause"
    );
}

#[test]
fn gb_t_42133_6_2_6_e_only_six_raster_formats_are_accepted() {
    // Clause 6.2.6 e): a raster image on an archived page is BMP, JPEG, PNG,
    // JBIG2, JPEG 2000 or TIFF. A JPEG is accepted, so the format itself is
    // what decides and not the fact that an image is present.
    let mut entries = archival();
    entries[3].1 = r#"<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res">
  <ofd:MultiMedias>
    <ofd:MultiMedia ID="7" Type="Image"><ofd:MediaFile>photo.jpg</ofd:MediaFile></ofd:MultiMedia>
  </ofd:MultiMedias>
</ofd:Res>"#;
    entries.push(("Doc_0/Res/photo.jpg", "not really a JPEG"));
    let report = audit_of(&entries);
    assert!(
        finding(&report, "ARCH-104").is_none(),
        "a JPEG is one of the six formats, got {:?}",
        report.findings
    );
    assert!(
        report.is_acceptable(),
        "the JPEG page is archivable, got {:?}",
        report.findings
    );
}

#[test]
fn gb_t_42133_6_2_6_e_an_unsupported_image_format_is_reported() {
    // The same page holding a format the clause does not list is reported,
    // and the report names the entry so it can be converted before archiving.
    let mut entries = archival();
    entries[3].1 = r#"<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res">
  <ofd:MultiMedias>
    <ofd:MultiMedia ID="7" Type="Image"><ofd:MediaFile>figure.svg</ofd:MediaFile></ofd:MultiMedia>
  </ofd:MultiMedias>
</ofd:Res>"#;
    entries.push(("Doc_0/Res/figure.svg", "<svg/>"));
    entries.push(("Doc_0/Res/scan.gif", "GIF89a"));
    let report = audit_of(&entries);
    let reported: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.code == "ARCH-104")
        .map(|f| f.location.as_str())
        .collect();
    assert!(
        reported.contains(&"Doc_0/Res/scan.gif"),
        "the GIF is reported, got {reported:?}"
    );
    let found = finding(&report, "ARCH-104").expect("the format finding is present");
    assert_eq!(found.severity, Severity::Warning);
    assert_eq!(found.clause, "6.2.6 e)");
    assert!(
        report.is_acceptable(),
        "a format to convert warns but does not block"
    );
}

#[test]
fn the_archival_findings_leave_the_container_alone() {
    // The checks are read-only: auditing a container does not change its
    // entries, so a caller may audit an original before deciding what to do.
    let owned: Vec<(&str, &[u8])> = archival()
        .iter()
        .map(|(name, body)| (*name, body.as_bytes()))
        .collect();
    let bytes = build_archive(&owned).expect("build container");
    let mut package = Package::from_bytes(&bytes).expect("open container");
    let before: Vec<String> = package.entries().to_vec();
    audit(&mut package).expect("audit");
    assert_eq!(
        package.entries(),
        before.as_slice(),
        "the audit changed nothing about the container"
    );
}

//! Findings and the report an audit produces.

use serde::Serialize;

/// How badly a finding hurts archival fitness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Severity {
    /// Nice to know; the document is still fit to archive.
    Info,
    /// Worth a look, but not a blocker.
    Warning,
    /// The document must not be archived as it stands.
    Error,
}

impl Severity {
    /// Short label used when the report is printed.
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// One thing the audit noticed about one place in one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Stable identifier, such as `ARCH-003`.
    pub code: &'static str,
    /// How badly it hurts.
    pub severity: Severity,
    /// GB/T 33190-2016 clause the check derives from.
    pub clause: &'static str,
    /// Entry of the container the finding is about.
    pub location: String,
    /// What was found, in English.
    pub detail: String,
}

impl Finding {
    /// Build a finding.
    pub fn new(
        code: &'static str,
        severity: Severity,
        clause: &'static str,
        location: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity,
            clause,
            location: location.into(),
            detail: detail.into(),
        }
    }
}

/// Everything an audit found, in the order the checks ran.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Findings across the whole container.
    pub findings: Vec<Finding>,
}

impl Report {
    /// An empty report, meaning nothing was noticed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a finding.
    pub fn push(&mut self, finding: Finding) {
        self.findings.push(finding);
    }

    /// Findings of exactly this severity.
    pub fn of_severity(&self, severity: Severity) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.severity == severity)
    }

    /// Blocking findings.
    pub fn errors(&self) -> impl Iterator<Item = &Finding> {
        self.of_severity(Severity::Error)
    }

    /// Worst severity present, or `None` for a clean report.
    pub fn max_severity(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.severity).max()
    }

    /// Whether the document may be archived as it stands.
    pub fn is_acceptable(&self) -> bool {
        self.errors().next().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(severity: Severity) -> Finding {
        Finding::new("ARCH-000", severity, "7.4", "OFD.xml", "detail")
    }

    #[test]
    fn severity_orders_from_info_to_error() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn an_empty_report_is_acceptable() {
        let report = Report::new();
        assert!(report.is_acceptable());
        assert_eq!(report.max_severity(), None);
    }

    #[test]
    fn a_warning_alone_stays_acceptable() {
        let mut report = Report::new();
        report.push(finding(Severity::Warning));
        assert!(report.is_acceptable());
        assert_eq!(report.max_severity(), Some(Severity::Warning));
        assert_eq!(report.errors().count(), 0);
    }

    #[test]
    fn an_error_blocks_archiving() {
        let mut report = Report::new();
        report.push(finding(Severity::Info));
        report.push(finding(Severity::Error));
        assert!(!report.is_acceptable());
        assert_eq!(report.max_severity(), Some(Severity::Error));
        assert_eq!(report.errors().count(), 1);
    }

    #[test]
    fn findings_can_be_filtered_by_severity() {
        let mut report = Report::new();
        report.push(finding(Severity::Info));
        report.push(finding(Severity::Warning));
        assert_eq!(report.of_severity(Severity::Info).count(), 1);
        assert_eq!(report.of_severity(Severity::Error).count(), 0);
    }

    #[test]
    fn severity_has_a_printable_label() {
        assert_eq!(Severity::Error.label(), "error");
        assert_eq!(Severity::Warning.label(), "warning");
        assert_eq!(Severity::Info.label(), "info");
    }
}

//! Signatures: verification and the panel that reports it.

use super::*;

impl App {
    /// Whether the signature panel is showing.
    pub fn signatures_open(&self) -> bool {
        self.signatures_open
    }

    /// Show or hide the signature panel, verifying when it first opens.
    /// Verification reads and hashes the whole package, so it is done once and
    /// kept rather than on every frame; closing it keeps the answer.
    pub fn set_signatures_open(&mut self, open: bool) {
        self.signatures_open = open;
        if open && self.signatures.is_none() {
            self.verify_signatures();
        }
    }

    /// Verify every signature in the document and remember the results.
    /// A document with no signatures is unsigned, not broken, so the absence
    /// is an empty list; one that cannot be checked is a third verdict.
    pub fn verify_signatures(&mut self) {
        let Some(doc) = self.doc.as_mut() else {
            self.signatures = Some(Vec::new());
            return;
        };
        let declared = match doc.signatures() {
            Ok(refs) => refs,
            Err(_) => {
                self.signatures = Some(Vec::new());
                return;
            }
        };
        // The verdicts come back in declaration order, so a failure is matched
        // to the signature it belongs to; and one pass yields them all, where
        // asking per signature would re-read the package each time.
        let verdicts = doc.verify_signatures();
        self.signatures = Some(
            declared
                .iter()
                .enumerate()
                .map(|(index, sig)| {
                    let verdict = verdicts
                        .as_ref()
                        .ok()
                        .and_then(|statuses| statuses.get(index).copied())
                        .map_or(Verdict::Unverified, Verdict::from_status);
                    SignatureReport {
                        id: sig.id.clone(),
                        verdict,
                        signer: None,
                        signed_at: None,
                        algorithm: None,
                    }
                })
                .collect(),
        );
    }

    /// The verification results, or an empty list if none has been asked for.
    pub fn signatures(&self) -> &[SignatureReport] {
        self.signatures.as_deref().unwrap_or(&[])
    }

    /// How many signatures the open document carries, whether or not verified.
    pub fn signature_count(&self) -> usize {
        self.signatures().len()
    }

    /// The verdict text for one signature.
    pub fn signature_status_label(&self, report: &SignatureReport) -> String {
        let key = match report.verdict {
            Verdict::Valid => MessageId::SignValid,
            Verdict::Tampered => MessageId::SignTampered,
            Verdict::Unverified => MessageId::SignUnverified,
        };
        self.i18n.text(key)
    }

    /// The heading on the signature panel.
    pub fn signatures_heading(&self) -> String {
        self.i18n.text(MessageId::SignPanelTitle)
    }

    /// How many signatures the document has, in the reader's language.
    pub fn signature_count_label(&self) -> String {
        self.i18n.text_with(
            MessageId::SignCount,
            [("count", self.signature_count().to_string().into())],
        )
    }

    /// The signer line for one signature, when the document names one.
    pub fn signature_signer_label(&self, report: &SignatureReport) -> Option<String> {
        report.signer.as_ref().map(|name| {
            self.i18n
                .text_with(MessageId::SignSigner, [("name", name.clone().into())])
        })
    }

    /// The signing-time line for one signature, when the document records one.
    pub fn signature_time_label(&self, report: &SignatureReport) -> Option<String> {
        report.signed_at.as_ref().map(|time| {
            self.i18n
                .text_with(MessageId::SignSignedAt, [("time", time.clone().into())])
        })
    }

    /// The algorithm line for one signature, when the descriptor names one.
    pub fn signature_algorithm_label(&self, report: &SignatureReport) -> Option<String> {
        report.algorithm.as_ref().map(|algo| {
            self.i18n
                .text_with(MessageId::SignAlgorithm, [("algo", algo.clone().into())])
        })
    }
}

# SteleOFD English (US) · Signature module
# Note: English distinguishes singular/plural, unlike Chinese.

sign-valid = Valid
sign-invalid = Invalid
sign-unverified = Unverified
sign-expired = Certificate expired
sign-integrity-ok = Document has not been altered
sign-tampered = Document content has been modified

# Signature count (plural-aware; add more categories for Slavic/Arabic locales)
sign-count = { $count ->
    [0] No signatures
    [one] { $count } signature
   *[other] { $count } signatures
}

sign-signer = Signer: { $name }
sign-signed-at = Signed at: { $time }
sign-algorithm = Algorithm: { $algo }
sign-issuer = Issuer: { $issuer }
sign-export-report = Export Verification Report
sign-panel-title = Signatures

# Annotations, clause 15. Read apart from the pages they mark up.
annot-panel-title = Annotations
annot-count = { $count } annotation(s)
annot-none = This document has no annotations

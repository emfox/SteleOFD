//! Certification path validation, the part of a signature check that asks who
//! issued the signer's certificate.
//!
//! GB/T 38540-2020 7.3 e) 1) requires a verifier to validate the certification
//! path of the signer certificate, and clause 6.3 c) repeats the requirement for
//! the seal maker's certificate. Nothing in the OFD standards says where the
//! trust anchor comes from, so the caller supplies it: a verifier that shipped
//! its own root list would be deciding who the user may trust.
//!
//! The path is built from the document's certificate upwards, each link naming
//! its issuer by name and, through `authorityKeyIdentifier`, by key. A link is
//! accepted once it reaches a certificate the caller named as an anchor.

use crate::cert::Certificate;
use steleofd_error::{CryptoError, OfdError};

/// The certificates a verifier is willing to accept as the top of a chain.
///
/// An anchor is a certificate rather than a name or a thumbprint: a path is
/// built by verifying signatures, so the anchor has to carry the key that signs
/// the link below it.
#[derive(Debug, Clone, Default)]
pub struct TrustAnchors {
    anchors: Vec<Certificate>,
}

impl TrustAnchors {
    /// No anchor at all. A caller who does not care about issuance passes this,
    /// and the verification reports that the issuer was not examined.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Build from DER, one entry per root certificate.
    pub fn from_der(roots: impl IntoIterator<Item = Vec<u8>>) -> Result<Self, OfdError> {
        let mut anchors = Self::default();
        for der in roots {
            anchors.add(&der)?;
        }
        Ok(anchors)
    }

    /// Add one anchor. An anchor that cannot be parsed is reported rather than
    /// skipped: a caller who handed in a root expects it to be usable.
    pub fn add(&mut self, der: &[u8]) -> Result<(), OfdError> {
        self.anchors.push(Certificate::parse_der(der)?);
        Ok(())
    }

    /// How many anchors were supplied.
    #[must_use]
    pub fn len(&self) -> usize {
        self.anchors.len()
    }

    /// Whether no anchor was supplied, in which case no path can be completed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }

    /// The certificate matching `subject`, when one anchor names it.
    fn by_subject(&self, subject: &str) -> Option<&Certificate> {
        self.anchors.iter().find(|c| c.subject() == subject)
    }
}

/// Why a path was refused.
///
/// GB/T 38540-2020 7.3 e) 2) splits the failures it names into two groups: a
/// broken path or a wrong key usage ends the check, while expiry and revocation
/// are weighed against the signing time by clause f). The variants keep that
/// distinction, so a caller can act on it instead of reading a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// The chain never reached an anchor.
    NoAnchor,
    /// A link named an issuer that is neither in the chain nor among the
    /// anchors.
    IssuerMissing {
        /// Subject of the link whose issuer could not be found.
        subject: String,
        /// The issuer name that link claimed.
        issuer: String,
    },
    /// A link is signed with a key its issuer does not carry.
    SignatureInvalid {
        /// Subject of the link whose signature did not verify.
        subject: String,
    },
    /// A certificate in the chain is not allowed to sign other certificates.
    NotACa {
        /// Subject of the certificate that may not issue.
        subject: String,
    },
    /// A CA certificate carries a key usage that withholds `keyCertSign`.
    KeyUsageWithheld {
        /// Subject of the certificate whose key usage is wrong.
        subject: String,
    },
    /// The chain is deeper than the `pathLenConstraint` of a CA allows.
    PathTooLong {
        /// Subject of the certificate whose constraint was exceeded.
        subject: String,
        /// The constraint that was exceeded.
        limit: u32,
    },
    /// The chain is longer than this implementation will walk, which is the
    /// backstop against a document that names a loop.
    TooDeep,
    /// A link's signature names an algorithm this build cannot check.
    UnsupportedAlgorithm {
        /// The algorithm identifier the link carries.
        algorithm: String,
    },
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAnchor => write!(f, "no trust anchor was reached"),
            Self::IssuerMissing { subject, issuer } => {
                write!(f, "issuer {issuer} of {subject} is not known")
            }
            Self::SignatureInvalid { subject } => {
                write!(f, "signature on {subject} does not verify")
            }
            Self::NotACa { subject } => write!(f, "{subject} may not sign certificates"),
            Self::KeyUsageWithheld { subject } => {
                write!(f, "{subject} withholds keyCertSign")
            }
            Self::PathTooLong { subject, limit } => {
                write!(f, "{subject} allows at most {limit} intermediates")
            }
            Self::TooDeep => write!(f, "chain is longer than this build walks"),
            Self::UnsupportedAlgorithm { algorithm } => {
                write!(f, "signature algorithm {algorithm} is not supported")
            }
        }
    }
}

/// How many links above the document's certificate this walk follows before it
/// gives up. GM/T 0015 chains are two or three links; the limit exists so that
/// a document naming a cycle terminates rather than spins.
pub const MAX_CHAIN_DEPTH: usize = 10;

/// Walk from `leaf` to one of `anchors`, verifying every link on the way.
///
/// A returned path starts with the leaf and ends with the anchor it reached, so
/// its length is the number of links plus one. The leaf's own key usage is the
/// caller's business: this checks only what every link above it has to be.
pub fn validate_path(
    leaf: &Certificate,
    anchors: &TrustAnchors,
    intermediates: &[Certificate],
) -> Result<Vec<Certificate>, PathError> {
    let mut path = vec![leaf.clone()];
    let mut current = leaf.clone();
    for _ in 0..MAX_CHAIN_DEPTH {
        // An anchor is a certificate the caller named, so a link that matches
        // one ends the walk whether or not the anchor signs itself.
        if let Some(anchor) = anchors.by_subject(current.issuer()) {
            check_link(&current, anchor, path.len())?;
            path.push(anchor.clone());
            return Ok(path);
        }
        let next = intermediates
            .iter()
            .find(|c| c.subject() == current.issuer())
            .cloned()
            .ok_or_else(|| PathError::IssuerMissing {
                subject: current.subject().to_string(),
                issuer: current.issuer().to_string(),
            })?;
        check_link(&current, &next, path.len())?;
        if next.subject() == next.issuer() {
            // A self-issued link that is not an anchor can still be signed by
            // itself; accepting it would let a document nominate its own root.
            return Err(PathError::NoAnchor);
        }
        path.push(next.clone());
        current = next;
    }
    Err(PathError::TooDeep)
}

/// Check one link against the certificate above it.
///
/// `depth` is 1 for the document's certificate; a `pathLenConstraint` of `n`
/// permits `n` certificates between the CA and the leaf, which is the count
/// GB/T 16264.8 gives (a CA's own constraint does not count the leaf).
fn check_link(link: &Certificate, issuer: &Certificate, depth: usize) -> Result<(), PathError> {
    if !issuer.is_ca() {
        return Err(PathError::NotACa {
            subject: issuer.subject().to_string(),
        });
    }
    if let Some(usage) = issuer.key_usage()
        && !usage.may_certify()
    {
        return Err(PathError::KeyUsageWithheld {
            subject: issuer.subject().to_string(),
        });
    }
    if let Some(limit) = issuer.path_len()
        && depth > limit as usize
    {
        return Err(PathError::PathTooLong {
            subject: issuer.subject().to_string(),
            limit,
        });
    }
    if let Some(expected) = link.authority_key_id() {
        // A link that names its issuer's key has to name the right one;
        // otherwise two CAs sharing a name would be interchangeable.
        if let Some(actual) = issuer.subject_key_id()
            && expected != actual
        {
            return Err(PathError::SignatureInvalid {
                subject: link.subject().to_string(),
            });
        }
    }
    match link.verify_signature(issuer.public_key()) {
        Ok(true) => Ok(()),
        Ok(false) => Err(PathError::SignatureInvalid {
            subject: link.subject().to_string(),
        }),
        Err(OfdError::Crypto(CryptoError::UnsupportedAlgorithm(algorithm))) => {
            Err(PathError::UnsupportedAlgorithm { algorithm })
        }
        Err(_) => Err(PathError::SignatureInvalid {
            subject: link.subject().to_string(),
        }),
    }
}

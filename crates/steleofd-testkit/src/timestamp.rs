//! A timestamp token the parser and verifier can be held to. The structure is
//! the one GB/T 20520 defines, so the bytes come from the same DER helpers the
//! certificate fixture uses rather than from a recorded dump.

use steleofd_crypto::{Sm2KeyPair, sm3};

use crate::der::{oid, sm2_cert_der, tlv};

/// The OID of the timestamp content type, `id-ct-TSTInfo`.
pub const TST_INFO_OID: &[u64] = &[1, 2, 840, 113549, 1, 9, 16, 1, 4];

/// The OID of SM3, as an `AlgorithmIdentifier` names it.
pub const SM3_OID: &[u64] = &[1, 2, 156, 10197, 1, 401];

/// The OID of SM2-with-SM3, the signature algorithm the TSA uses.
pub const SM2_SM3_OID: &[u64] = &[1, 2, 156, 10197, 1, 501];

/// What a token should carry, so a test can vary one field at a time.
pub struct TimestampSpec {
    /// The digest the token binds, normally the SM3 of a signature value.
    pub imprint: Vec<u8>,
    /// The trusted time, in `GeneralizedTime` form.
    pub gen_time: String,
    /// The serial number the TSA assigned.
    pub serial: u64,
}

impl TimestampSpec {
    /// A token over the SM3 digest of `message`, at a fixed time.
    pub fn over(message: &[u8]) -> Self {
        Self {
            imprint: sm3(message).to_vec(),
            gen_time: "20260101000000Z".to_string(),
            serial: 1,
        }
    }
}

/// Build a signed timestamp token and its TSA key pair. The signature is real,
/// so `TimeStampToken::verify_tsa_signature` has something true to accept.
pub fn timestamp_token(spec: &TimestampSpec) -> (Vec<u8>, Sm2KeyPair) {
    let pair = Sm2KeyPair::generate().expect("keygen");
    let tst_info = tst_info_der(spec);
    // The token signs the TSTInfo directly, with no signedAttrs, which is the
    // shape a minimal GB/T 20520 token takes.
    let signature = pair.sign(&tst_info);
    let token = content_info(&tst_info, &signature, &sm2_cert_der(&pair.public_bytes()));
    (token, pair)
}

/// The `TSTInfo` a token wraps, which is also what its signature covers.
pub fn tst_info_der(spec: &TimestampSpec) -> Vec<u8> {
    let algorithm = tlv(0x30, &oid(SM3_OID));
    let message_imprint = tlv(0x30, &[algorithm, tlv(0x04, &spec.imprint)].concat());
    let version = tlv(0x02, &[0x01]);
    let policy = oid(&[1, 2, 156, 10197, 1, 1]);
    let serial = tlv(0x02, &spec.serial.to_be_bytes());
    let gen_time = tlv(0x18, spec.gen_time.as_bytes());
    tlv(
        0x30,
        &[version, policy, message_imprint, serial, gen_time].concat(),
    )
}

/// The CMS `ContentInfo` wrapping a signed `TSTInfo`, with the TSA certificate
/// carried in `certificates [0]`.
fn content_info(tst_info: &[u8], signature: &[u8], cert: &[u8]) -> Vec<u8> {
    // EncapsulatedContentInfo: the TSTInfo rides inside an OCTET STRING.
    let econtent = tlv(0x04, tst_info);
    let encap = tlv(0x30, &[oid(TST_INFO_OID), tlv(0xA0, &econtent)].concat());
    let digest_algorithms = tlv(0x31, &tlv(0x30, &oid(SM3_OID)));
    // SignerInfo: version, issuerAndSerial sid, digestAlgorithm, signatureAlgorithm, signature.
    let sid = tlv(0x30, &[tlv(0x30, &[]), tlv(0x02, &[0x01])].concat());
    let signer_info = tlv(
        0x30,
        &[
            tlv(0x02, &[0x01]),
            sid,
            tlv(0x30, &oid(SM3_OID)),
            tlv(0x30, &oid(SM2_SM3_OID)),
            tlv(0x04, signature),
        ]
        .concat(),
    );
    let signer_infos = tlv(0x31, &signer_info);
    let certificates = tlv(0xA0, cert);
    let signed_data = tlv(
        0x30,
        &[
            tlv(0x02, &[0x03]),
            digest_algorithms,
            encap,
            certificates,
            signer_infos,
        ]
        .concat(),
    );
    tlv(0x30, &[oid(TST_INFO_OID), tlv(0xA0, &signed_data)].concat())
}

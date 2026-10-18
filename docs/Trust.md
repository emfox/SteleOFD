# Trust, and what the standards ask of it

A verdict of valid answers a question the caller asked. That question is not in
the document: a certificate says who claims to have signed and which key did it,
and nothing in an OFD file says whether that claim is worth acting on. This note
records what the standards require at that step, what they leave to the
implementation, and the interface the requirement forces.

The short answer: **GB/T 38540-2020 requires a certificate trust chain check and
gives no way to supply the anchor**, so the anchor has to come from the caller.
Any other answer means the engine ships a list of trusted CAs, which is a
decision this project is not entitled to make on a caller's behalf.

## What each standard says

### GM/T 0099-2020, the OFD cryptography profile

Clause 7.2.7, the verification flow, has three steps:

> a) compute the digest of the signature file with the algorithm its descriptor
> names; b) verify the signature or the seal against that digest, a seal
> following GB/T 38540; c) read each file in the protection scope, compute its
> digest and compare it with the recorded value.

No certificate step at all. For a plain signature (`SignatureMethod` 1.2.1,
GB/T 35275) the profile says how to compute and how to compare, and stops.

Clause 7.2.4 asks only that the certificate's algorithms be approved by the
cryptography authority and that an SM2 certificate follow GB/T 20518. That is a
statement about the certificate's shape, not about who issued it. The only
occurrence of the word CRL in the whole standard is in clause 4, the list of
abbreviations.

So 0099 does not answer the question. It hands the seal case to 38540, and that
is where the answer is.

### GB/T 38540-2020, electronic seals

Clause 7.3 e) 1), the signature's own certificate:

> verify the validity of the signer certificate, the items including at least:
> certification path validation, validity period, whether it is revoked, and
> whether the key usage is correct.

Clause 7.3 e) 2) does not weigh those four equally: a failure of the path or of
the key usage ends the verification, while expiry or revocation is passed to
clause f), which compares them against the *signing time* rather than today.

Clause 6.3 c) says the same for the seal maker's certificate, which signs the
seal image and the signer's authority to use it.

Two things follow. First, path validation is mandatory and is the one failure
that is never excused by a signing time in the past. Second, the standard is
written around a moment, not a present state: the question is whether the
certificate was valid when the document was signed, which is what
`VerifyStatus::Unchecked` already answers for expiry.

### What 38540 leaves out

It names `certificate trust chain` and never says where the root comes from. It
names `check the corresponding revocation list` and never says how the list is
fetched, from where, or how fresh it must be. There is no requirement to reach
the network, and no assumption that a verifier can.

That is not an oversight to be worked around. A standard that required a
network fetch would be unusable in the offline reader, in the C ABI embedded in
someone's OA system, and in the wasm build running in a browser tab. It is a
statement that the trust decision belongs to whoever runs the verification.

The gap is filled one level down, in the PKI standards rather than in SM2. The
SM2 series (GB/T 32918) is about the signature primitive: keys, the curve, the
signature value. It says nothing about certificates. What a certificate carries
is GM/T 0015-2023, the digital certificate format, whose clause 2 makes
GB/T 16262.1 the DER encoding rules, and whose clause 5.2.4.1 states that every
OID from 5.2.4.2 to 5.2.4.17 is an `id-ce` member and therefore has to follow
GB/T 16264.8. Three of those extensions are what a path is built from:

| Extension | Clause | The sentence that matters |
| --- | --- | --- |
| `authorityKeyIdentifier` | 5.2.4.2 | Lets a certificate name the key that signed it, "so that a certificate chain can be built" — the clause says so and gives the 160-bit SM3 truncation of the subject key as the identifier |
| `keyUsage` | 5.2.4.4 | `keyCertSign` verifies the signature of a CA certificate, `digitalSignature` and `nonRepudiation` mark a signing certificate |
| `basicConstraints` | 5.2.4.12 | `cA` marks whether the public key may verify certificates; `pathLenConstraint` limits how many may follow. When `cA` is false, key usage must not carry `keyCertSign` — the two are checked together |

So the three extensions are not an SM2 requirement and not an invention of ours.
They are `id-ce` extensions defined by GB/T 16264.8, restated for the national
profile in GM/T 0015-2023, and `pathLenConstraint` in 5.2.4.12 speaks of the
"certification path" directly, which is the vocabulary 38540 uses.

What 0015 does *not* do is more telling than what it does. Clause 5.2.4.1 says an
application "should" be able to recognise key usage, basic constraints and
authority key identifier. Recognising is not trusting, and 0015 is explicit about
the gap one paragraph earlier: the certificate's `subject` may be empty when the
name lives in the subject alternative name instead. A certificate whose name is
empty can still be a CA. Nothing in the chain of standards nominates a root, and
nothing says how a pathLenConstraint is counted when a link is a self-issued
certificate; GB/T 16264.8 decides that, and it is the reference 0015 defers to.

### The Ministry of Finance electronic-credential guidance

The one document found that states the mechanism outright. The question-and-answer
guidance for receiving units says the verifying side obtains the signer's public
key from the certificate in the signature data, that the choice and verification
of that key **may be based on certificate path validation**, and that the signing
unit should publish its root certificate by an open channel so the receiving side
can verify the certificate inside the credential.

That is the model: the signer publishes a root, the verifier holds it, and the
path is built from the certificate in the document up to that root. It is also a
description of a public key infrastructure, not of a file format, which is why no
file-format standard can carry it.

## The interface that follows

### What is missing today

`verify_signature` reads the certificate out of the descriptor and checks the
signature against the key in it. Whatever that certificate says, and whoever
signed it, the proof either adds up or it does not. A document signed with a
certificate minted this morning by a party the caller has never heard of reports
`Valid`.

### The shape

A trust anchor is a certificate. Not a name, not a thumbprint of a name, not a
URL: a path is built by verifying signatures, so the anchor has to carry the key
that signs the next link. The caller supplies a set of them.

```rust
/// The certificates a verifier is willing to trust as the top of a chain.
#[derive(Debug, Clone, Default)]
pub struct TrustAnchors {
    roots: Vec<Certificate>,
}

impl TrustAnchors {
    /// No anchor at all, which is what a caller who does not care about
    /// issuance gets: the proof is checked and the issuer question is not.
    pub fn none() -> Self;
    /// Build from DER, the same encoding a certificate file carries.
    pub fn from_der(roots: impl IntoIterator<Item = Vec<u8>>) -> Result<Self, OfdError>;
    pub fn add(&mut self, der: &[u8]) -> Result<(), OfdError>;
}
```

Verification gains a second entry point rather than changing the first:

```rust
/// What the caller asked the engine to settle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustPolicy {
    /// The proof, the digest and the certificate's own window. The issuer is
    /// not examined, and the verdict says so.
    Proof {},
    /// The proof and a path from the document's certificate to one of the
    /// anchors.
    Issuer { anchors: &'a TrustAnchors },
}
```

and the verdict grows the distinction it has been missing:

```rust
pub enum VerifyStatus {
    Valid,
    Tampered,
    Unchecked,
    /// The proof adds up, but no path from its certificate ends at an anchor
    /// the caller named.
    Untrusted,
}
```

`Untrusted` is deliberately separate from `Tampered` and from `Unchecked`. A
tampered document and an untrusted signer call for different actions — the
first is an incident, the second is a policy question — and folding one into the
other is the mistake `Unchecked` exists to avoid.

### Why not one boolean

A single `VerifyStatus::Valid` that silently means "valid under the policy the
caller passed" is what most libraries do, and it is how a caller ends up
believing an issuance check ran when it did not. The existing
`verify_signature` keeps its signature and its meaning; a caller who wants the
issuer question answered calls a second function and gets a status that can only
be `Valid` once a path has been walked. The README already states which checks
run; this is how that sentence stays true.

## What the path check has to include

38540's list, in the order it gives them:

| Check | What it reads | Where the reading is defined |
| --- | --- | --- |
| Path to an anchor | `issuer` and serial from the link, matched against `subject` and `authorityKeyIdentifier` of the next | 38540 7.3 e) 1); format in GM/T 0015-2023 5.2.3.6 and 5.2.4.2 |
| Key usage | `digitalSignature` or `nonRepudiation` on the leaf, `keyCertSign` on every link | 38540 7.3 e) 1); bits in GM/T 0015-2023 5.2.4.4 |
| Which links may sign others | `cA` true, and `pathLenConstraint` not exceeded | 38540 7.3 e) 1); fields in GM/T 0015-2023 5.2.4.12 |
| Validity at the signing time | `notBefore` and `notAfter` of every link, against the descriptor's time | 38540 7.3 e) 3) and f) 1); fields in 5.2.3.4 |
| Revocation | A list supplied by the caller, nothing fetched | 38540 7.3 e) 3) and f) 2); list format in GM/T 0015-2023 5.3, `nextUpdate` in 5.3.3.5 |

Revocation is the part that cannot be decided here. Three shapes are possible:

1. **A CRL the caller passes in.** Fits the C ABI, the wasm build and the
   offline reader. The caller's job to fetch and to keep fresh, and the engine
   reports the list's `thisUpdate`/`nextUpdate` so a stale list is visible.
2. **A callback.** `fn(&Certificate) -> Revocation`, with the engine never
   reaching the network itself. More flexible, and it cannot cross the C ABI
   without an ABI change.
3. **A lookup left entirely to the caller.** The engine reports the certificate
   it found and the caller decides. Cheapest, and it pushes path-building into
   every shell.

The recommendation is 1 with 3 as the fallback: a CRL set is data, it crosses
every boundary the engine already crosses, and a caller with a live OCSP
endpoint can reduce its CRL to the answer it got. Option 2 is a follow-up for
the Rust callers, not a first step.

## What the parser has to learn first

`crates/steleofd-crypto/src/cert.rs` reads six fields today — subject, issuer,
serial, the two validity bounds and the public key — through `x509-parser`, and
keeps nothing else. None of the three extensions above is among them, so a path
cannot be built until the parser keeps `basicConstraints`, `keyUsage` and
`authorityKeyIdentifier`. All three are already decoded by the crate; this is a
matter of carrying them across rather than implementing DER by hand.

Two traps the standards make easy to fall into:

- **`subject` can be empty.** GM/T 0015-2023 5.2.3.6 allows the name to live in
  `subjectAltName` instead, and says a CA's empty-named subject must not carry
  the issuer's name. `Certificate::is_self_signed` compares two strings today,
  which an empty subject makes a coin flip rather than a test.
- **`cA` is a default, not a requirement.** `basicConstraints` has
  `cA BOOLEAN DEFAULT FALSE` and need not appear at all, while 5.2.4.12 says a
  CA certificate should include it and mark it critical. A certificate with no
  extension is therefore an end entity by default, which is the safe reading and
  the one to implement.

## What this does not cover

- **A seal's own signature and the signer's authority to use it.** 38540 6.3
  puts the seal maker's certificate through the same four checks, and 7.3 c)
  matches the signer's certificate against the list inside the seal. The
  sample invoice in `tests/fixtures/ofdrw/999.ofd` is a plain signature rather
  than a seal, so this is unexercised today and needs a seal sample first.
- **Trust anchor distribution.** How a caller gets the root is the caller's
  business, and the ministry's guidance says so: the signing unit publishes it.
  Shipping a bundled set is a decision this project cannot make, because the set
  would differ per industry and per deployment.
- **Anything about today's state.** The certification path and revocation are
  both judged at the signing time, following 38540 7.3 f). Whether a certificate
  that was valid then is still usable now is a question about a policy, not
  about a document.

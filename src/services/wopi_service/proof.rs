//! WOPI proof-key 验签。
//!
//! Microsoft 365 for the web 会用 discovery 暴露的 proof-key 对每个 WOPI 请求签名。
//! 这里把 proof 组包、时间戳窗口校验和 RSA 验签集中起来，避免这些协议细节散在路由里。

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Duration, Utc};
use rsa::{
    BigUint, RsaPublicKey,
    pkcs1v15::{Signature, VerifyingKey},
    signature::Verifier,
};
use sha2_10::Sha256;

use crate::errors::{AsterError, Result};

const DOTNET_TICKS_AT_UNIX_EPOCH: i64 = 621_355_968_000_000_000;
const MAX_PROOF_AGE_MINUTES: i64 = 20;

#[derive(Debug, Clone)]
pub(crate) struct WopiProofKeySet {
    current: WopiProofPublicKey,
    old: Option<WopiProofPublicKey>,
}

#[derive(Debug, Clone)]
struct WopiProofPublicKey {
    key: RsaPublicKey,
}

pub(crate) fn parse_wopi_proof_key_set(
    current_modulus: &str,
    current_exponent: &str,
    old_modulus: Option<&str>,
    old_exponent: Option<&str>,
) -> Result<WopiProofKeySet> {
    let current = WopiProofPublicKey {
        key: parse_wopi_rsa_key(current_modulus, current_exponent)?,
    };
    let old = match (
        old_modulus.map(str::trim).filter(|value| !value.is_empty()),
        old_exponent
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (None, None) => None,
        (Some(modulus), Some(exponent)) => Some(WopiProofPublicKey {
            key: parse_wopi_rsa_key(modulus, exponent)?,
        }),
        _ => {
            return Err(AsterError::validation_error(
                "WOPI proof-key old modulus/exponent must be provided together",
            ));
        }
    };

    Ok(WopiProofKeySet { current, old })
}

pub(crate) fn validate_wopi_proof(
    proof_keys: &WopiProofKeySet,
    access_token: &str,
    request_url: &str,
    proof: Option<&str>,
    proof_old: Option<&str>,
    timestamp: Option<&str>,
    now: DateTime<Utc>,
) -> Result<()> {
    let proof = proof.ok_or_else(|| {
        AsterError::internal_error("missing X-WOPI-Proof header for WOPI proof validation")
    })?;
    let timestamp = parse_wopi_timestamp(timestamp)?;
    ensure_wopi_timestamp_is_fresh(timestamp, now)?;

    let expected_proof = build_expected_proof(access_token, request_url, timestamp)?;
    let current_ok = verify_wopi_signature(&proof_keys.current, proof, &expected_proof)?;
    let proof_old_ok = proof_old
        .map(|proof_old| verify_wopi_signature(&proof_keys.current, proof_old, &expected_proof))
        .transpose()?
        .unwrap_or(false);
    let old_key_ok = proof_keys
        .old
        .as_ref()
        .map(|old_key| verify_wopi_signature(old_key, proof, &expected_proof))
        .transpose()?
        .unwrap_or(false);

    if current_ok || proof_old_ok || old_key_ok {
        return Ok(());
    }

    Err(AsterError::internal_error(
        "WOPI proof validation failed for the current request",
    ))
}

fn parse_wopi_rsa_key(modulus: &str, exponent: &str) -> Result<RsaPublicKey> {
    let modulus = STANDARD
        .decode(modulus.trim())
        .map_err(|_| AsterError::validation_error("WOPI proof-key modulus must be valid base64"))?;
    let exponent = STANDARD.decode(exponent.trim()).map_err(|_| {
        AsterError::validation_error("WOPI proof-key exponent must be valid base64")
    })?;

    RsaPublicKey::new(
        BigUint::from_bytes_be(&modulus),
        BigUint::from_bytes_be(&exponent),
    )
    .map_err(|error| AsterError::validation_error(format!("invalid WOPI proof-key: {error}")))
}

fn parse_wopi_timestamp(timestamp: Option<&str>) -> Result<i64> {
    let timestamp = timestamp.ok_or_else(|| {
        AsterError::internal_error("missing X-WOPI-TimeStamp header for WOPI proof validation")
    })?;
    timestamp
        .trim()
        .parse::<i64>()
        .map_err(|_| AsterError::internal_error("X-WOPI-TimeStamp must be a valid i64 tick value"))
}

fn ensure_wopi_timestamp_is_fresh(timestamp: i64, now: DateTime<Utc>) -> Result<()> {
    let min_allowed = dotnet_ticks(now - Duration::minutes(MAX_PROOF_AGE_MINUTES));
    if timestamp < min_allowed {
        return Err(AsterError::internal_error(
            "WOPI proof timestamp is older than the allowed replay window",
        ));
    }
    Ok(())
}

fn build_expected_proof(access_token: &str, request_url: &str, timestamp: i64) -> Result<Vec<u8>> {
    // WOPI proof payload uses the uppercase request URL and network byte order
    // for both the 4-byte length prefixes and the 8-byte timestamp value.
    let upper_request_url = request_url.to_ascii_uppercase();
    let mut payload = Vec::new();
    append_len_prefixed_bytes(&mut payload, access_token.as_bytes())?;
    append_len_prefixed_bytes(&mut payload, upper_request_url.as_bytes())?;
    append_len_prefixed_bytes(&mut payload, &timestamp.to_be_bytes())?;
    Ok(payload)
}

fn append_len_prefixed_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| AsterError::internal_error("WOPI proof payload component is too large"))?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn verify_wopi_signature(
    key: &WopiProofPublicKey,
    encoded_signature: &str,
    expected_proof: &[u8],
) -> Result<bool> {
    let decoded_signature = STANDARD
        .decode(encoded_signature.trim())
        .map_err(|_| AsterError::internal_error("WOPI proof signature must be valid base64"))?;
    let signature = Signature::try_from(decoded_signature.as_slice()).map_err(|_| {
        AsterError::internal_error("WOPI proof signature is not a valid RSA PKCS#1 blob")
    })?;
    let verifying_key = VerifyingKey::<Sha256>::new(key.key.clone());
    Ok(verifying_key.verify(expected_proof, &signature).is_ok())
}

fn dotnet_ticks(value: DateTime<Utc>) -> i64 {
    value.timestamp_millis() * 10_000 + DOTNET_TICKS_AT_UNIX_EPOCH
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use chrono::{Duration, Utc};
    use rsa::{
        RsaPrivateKey,
        pkcs1v15::SigningKey,
        rand_core::OsRng,
        signature::{SignatureEncoding, Signer},
        traits::PublicKeyParts,
    };
    use sha2_10::Sha256;

    use super::{
        WopiProofKeySet, build_expected_proof, dotnet_ticks, parse_wopi_proof_key_set,
        validate_wopi_proof,
    };

    fn build_test_keys() -> (RsaPrivateKey, RsaPrivateKey, WopiProofKeySet) {
        let mut rng = OsRng;
        let current = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let old = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let proof_keys = parse_wopi_proof_key_set(
            &STANDARD.encode(current.to_public_key().n().to_bytes_be()),
            &STANDARD.encode(current.to_public_key().e().to_bytes_be()),
            Some(&STANDARD.encode(old.to_public_key().n().to_bytes_be())),
            Some(&STANDARD.encode(old.to_public_key().e().to_bytes_be())),
        )
        .unwrap();

        (current, old, proof_keys)
    }

    fn sign(private_key: &RsaPrivateKey, payload: &[u8]) -> String {
        let signing_key = SigningKey::<Sha256>::new(private_key.clone());
        STANDARD.encode(signing_key.sign(payload).to_vec())
    }

    fn build_reference_payload(access_token: &str, request_url: &str, timestamp: i64) -> Vec<u8> {
        let upper_request_url = request_url.to_ascii_uppercase();
        let mut payload = Vec::new();

        let access_token = access_token.as_bytes();
        let access_token_len = u32::try_from(access_token.len()).unwrap_or(u32::MAX);
        payload.extend_from_slice(&access_token_len.to_be_bytes());
        payload.extend_from_slice(access_token);

        let request_url = upper_request_url.as_bytes();
        let request_url_len = u32::try_from(request_url.len()).unwrap_or(u32::MAX);
        payload.extend_from_slice(&request_url_len.to_be_bytes());
        payload.extend_from_slice(request_url);

        let timestamp = timestamp.to_be_bytes();
        let timestamp_len = u32::try_from(timestamp.len()).unwrap_or(u32::MAX);
        payload.extend_from_slice(&timestamp_len.to_be_bytes());
        payload.extend_from_slice(&timestamp);

        payload
    }

    #[test]
    fn build_expected_proof_uses_network_byte_order() {
        let payload = build_expected_proof("token", "https://drive.example.com/wopi", 123).unwrap();
        assert_eq!(
            payload,
            build_reference_payload("token", "https://drive.example.com/wopi", 123)
        );
    }

    #[test]
    fn validate_wopi_proof_accepts_current_signature() {
        let (current, _old, proof_keys) = build_test_keys();
        let now = Utc::now();
        let timestamp = dotnet_ticks(now);
        let payload = build_reference_payload(
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            timestamp,
        );

        validate_wopi_proof(
            &proof_keys,
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            Some(&sign(&current, &payload)),
            None,
            Some(&timestamp.to_string()),
            now,
        )
        .unwrap();
    }

    #[test]
    fn validate_wopi_proof_accepts_old_key_rotation_window() {
        let (_current, old, proof_keys) = build_test_keys();
        let now = Utc::now();
        let timestamp = dotnet_ticks(now);
        let payload = build_reference_payload(
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            timestamp,
        );

        validate_wopi_proof(
            &proof_keys,
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            Some(&sign(&old, &payload)),
            None,
            Some(&timestamp.to_string()),
            now,
        )
        .unwrap();
    }

    #[test]
    fn validate_wopi_proof_accepts_proof_old_signed_by_current_key() {
        let (current, _old, proof_keys) = build_test_keys();
        let now = Utc::now();
        let timestamp = dotnet_ticks(now);
        let payload = build_reference_payload(
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            timestamp,
        );

        validate_wopi_proof(
            &proof_keys,
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            Some(&STANDARD.encode([0_u8; 256])),
            Some(&sign(&current, &payload)),
            Some(&timestamp.to_string()),
            now,
        )
        .unwrap();
    }

    #[test]
    fn validate_wopi_proof_rejects_stale_timestamp() {
        let (current, _old, proof_keys) = build_test_keys();
        let now = Utc::now();
        let timestamp = dotnet_ticks(now - Duration::minutes(21));
        let payload = build_reference_payload(
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            timestamp,
        );

        let err = validate_wopi_proof(
            &proof_keys,
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            Some(&sign(&current, &payload)),
            None,
            Some(&timestamp.to_string()),
            now,
        )
        .unwrap_err();

        assert!(
            err.message()
                .contains("older than the allowed replay window")
        );
    }

    #[test]
    fn parse_wopi_proof_key_set_requires_old_pairs() {
        let mut rng = OsRng;
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let err = parse_wopi_proof_key_set(
            &STANDARD.encode(key.to_public_key().n().to_bytes_be()),
            &STANDARD.encode(key.to_public_key().e().to_bytes_be()),
            Some("AQAB"),
            None,
        )
        .unwrap_err();
        assert!(err.message().contains("must be provided together"));
    }
}

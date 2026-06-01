//! WOPI proof-key 验签。
//!
//! Microsoft 365 for the web 会用 discovery 暴露的 proof-key 对每个 WOPI 请求签名。
//! 这里把 proof 组包、时间戳窗口校验和 RSA 验签集中起来，避免这些协议细节散在路由里。

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Duration, Utc};
use ring::signature::{RSA_PKCS1_2048_8192_SHA256, RsaPublicKeyComponents};

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
    modulus: Vec<u8>,
    exponent: Vec<u8>,
}

pub(crate) fn parse_wopi_proof_key_set(
    current_modulus: &str,
    current_exponent: &str,
    old_modulus: Option<&str>,
    old_exponent: Option<&str>,
) -> Result<WopiProofKeySet> {
    let current = WopiProofPublicKey {
        modulus: parse_wopi_key_component(current_modulus, "modulus")?,
        exponent: parse_wopi_key_component(current_exponent, "exponent")?,
    };
    let old = match (
        old_modulus.map(str::trim).filter(|value| !value.is_empty()),
        old_exponent
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (None, None) => None,
        (Some(modulus), Some(exponent)) => Some(WopiProofPublicKey {
            modulus: parse_wopi_key_component(modulus, "old modulus")?,
            exponent: parse_wopi_key_component(exponent, "old exponent")?,
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

fn parse_wopi_key_component(encoded: &str, name: &str) -> Result<Vec<u8>> {
    let decoded = STANDARD.decode(encoded.trim()).map_err(|_| {
        AsterError::validation_error(format!("WOPI proof-key {name} must be valid base64"))
    })?;
    let first_nonzero = decoded
        .iter()
        .position(|value| *value != 0)
        .unwrap_or(decoded.len());
    let trimmed = decoded[first_nonzero..].to_vec();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error(format!(
            "WOPI proof-key {name} must not be zero"
        )));
    }
    Ok(trimmed)
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
    let max_allowed = dotnet_ticks(now + Duration::minutes(MAX_PROOF_AGE_MINUTES));
    if timestamp < min_allowed {
        return Err(AsterError::internal_error(
            "WOPI proof timestamp is older than the allowed replay window",
        ));
    }
    if timestamp > max_allowed {
        return Err(AsterError::internal_error(
            "WOPI proof timestamp is newer than the allowed replay window",
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
    let public_key = RsaPublicKeyComponents {
        n: key.modulus.as_slice(),
        e: key.exponent.as_slice(),
    };
    Ok(public_key
        .verify(
            &RSA_PKCS1_2048_8192_SHA256,
            expected_proof,
            &decoded_signature,
        )
        .is_ok())
}

fn dotnet_ticks(value: DateTime<Utc>) -> i64 {
    value.timestamp_millis() * 10_000 + DOTNET_TICKS_AT_UNIX_EPOCH
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use chrono::{Duration, Utc};
    use ring::{
        rand::SystemRandom,
        signature::{RSA_PKCS1_SHA256, RsaKeyPair, RsaPublicKeyComponents},
    };

    use super::{
        WopiProofKeySet, build_expected_proof, dotnet_ticks, parse_wopi_proof_key_set,
        validate_wopi_proof,
    };

    const CURRENT_PRIVATE_KEY: &str = r#"
-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEAvLjzLcWtd+Rn5EKEGSEt7HsRUg6rQd9rc+CF4tPYOOMEKTyB
M5z9mWWV6+IMNDRh57E+3QAPAIthcBqEkws7aIHA3bm0owFFt1E1UGmfDoJWLJvT
oGUkuL//qWTHXdxVID5fNHSpksfPQcxoIPSqhgP3AQkzsi06taFehYSZeUBcctPm
Px0LPfamGy9yd28CtC0MpIUkpp8Y3iDfx4CyOoNxymxEIpM9O86IpiyP3jZ3qNA+
ldP4dYaWfw2aDKA+xt8VcAJ8ONHR9lZBzJxiUl3ouz/j5sgCAkQHgviPuZ9GtyXG
uqGuztg33ktjPP+Nt3hNOO6/BMM1bI1xLN5yEQIDAQABAoIBAAhyKofw4duMwE2J
4ImTX4/Gzjai620uR4vPD47gNjwNhOEnkQyzSPI1hqkg27T2Zy9MUmjnmMRIeJrg
xPAjv4vkyrHhnsDwzKLwoncv0ut+T8b9TlJOVH9kMFfvZ7C+rJydzfr2AaTNBmyG
bl6TNJJ82PAV7ldaCNeaGjXVglzXvalw3vXalh2UHXkGWxWDr9G4/y3iF52L1gDD
OH8o82cQXEm/yAbFeWtPERtkxWpsEQTOxnrqYdQeJ18tWIEQVFtGl6wCjYHYFqaw
n1xdo6Lr87oSqxNnpNFDrhHNeKlC+c5cWyA51TR3z0u/ZS7z24idcCZJebb6XDo6
Fr+3u8UCgYEA/j2OzVExgFJQ8Nu3r2QyIweMEjBbxjt2Y6xK4mvpo5VrWq5llBpj
zCzn8AJSxHQqdXBgK+6zuQuTeWQz3YIC49/NZ/2aGdcusuTdTNN8yqrX5IXOWwAJ
YRW7dLRq6fJ+vhTtOSaOS8BCRQmF6YWzvOzSbwFtpri4Ih919ZGZvRMCgYEAvgdQ
D1owrFXZov5es8/gJoMZLNOJr3LM7QDZzrcyohBimVNbdWrFYGf5U2J8l+aOod8U
DUSu48+MVttrs22mvrN9TTV3AK1vDsQHZdSjpp44XZWB1/kZNWZWLqbK4SdByFIU
z9zJKLnVP8QKQpw8JkmPMfHirT+62D8vqYi47MsCgYEA2+qBiMYvzHDnxMA5zkQc
PkK7/cvIxtsOmD8jc2Gm8rI/72ulQAvnwWgipHBOCdL2Gym+dqH+4hTKVxm+518b
guNHOSmbz7hbk7D2YAscCe7n2quHiR2p/0meIeAiDwWMbn2JiYL5WTsP18naBNp7
U/OCPzT8FVf5JsMR9P4h/vMCgYBO7dCmH8r5ucrk9Yy2WRB8TpWlVdPpiOBvTJwr
TVJ9mBqsHsBtO8Txrx4TMWQY3828lGDKxg1yWCGtbgQFCfVpXjocWKmuIVtwoaGE
/VZf/XXiARhmcXO0B2aih+rarCiZoOY+FDGFdfKKQs4ULrqZGJKepx6E4WSlL1GH
tF9DEwKBgQDnZskrCh32LTbUPIsL/0dmOb8N4/PCTQbjfYksUhATzh4P70NjI7e0
4ZhQ2zrejfnc3k+fWn2GIlkckwosLAI6f/A7ZTh8CTAPtKOXpNZAh24r5yDYRdI9
F6gxJgCziHksLP4HsC5JC5RoOoSOPQclCtnS/1wUWyArvcWzVU/JUw==
-----END RSA PRIVATE KEY-----
"#;

    const OLD_PRIVATE_KEY: &str = r#"
-----BEGIN RSA PRIVATE KEY-----
MIIEogIBAAKCAQEAvcp/R2SvETmScvR9YgyB2Y/4Vxj8+MWfYfh7RFFTWEP84vOP
PaqmLzxAwOAYZBE5/beEaGaxwro+bdXcqsCrWhTJDtAO2+6NCBXFd+FV8Y4s/Mpc
KRzEvOiquIn9gdT4QAd25HkYQ0EZrVSzFA4FKblDFmfYNCkDRANbXhtihhJTXgEx
8ffa/j5sNhit+A8f+9uF56gWGD5FVYAjIiVzFmqzD7tHENKd6rnJDYNfvPJfJYn6
vMfKC44ixsthZA00AZ5lD95hBLl1jrGkHyhsJy9iIqDrT/uQLH7WgeRRqPTkC4DM
BIMDqdb8g+rIg6qm2+jhUAQSnn8M70qfJ+GE8wIDAQABAoIBACjf7EKRfhzNE+vb
GQfdXrffCGKluJHRag6dB9tCUptfZR7xyqdC0fC5Xs7LVKV0ilNIy2T6vQ0NtHVO
Smyh+yV29YhRqemW+lvD6Jf1eV+BOdIluOyHzB1NVLtSyLzGA8MyeFojdGTDqAaL
B9hpXpZKVpcEPW2aaaAjwvFFH5Z1C28OlkdCk7f4qE1HYLk0bF9rUpjExz5GwFnE
8Rmvc6pml/eI7eHY+WddWeJnXHvhiw6tCo0HJgscksjDZn+OscHZSEQmyoQ7wEQI
B6TdBVSFawvOevopUXfO8eYTwHyhKUnArEgE/fNXBg78FMhE7y9PPRKEhaJCXsh+
PUmTmGECgYEA9VK2UrDtljP8wpgoApJOm7+rA6DHY+kqGnYWabDleAoFH/H2eXoc
TmLNulme5giX/dY7WJ9qBH1yK3UZ8TQRz5+9BxdhDpo4LRjUEOuuzolDtm1hqlUW
hE4DY+VL2x7+/SO2rHQCIYniyKil7FJ3Ym3jOoPIDoNmXUn2FpCiMeECgYEAxg0R
zkoQ6pwy6XPeAZEPuJdtD2uXDIsW/EupHevP8uOn0JTepn0T29OT2dGNPJYSfBOb
L+AMnV32IidLLMoqEm/YbFrUUJfAvukWQEm6BJv1Djw3RKsfBSfsCnB0nQT+JZuf
UYeJ45jUt8mjDT9X7s7QRoB2vftNMWvVlcC4eVMCgYAlfDP7wqkrEFqI6XMDoZN9
XPYmocSV0aTrUivujmchxnYuAWzl9vCoUZSZ6uPKxnljAf8jdYhfk0OEvGnwX0Jx
dTkPAlWEQ7Bdw7Nzum+Fg5fjIieQPVwpbzo5Y2oJ21yfFXvuMfO5aDZM7ugbiiZP
1faolEZXYWCc1JZTsFn4QQKBgEePcV+YY4Rh7ANuWkk2oPeRv1ZTCcD+gM+ohvLI
wdqBZ6F2KPz/NK25RTLvBJlfoE40x14FFonF6altiTwl0A3ZW9nK9+wm6P4SOngA
K7Z+o40BNPca3Zp/UkpzV69knm/4SxiqYKhcEIBX2xJuUNd44siWolEC/GFfFU2G
1SEBAoGAYdgiEhIdsrrK5j08Ib8uYVLYVw+dvwdK+ylBw9RwqakIkI0se2Vm2LOK
ABzR7yXWbGsEuWuFOURfwc0jHSCA/s/p0oKRTJS1xFbdO92VCBJwTvvltcjGHXre
80sGcP+jhvPhNUptnWQJP2ioMw6kjhQfMxJaMF5g/cfmRggGvMY=
-----END RSA PRIVATE KEY-----
"#;

    fn build_test_keys() -> (RsaKeyPair, RsaKeyPair, WopiProofKeySet) {
        let current = load_private_key(CURRENT_PRIVATE_KEY);
        let old = load_private_key(OLD_PRIVATE_KEY);
        let current_public = public_components(&current);
        let old_public = public_components(&old);
        let proof_keys = parse_wopi_proof_key_set(
            &STANDARD.encode(&current_public.n),
            &STANDARD.encode(&current_public.e),
            Some(&STANDARD.encode(&old_public.n)),
            Some(&STANDARD.encode(&old_public.e)),
        )
        .unwrap();

        (current, old, proof_keys)
    }

    fn load_private_key(pem: &str) -> RsaKeyPair {
        RsaKeyPair::from_der(&decode_pem(pem)).unwrap()
    }

    fn decode_pem(pem: &str) -> Vec<u8> {
        let body: String = pem
            .lines()
            .filter(|line| !line.starts_with("-----"))
            .collect();
        STANDARD.decode(body).unwrap()
    }

    fn public_components(key: &RsaKeyPair) -> RsaPublicKeyComponents<Vec<u8>> {
        RsaPublicKeyComponents::from(key.public())
    }

    fn sign(private_key: &RsaKeyPair, payload: &[u8]) -> String {
        let rng = SystemRandom::new();
        let mut signature = vec![0; private_key.public().modulus_len()];
        private_key
            .sign(&RSA_PKCS1_SHA256, &rng, payload, &mut signature)
            .unwrap();
        STANDARD.encode(signature)
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
    fn validate_wopi_proof_rejects_proof_old_signed_by_old_key() {
        let (_current, old, proof_keys) = build_test_keys();
        let now = Utc::now();
        let timestamp = dotnet_ticks(now);
        let payload = build_reference_payload(
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            timestamp,
        );

        let err = validate_wopi_proof(
            &proof_keys,
            "wopi_token",
            "https://drive.example.com/api/v1/wopi/files/7?access_token=wopi_token",
            Some(&STANDARD.encode([0_u8; 256])),
            Some(&sign(&old, &payload)),
            Some(&timestamp.to_string()),
            now,
        )
        .unwrap_err();

        assert!(err.message().contains("WOPI proof validation failed"));
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
    fn validate_wopi_proof_rejects_future_timestamp() {
        let (current, _old, proof_keys) = build_test_keys();
        let now = Utc::now();
        let timestamp = dotnet_ticks(now + Duration::minutes(21));
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
                .contains("newer than the allowed replay window")
        );
    }

    #[test]
    fn parse_wopi_proof_key_set_requires_old_pairs() {
        let err = parse_wopi_proof_key_set(
            &STANDARD.encode([1_u8; 256]),
            &STANDARD.encode([1_u8, 0, 1]),
            Some("AQAB"),
            None,
        )
        .unwrap_err();
        assert!(err.message().contains("must be provided together"));
    }
}

use anyhow::Result;
use base16ct::lower::encode_string;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

fn sha256_hex(data: &[u8]) -> String {
    encode_string(&Sha256::digest(data))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

pub(crate) struct R2SignRequest<'a> {
    pub account_id: &'a str,
    pub access_key_id: &'a str,
    pub secret_access_key: &'a str,
    pub bucket: &'a str,
    pub key: &'a str,
    pub datetime: &'a str,
}

pub(crate) struct R2SignHeaders {
    pub authorization: String,
    pub x_amz_date: String,
    pub x_amz_content_sha256: String,
}

/// Sign an S3 GET request for Cloudflare R2 using AWS Signature Version 4.
pub(crate) fn sign_r2_get(req: &R2SignRequest) -> Result<R2SignHeaders> {
    sign_r2("GET", req, &[])
}

/// Sign an S3 PUT request for Cloudflare R2 using AWS Signature Version 4.
pub(crate) fn sign_r2_put(req: &R2SignRequest, body: &[u8]) -> Result<R2SignHeaders> {
    sign_r2("PUT", req, body)
}

fn sign_r2(method: &str, req: &R2SignRequest, body: &[u8]) -> Result<R2SignHeaders> {
    anyhow::ensure!(
        req.datetime.len() >= 8,
        "datetime must be at least 8 characters (got {})",
        req.datetime.len()
    );

    let R2SignRequest {
        account_id,
        access_key_id,
        secret_access_key,
        bucket,
        key,
        datetime,
    } = req;

    let datestamp = &datetime[..8];
    let host = format!("{account_id}.r2.cloudflarestorage.com");
    let region = "auto";
    let service = "s3";

    // Step 1: Canonical request
    // https://docs.aws.amazon.com/general/latest/gr/sigv4-create-canonical-request.html
    let body_hash = sha256_hex(body);
    let canonical_request = format!(
        r"{method}
/{bucket}/{key}

host:{host}
x-amz-content-sha256:{body_hash}
x-amz-date:{datetime}

host;x-amz-content-sha256;x-amz-date
{body_hash}"
    );

    // Step 2: String to sign
    let credential_scope = format!("{datestamp}/{region}/{service}/aws4_request");
    let canonical_request_hash = sha256_hex(canonical_request.as_bytes());
    let string_to_sign = format!(
        r"AWS4-HMAC-SHA256
{datetime}
{credential_scope}
{canonical_request_hash}"
    );

    // Step 3: Signing key — derived by chaining HMAC over date/region/service
    let signing_key = {
        let k_date = hmac_sha256(
            format!("AWS4{secret_access_key}").as_bytes(),
            datestamp.as_bytes(),
        );
        let k_region = hmac_sha256(&k_date, region.as_bytes());
        let k_service = hmac_sha256(&k_region, service.as_bytes());
        hmac_sha256(&k_service, b"aws4_request")
    };

    // Step 4: Signature and Authorization header
    let signature = encode_string(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";
    let authorization = format!(
        r"AWS4-HMAC-SHA256 Credential={access_key_id}/{credential_scope},SignedHeaders={signed_headers},Signature={signature}"
    );

    Ok(R2SignHeaders {
        authorization,
        x_amz_date: datetime.to_string(),
        x_amz_content_sha256: body_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // SHA-256 of empty string
    const EMPTY_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn test_req(datetime: &str) -> R2SignRequest<'_> {
        R2SignRequest {
            account_id: "testaccount",
            access_key_id: "AKIDEXAMPLE",
            secret_access_key: "secret",
            bucket: "bucket",
            key: "fading.bundle",
            datetime,
        }
    }

    #[test]
    fn get_passthrough_date_and_empty_hash() {
        let signed = sign_r2_get(&test_req("20260424T120000Z")).unwrap();
        assert_eq!(signed.x_amz_date, "20260424T120000Z");
        assert_eq!(signed.x_amz_content_sha256, EMPTY_HASH);
    }

    #[test]
    fn get_authorization_format() {
        let signed = sign_r2_get(&test_req("20260424T120000Z")).unwrap();
        assert!(
            signed.authorization.starts_with(
                "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20260424/auto/s3/aws4_request,"
            )
        );
        assert!(
            signed
                .authorization
                .contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date,")
        );
        assert!(signed.authorization.contains("Signature="));
        // Signature is 64 lowercase hex chars
        let sig = signed.authorization.split("Signature=").nth(1).unwrap();
        assert_eq!(sig.len(), 64);
        assert!(sig.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    }

    #[test]
    fn put_hashes_body() {
        let body = b"hello";
        let signed = sign_r2_put(&test_req("20260424T120000Z"), body).unwrap();
        assert_eq!(
            signed.x_amz_content_sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_ne!(signed.x_amz_content_sha256, EMPTY_HASH);
    }

    #[test]
    fn get_and_put_produce_different_signatures() {
        let req = test_req("20260424T120000Z");
        let get = sign_r2_get(&req).unwrap();
        let put = sign_r2_put(&req, b"data").unwrap();
        assert_ne!(get.authorization, put.authorization);
    }

    #[test]
    fn short_datetime_rejected() {
        let req = R2SignRequest {
            account_id: "acc",
            access_key_id: "key",
            secret_access_key: "secret",
            bucket: "bucket",
            key: "fading.bundle",
            datetime: "short",
        };
        assert!(sign_r2_get(&req).is_err());
    }
}

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

pub(crate) struct R2SignHeaders {
    pub authorization: String,
    pub x_amz_date: String,
    pub x_amz_content_sha256: String,
}

/// Sign an S3 PUT request for Cloudflare R2 using AWS Signature Version 4.
/// `datetime` must be in the format `"20260410T120000Z"`.
pub(crate) fn sign_r2_put(
    account_id: &str,
    access_key_id: &str,
    secret_access_key: &str,
    bucket: &str,
    key: &str,
    body: &[u8],
    datetime: &str,
) -> R2SignHeaders {
    let datestamp = &datetime[..8]; // "20260410"
    let host = format!("{account_id}.r2.cloudflarestorage.com");
    let region = "auto";
    let service = "s3";

    // Step 1: Canonical request
    // https://docs.aws.amazon.com/general/latest/gr/sigv4-create-canonical-request.html
    let body_hash = sha256_hex(body);
    let canonical_request = format!(
        r"PUT
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
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key_id}/{credential_scope},SignedHeaders={signed_headers},Signature={signature}"
    );

    R2SignHeaders {
        authorization,
        x_amz_date: datetime.to_string(),
        x_amz_content_sha256: body_hash,
    }
}

//! Amazon SES over SMTP. SES's SMTP interface takes the IAM access key id as the user name, and a password that
//! is *derived* from the IAM secret key and the region (the secret key itself is not accepted). This is AWS's
//! documented derivation, so no separate "SMTP credentials" need to be created for an IAM user that may send.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;

fn sign(key: &[u8], message: &str) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// The SMTP password for an IAM secret access key in a region.
pub fn smtp_password(secret_access_key: &str, region: &str) -> String {
    let mut signature = sign(format!("AWS4{secret_access_key}").as_bytes(), "11111111");
    for part in [region, "ses", "aws4_request", "SendRawEmail"] {
        signature = sign(&signature, part);
    }
    let mut with_version = vec![0x04];
    with_version.extend(signature);
    STANDARD.encode(with_version)
}

/// `email-smtp.<region>.amazonaws.com`.
pub fn smtp_host(region: &str) -> String {
    format!("email-smtp.{region}.amazonaws.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_password_matches_awss_documented_derivation() {
        // Expected values were produced by AWS's own sample script (Python, hmac + sha256).
        assert_eq!(
            smtp_password("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY", "us-east-1"),
            "BLBM/9hSUELfq8Gw+rU1YcBjkOxGbhT2XG763xVLGWL9"
        );
        assert_eq!(
            smtp_password("secret", "ap-southeast-1"),
            "BAXrMRsGpOLvGtUdNtUIrRXl88mqW9rfYvwNGbVOd49r"
        );
        assert_ne!(
            smtp_password("secret", "us-east-1"),
            smtp_password("secret", "eu-west-1"),
            "the region is part of it"
        );
        assert_eq!(
            smtp_host("ap-southeast-1"),
            "email-smtp.ap-southeast-1.amazonaws.com"
        );
    }
}

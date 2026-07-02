use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use miniserde::Deserialize;
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::RandomizedSigner;
use rsa::signature::SignatureEncoding;
use sha2::Sha256;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::rng::UrandomRng;

#[derive(Debug, Deserialize)]
pub struct ServiceAccount {
    pub private_key: String,
    pub private_key_id: String,
    pub client_email: String,
    pub project_id: String,
}

impl ServiceAccount {
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read credentials {}: {e}", path.display()))?;
        let sa: ServiceAccount = miniserde::json::from_str(&text)
            .map_err(|e| format!("invalid service account JSON: {e}"))?;
        Ok(sa)
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<u64>,
}

pub struct TokenProvider {
    signing_key: SigningKey<Sha256>,
    private_key_id: String,
    client_email: String,
    project_id: String,
    cached: Mutex<Option<Cached>>,
}

struct Cached {
    token: String,
    expires_at: u64,
}

impl TokenProvider {
    pub fn new(sa: ServiceAccount) -> Result<Self, Box<dyn std::error::Error>> {
        let key = RsaPrivateKey::from_pkcs8_pem(&sa.private_key)
            .map_err(|e| format!("invalid RSA private key: {e}"))?;
        Ok(Self {
            signing_key: SigningKey::<Sha256>::new(key),
            private_key_id: sa.private_key_id,
            client_email: sa.client_email,
            project_id: sa.project_id,
            cached: Mutex::new(None),
        })
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn access_token(&self, agent: &ureq::Agent) -> Result<String, Box<dyn std::error::Error>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Some(c) = self.cached.lock().unwrap().as_ref() {
            if c.expires_at > now + 60 {
                return Ok(c.token.clone());
            }
        }

        let iat = now;
        let exp = iat + 3600;
        let header = format!(
            r#"{{"alg":"RS256","typ":"JWT","kid":"{}"}}"#,
            self.private_key_id
        );
        let claims = format!(
            r#"{{"iss":"{}","scope":"https://www.googleapis.com/auth/cloud-platform","aud":"https://oauth2.googleapis.com/token","iat":{},"exp":{}}}"#,
            self.client_email, iat, exp
        );
        let h = URL_SAFE_NO_PAD.encode(&header);
        let c = URL_SAFE_NO_PAD.encode(&claims);
        let signing_input = format!("{h}.{c}");

        // Randomized (blinded) RS256 signing to mitigate timing side-channels
        // such as the Marvin Attack. The blinding entropy comes from our
        // /dev/urandom-backed RNG. See RUSTSEC-2023-0071.
        let mut rng = UrandomRng::new()?;
        let signature = self
            .signing_key
            .try_sign_with_rng(&mut rng, signing_input.as_bytes())?;
        let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
        let assertion = format!("{signing_input}.{sig_b64}");

        let body = format!(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={assertion}"
        );
        let resp = agent
            .post("https://oauth2.googleapis.com/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send(body)?
            .body_mut()
            .read_to_string()?;

        let parsed: TokenResponse = miniserde::json::from_str(&resp)
            .map_err(|e| format!("token response parse: {e}: {resp}"))?;
        let expires_in = parsed.expires_in.unwrap_or(3600);
        let expires_at = now + expires_in;

        *self.cached.lock().unwrap() = Some(Cached {
            token: parsed.access_token.clone(),
            expires_at,
        });
        log::debug!("refreshed access token, expires_at={expires_at}");
        Ok(parsed.access_token)
    }
}

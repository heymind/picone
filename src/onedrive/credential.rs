use std::env;
use std::ops::Add;
use std::time::{Duration, SystemTime};
use std::sync::RwLock;

pub struct Credential {
    pub client_id: String,
    pub client_secret: String,
    pub access_token: String,
    pub refresh_token: String,
    pub redirect_uri: String,
    expires_at: SystemTime,
}


impl Credential {
    pub fn new(client_id: String, client_secret: String, access_token: String, refresh_token: String, redirect_uri: String) -> Self {
        Credential { client_id, client_secret, access_token, refresh_token, expires_at: SystemTime::now(), redirect_uri }
    }
    pub fn is_expired(&self) -> bool { self.expires_at < SystemTime::now() }
    pub fn from_env() -> Option<Self> {
        Some(Credential {
            client_id: env::var("CLIENT_ID").ok()?,
            client_secret: env::var("CLIENT_SECRET").ok()?,
            access_token: env::var("ACCESS_TOKEN").ok()?,
            refresh_token: env::var("REFRESH_TOKEN").ok()?,
            redirect_uri: env::var("REDIRECT_URI").ok()?,
            expires_at: SystemTime::now(),
        })
    }
    pub fn refresh(&mut self, access_token: &str, expires_in: u64) {
        self.access_token = access_token.to_owned();
        self.expires_at = SystemTime::now().add(Duration::from_secs(expires_in));
    }

}

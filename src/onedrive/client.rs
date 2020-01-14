use std::sync::{Arc, RwLock};

use anyhow::{Error, Result};
use reqwest::Body;
use reqwest::header;
use serde::Deserialize;

use super::Credential;

pub struct Client {
    client: reqwest::blocking::Client,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub display_name: String,
    pub id: String,
    pub user_principal_name: String,
}


const OAUTH_TOKEN_ENDPOINT: &'static str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const ME_ENDPOINT: &'static str = "https://graph.microsoft.com/v1.0/me/";

//const CREATE_UPLOAD_SESSION:&'static str = ;
impl Client {
    pub fn refresh(credential: &mut Credential) -> Result<()> {
        let client: reqwest::blocking::Client = reqwest::blocking::Client::new();
        let resp = client.post(OAUTH_TOKEN_ENDPOINT).form(&(
            ("client_id", &credential.client_id),
            ("redirect_uri", &credential.redirect_uri),
            ("client_secret", &credential.client_secret),
            ("refresh_token", &credential.refresh_token),
            ("grant_type", "refresh_token")
        )).send()?;
        if resp.status().is_success() {
            let data: serde_json::Value = resp.json()?;
            info!("new token {}", &data["access_token"]);
            credential.refresh(data["access_token"].as_str().unwrap(), data["expires_in"].as_u64().unwrap());
            Ok(())
        } else { Err(anyhow!("Error {}",resp.text()?)) }
    }
    pub fn new_and_refresh(cred: &RwLock<Credential>) -> Result<Self> {
        let cred_read = cred.read().unwrap();
        if cred_read.is_expired() {
            drop(cred_read);
            let mut cred = cred.write().unwrap();
            Self::refresh(&mut cred)?;
            Ok(Self::new(&cred)?)
        } else {
            Ok(Self::new(&cred_read)?)
        }
    }
    pub fn new(credential: &Credential) -> Result<Self> {
        if credential.is_expired() {
            return Err(anyhow!("credential expired"));
        }
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, header::HeaderValue::from_str(&format!("bearer {}", credential.access_token))?);

        Ok(Client {
            client: reqwest::blocking::Client::builder()
                .default_headers(headers)
                .build()?
        })
    }
    pub fn get_file_url(&self, id_or_path: &str) -> Result<Option<String>> {
        let resp = self.client.get(&format!("https://graph.microsoft.com/v1.0/me/drive/special/approot:/{}", id_or_path)).send()?;
        let data: serde_json::Value = resp.json()?;
        Ok(data["@microsoft.graph.downloadUrl"].as_str().map(|s|s.to_owned()))
    }
    // upload -> file id
    pub fn upload(&self, content: Vec<u8>, file_path: &str) -> Result<(String, String)> {
        let upload_session_resp = self.client.post(&format!("https://graph.microsoft.com/v1.0/me/drive/special/approot:/{}:/createUploadSession", file_path)).body("").send()?;
        if upload_session_resp.status().is_success() {
            let data: serde_json::Value = upload_session_resp.json()?;
            let upload_url: &str = &data["uploadUrl"].as_str().ok_or(anyhow!("can not fetch uploadUrl"))?;
            let client: reqwest::blocking::Client = reqwest::blocking::Client::builder()
                .build()?;
            let resp = client.put(upload_url).body(content).send()?;
            let data: serde_json::Value = resp.json()?;
            info!("put file {}", &data);
            Ok((data["id"].as_str().ok_or(anyhow!("can not get file id"))?.to_owned(), format!("/{}", file_path)))
        } else {
            Err(anyhow!("req upload_session fail {:?}",upload_session_resp.text()?))
        }
    }
    pub fn me(&self) -> Result<()> {
        let resp = self.client.get(ME_ENDPOINT).send()?;
//        info!("Result {:?}", resp.text()?);
        let user = resp.json::<User>()?;
        info!("Result {:?}", user);
        Ok(())
    }
}

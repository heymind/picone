#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate anyhow;
extern crate dotenv;
#[macro_use]
extern crate log;
extern crate multipart;
extern crate reqwest;
#[macro_use]
extern crate rocket;
extern crate serde_json;

use std::io::{BufReader, Cursor, Read};
use std::sync::{Arc, RwLock};

use anyhow::private::kind::TraitKind;
use anyhow::Result;
use dotenv::dotenv;
use multipart::server::{Multipart, ReadEntryResult};
use rocket::{Data, Request, Response, State};
use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket_contrib::json::Json;
use serde::Serialize;

use crate::onedrive::{Client, Credential};

mod onedrive;
mod util;

#[derive(Debug)]
pub struct Error(anyhow::Error);

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error(e)
    }
}

impl<'r> Responder<'r> for Error {
    fn respond_to(self, _: &Request) -> Result<Response<'r>, Status> {
        Response::build().header(ContentType::Plain).sized_body(Cursor::new(self.0.to_string())).ok()
    }
}


#[get("/<file>")]
fn index(credential: State<RwLock<Credential>>, file: String) -> Result<Response> {
    let client = Client::new_and_refresh(&credential)?;
    let url = client.get_file_url(&file)?;
    if let Some(url) = url {
        let mut resp = reqwest::blocking::get(&url)?;
        let mut buf: Vec<u8> = Vec::new();
        resp.read_to_end(&mut buf)?;
        let content_type = resp.headers()[reqwest::header::CONTENT_TYPE].to_str()?.to_owned();
        Ok(Response::build()
            .status(Status::Ok)
            .raw_header("Cache-Control", "public, max-age=31536000")
            .raw_header("Content-Type", content_type)
            .sized_body(Cursor::new(buf))
            .finalize())
    } else {
        Ok(Response::build().status(Status::NotFound).finalize())
    }
}

#[derive(Debug, Serialize)]
struct UploadResult {
    id: String,
    url: String,
}

#[post("/upload", data = "<data>")]
fn upload(credential: State<RwLock<Credential>>, cont_type: &ContentType, data: Data) -> Result<Json<UploadResult>> {
    let client = Client::new_and_refresh(&credential)?;

    if cont_type.is_form_data() {
        let (_, boundary) = cont_type.params().find(|&(k, _)| k == "boundary")
            .ok_or(anyhow!("multipart/form-data boundary not found"))?;
        let mut multipart = Multipart::with_body(data.open(), boundary);
        while let Ok(Some(mut entry)) = multipart.read_entry() {
            let mut buf = Vec::new();
            if &*(entry.headers.name) == "file" {
                let name = if let Some(filename) = entry.headers.filename {
                    format!("{}_{}", util::random_id(), filename)
                } else if let Some(mime) = entry.headers.content_type {
                    let sub: &str = mime.1.as_str();
                    let ext = if sub.starts_with("svg") { "svg" } else if sub == "x-icon" { "ico" } else { sub };
                    format!("{}.{}", util::random_id(), ext)
                } else {
                    return Err(anyhow!("neither filename or mimetype provided."));
                };
                entry.data.read_to_end(&mut buf)?;
                let (id, path) = client.upload(buf, &name)?;
                return Ok(Json(UploadResult {
                    id,
                    url: path,
                }));
            }
        }
        return Err(anyhow!("form field named 'file' does not exist"));
    } else if cont_type.top() == "image" {
        let ext = if cont_type.is_svg() { "svg" } else if cont_type.is_icon() { "ico" } else { cont_type.sub().as_str() };
        let mut stream = data.open();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        let (id, path) = client.upload(buf, &format!("{}.{}", util::random_id(), ext))?;
        return Ok(Json(UploadResult {
            id,
            url: path,
        }));
    } else {
        return Err(anyhow!("unsupported content-type"));
    }
}

fn main() {
    dotenv().ok();
    env_logger::init();
    info!("starting up");
    let mut cred = onedrive::Credential::from_env().unwrap();
    rocket::ignite().manage(RwLock::new(cred)).mount("/", routes![index,upload]).launch();
}
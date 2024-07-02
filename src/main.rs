pub mod auth;
pub mod err_handlers;

use crate::{auth::fetch_auth, err_handlers::error_handlers};

use actix_web::{
    error, get, http::StatusCode, middleware, web, App, Error, HttpResponse, HttpServer, Result,
};
use anyhow::anyhow;
use parse_sap_atom_feed::{
    atom::{feed::Feed, AtomService},
    odata_error::ODataError,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    io::{self, BufRead},
    path::Path,
    str::{self, FromStr},
    sync::Mutex,
};
use tinytemplate::TinyTemplate;

include!(concat!(env!("OUT_DIR"), "/catalogservice.rs"));

use catalogservice::*;

static INDEX: &str = include_str!("../html/index.html");
static CATALOGSERVICE_VARNAME: &[u8] = "SAP_CATALOGSERVICE_HOSTNAME".as_bytes();
static HOST_PATH: &[u8] = "/sap/opu/odata/iwfnd".as_bytes();
static SERVICE_NAME: &[u8] = "catalogservice;v=2".as_bytes();

// ---------------------------------------------------------------------------------------------------------------------
pub fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(BufReader::new(file).lines())
}

// ---------------------------------------------------------------------------------------------------------------------
fn fetch_env_var(varname: &str) -> Result<String, String> {
    let mut value = String::from("unknown");

    // Try to obtain the environment variable file .env
    if let Ok(lines) = read_lines(".env") {
        for line in lines {
            match line {
                Ok(l) => {
                    if l.starts_with(varname) {
                        let (_, u) = l.split_at(l.find("=").unwrap() + 1);
                        value = u.to_owned();
                    }
                }
                Err(_) => (),
            }
        }
    }

    if value.eq("unknown") {
        Err(format!("No value for {} found in .env file", varname))
    } else {
        Ok(value)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
#[derive(Serialize, Debug)]
struct AppState {
    hostname: String,
    catalog_list: Mutex<Option<Vec<String>>>,
    service_list: Mutex<Option<Vec<(String, String)>>>,
    error_msg: Mutex<Option<String>>,
    last_srv: Mutex<Option<String>>,
}

// ---------------------------------------------------------------------------------------------------------------------
// Start web server
// ---------------------------------------------------------------------------------------------------------------------
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let hostname = match fetch_env_var(str::from_utf8(CATALOGSERVICE_VARNAME).unwrap()) {
        Ok(value) => value,
        Err(err_msg) => {
            log::error!("{err_msg}");
            std::process::exit(0x01);
        }
    };

    log::info!("SAP CatalogService hostname = {}", hostname);
    log::info!("Starting HTTP server at http://localhost:8080");

    // Initial app state
    let app_state = web::Data::new(AppState {
        hostname: hostname,
        catalog_list: Mutex::new(None),
        service_list: Mutex::new(None),
        error_msg: Mutex::new(None),
        last_srv: Mutex::new(None),
    });

    HttpServer::new(move || {
        let mut tt = TinyTemplate::<'_>::new();

        tt.add_template("index.html", INDEX).unwrap();

        App::new()
            .app_data(app_state.clone())
            .app_data(web::Data::new(tt))
            .wrap(middleware::Logger::default())
            .service(web::resource("/").route(web::get().to(doc_root)))
            .service(catalog_services)
            .service(fetch_metadata)
            .service(web::scope("").wrap(error_handlers()))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

// ---------------------------------------------------------------------------------------------------------------------
// Serve document root
// ---------------------------------------------------------------------------------------------------------------------
async fn doc_root(
    app_state: web::Data<AppState>,
    tmpl: web::Data<TinyTemplate<'_>>,
    _query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    log::info!("---> doc_root()");
    let srv_doc_url = format!(
        "https://{}/{}/{}/",
        app_state.hostname,
        str::from_utf8(HOST_PATH).unwrap(),
        str::from_utf8(SERVICE_NAME).unwrap()
    );

    *app_state.service_list.lock().unwrap() = None;
    *app_state.error_msg.lock().unwrap() = None;
    *app_state.last_srv.lock().unwrap() = None;

    // Read service document
    log::info!("     Fetching CatalogService service document");
    let srv_doc = match fetch_odata_service_doc(&srv_doc_url).await {
        Ok(srv_doc) => srv_doc,
        Err(err) => {
            *app_state.error_msg.lock().unwrap() = Some(format!("{}", err));
            log::error!("<--- doc_root() ERROR");
            return Ok(build_http_response(
                app_state,
                StatusCode::INTERNAL_SERVER_ERROR,
                tmpl,
            ));
        }
    };

    // From the service document, extract CatalogCollection URL
    let catalog_collection = match srv_doc
        .workspace
        .collections
        .into_iter()
        .find(|c| c.href == "CatalogCollection")
    {
        Some(cat_coll) => cat_coll,
        None => {
            *app_state.error_msg.lock().unwrap() = Some(format!("That's weird, the CatalogService does not have a collection called CatalogCollection"));
            log::error!("<--- doc_root() ERROR");
            return Ok(build_http_response(
                app_state,
                StatusCode::INTERNAL_SERVER_ERROR,
                tmpl,
            ));
        }
    };

    // Read the available catalogs
    log::info!("     Fetching CatalogService catalogs");
    let feed_url = format!("{}{}", srv_doc_url, catalog_collection.href);
    let catalog_feed = match fetch_feed::<Catalog>(&feed_url).await {
        Ok(feed) => feed,
        Err(err) => {
            *app_state.error_msg.lock().unwrap() = Some(err.to_string());
            log::error!("<--- doc_root() ERROR");
            return Ok(build_http_response(
                app_state,
                StatusCode::INTERNAL_SERVER_ERROR,
                tmpl,
            ));
        }
    };

    // From the catalog feed, extract the list of available Catalog names
    if catalog_feed.entries.is_none() {
        *app_state.error_msg.lock().unwrap() = Some(format!(
            "No service catalogs have been defined: {}",
            catalog_feed.id
        ));
        log::error!("<--- doc_root() ERROR");

        return Ok(build_http_response(
            app_state,
            StatusCode::INTERNAL_SERVER_ERROR,
            tmpl,
        ));
    }

    let mut catalog_list: Vec<String> = Vec::new();
    catalog_feed.entries.unwrap().into_iter().for_each(|c| {
        catalog_list.push(c.content.properties.unwrap().title);
    });

    *app_state.catalog_list.lock().unwrap() = Some(catalog_list);

    log::info!("<--- doc_root()");
    Ok(build_http_response(app_state, StatusCode::OK, tmpl))
}

// ---------------------------------------------------------------------------------------------------------------------
// Display OData services in selected catalog
// ---------------------------------------------------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
pub struct FetchServicesQS {
    catalog_name: String,
}

#[get("/fetchServices")]
async fn catalog_services<'template>(
    qs: web::Query<FetchServicesQS>,
    app_state: web::Data<AppState>,
    tmpl: web::Data<TinyTemplate<'template>>,
) -> Result<HttpResponse, Error> {
    log::info!("---> catalog_services()");
    let services_url = format!(
        "https://{}/{}/{}/CatalogCollection('{}')/Services",
        app_state.hostname,
        str::from_utf8(HOST_PATH).unwrap(),
        str::from_utf8(SERVICE_NAME).unwrap(),
        qs.catalog_name
    );

    // Read services in selected catalog
    log::info!("     Fetching services in catalog {}", qs.catalog_name);
    let services_feed = match fetch_feed::<Service>(&services_url).await {
        Ok(feed) => feed,
        Err(e) => {
            *app_state.error_msg.lock().unwrap() = Some(format!(
                "{}\nAn error occurred trying to read the Services in catalog {}",
                e, qs.catalog_name
            ));
            log::error!("<--- catalog_services() ERROR");
            return Ok(build_http_response(
                app_state,
                StatusCode::INTERNAL_SERVER_ERROR,
                tmpl,
            ));
        }
    };

    // Build service list
    if services_feed.entries.is_none() {
        *app_state.error_msg.lock().unwrap() =
            Some(format!("No services found: {}", services_feed.id));
        log::error!("<--- catalog_services() ERROR");
        return Ok(build_http_response(
            app_state,
            StatusCode::INTERNAL_SERVER_ERROR,
            tmpl,
        ));
    }

    let mut service_list: Vec<(String, String)> = Vec::new();
    services_feed.entries.unwrap().into_iter().for_each(|srv| {
        let props = srv.content.properties.unwrap().clone();
        service_list.push((props.id, props.metadata_url));
    });

    service_list.sort_by(|a, b| a.0.cmp(&b.0));

    let first_srv = service_list[0].1.clone();

    *app_state.last_srv.lock().unwrap() = Some(first_srv);
    *app_state.service_list.lock().unwrap() = Some(service_list);
    log::info!("<--- catalog_services()");

    return Ok(build_http_response(app_state, StatusCode::OK, tmpl));
}

// ---------------------------------------------------------------------------------------------------------------------
// Fetch service metadata
// ---------------------------------------------------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
pub struct FetchMetadataQS {
    url: String,
}

#[get("/fetchMetadata")]
async fn fetch_metadata<'template>(
    qs: web::Query<FetchMetadataQS>,
    app_state: web::Data<AppState>,
    tmpl: web::Data<TinyTemplate<'template>>,
) -> Result<HttpResponse, Error> {
    log::info!("---> fetch_metadata()");
    *app_state.last_srv.lock().unwrap() = Some(qs.url.clone());

    let auth_chars = match fetch_auth() {
        Ok(auth_chars) => auth_chars,
        Err(err) => {
            *app_state.error_msg.lock().unwrap() = Some(err);
            log::error!("<--- fetch_metadata() ERROR");
            return Ok(build_http_response(
                app_state,
                StatusCode::INTERNAL_SERVER_ERROR,
                tmpl,
            ));
        }
    };

    log::info!("GET: {}", qs.url);

    let client = reqwest::Client::new();
    let response = match client
        .get(qs.url.clone())
        .header("Authorization", format!("Basic {}", auth_chars))
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            *app_state.error_msg.lock().unwrap() = Some(err.to_string());
            log::error!("<--- fetch_metadata() ERROR");
            return Ok(build_http_response(
                app_state,
                StatusCode::from_u16(err.status().unwrap().as_u16()).unwrap(),
                tmpl,
            ));
        }
    };

    let http_status_code = StatusCode::from_u16(response.status().as_u16()).unwrap();
    log::info!("HTTP Status code = {}", http_status_code);

    let raw_xml = response.text().await.unwrap();

    match http_status_code {
        StatusCode::OK => {
            *app_state.error_msg.lock().unwrap() = None;
            log::info!("<--- fetch_metadata()");
            // Dump the raw XML on the client
            Ok(HttpResponse::build(http_status_code)
                .content_type("text/plain")
                .body(raw_xml))
        }
        StatusCode::UNAUTHORIZED => {
            *app_state.error_msg.lock().unwrap() = Some("Logon failed".to_owned());
            log::error!("<--- fetch_metadata() ERROR");
            Ok(build_http_response(app_state, http_status_code, tmpl))
        }
        StatusCode::NOT_FOUND => {
            *app_state.error_msg.lock().unwrap() = Some("Service not found.  This may be because the service has been defined, but not activated.".to_owned());
            log::error!("<--- fetch_metadata() ERROR");
            Ok(build_http_response(app_state, http_status_code, tmpl))
        }
        StatusCode::INTERNAL_SERVER_ERROR => {
            *app_state.error_msg.lock().unwrap() = Some(parse_odata_error(&raw_xml));
            log::error!("<--- fetch_metadata() ERROR");
            Ok(build_http_response(app_state, http_status_code, tmpl))
        }
        _ => {
            *app_state.error_msg.lock().unwrap() = Some(raw_xml);
            log::error!("<--- fetch_metadata() ERROR");
            Ok(build_http_response(app_state, http_status_code, tmpl))
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------------------------------------------------
async fn fetch_feed<T>(feed_url: &str) -> Result<Feed<T>, anyhow::Error>
where
    T: DeserializeOwned,
{
    log::info!("---> fetch_feed<T>()");
    let client = reqwest::Client::new();

    let auth_chars = match fetch_auth() {
        Ok(auth_chars) => auth_chars,
        Err(e) => return Err(anyhow!(e)),
    };

    log::info!("GET: {}", feed_url);

    let response = match client
        .get(feed_url)
        .header("Authorization", format!("Basic {}", auth_chars))
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            log::error!("<--- fetch_feed<T>() ERROR in HTTP Request");
            return Err(anyhow!(e));
        }
    };

    let http_status_code = response.status();
    log::info!("HTTP Status code = {}", http_status_code);

    let raw_xml = response.text().await.unwrap();

    match http_status_code {
        reqwest::StatusCode::OK => match Feed::<T>::from_str(&raw_xml) {
            Ok(feed) => {
                log::info!("<--- fetch_feed<T>()");
                Ok(feed)
            }
            Err(e) => {
                log::error!("<--- fetch_feed<T>() ERROR in XML deserialization");
                Err(anyhow!(e))
            }
        },
        _ => {
            log::error!("<--- fetch_feed<T>() ERROR {}", http_status_code);
            Err(anyhow!(parse_odata_error(&raw_xml)))
        }
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
async fn fetch_odata_service_doc(srv_doc_url: &str) -> Result<AtomService, anyhow::Error> {
    log::info!("---> fetch_odata_service_doc()");
    let client = reqwest::Client::new();

    let auth_chars = match fetch_auth() {
        Ok(auth_chars) => auth_chars,
        Err(e) => {
            log::info!("<--- fetch_odata_service_doc()");
            return Err(anyhow!(e));
        }
    };

    log::info!("GET: {}", srv_doc_url);

    let response = match client
        .get(srv_doc_url)
        .header("Authorization", format!("Basic {}", auth_chars))
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            log::info!("<--- fetch_odata_service_doc()");
            return Err(anyhow!(e));
        }
    };

    let http_status_code = response.status();
    log::info!("HTTP Status code = {}", http_status_code);

    let raw_xml = response.text().await.unwrap();

    log::info!("<--- fetch_odata_service_doc()");
    match http_status_code {
        reqwest::StatusCode::OK => match AtomService::from_str(&raw_xml) {
            Ok(srv_doc) => Ok(srv_doc),
            Err(err) => Err(anyhow!(err)),
        },
        _ => Err(anyhow!(parse_odata_error(&raw_xml))),
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
fn parse_odata_error(raw_xml: &str) -> String {
    match ODataError::from_str(&raw_xml) {
        Ok(odata_error) => format!("{:#?}", odata_error.message),
        Err(err) => format!("{err:#?}"),
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
fn build_http_response<'template>(
    app_state: web::Data<AppState>,
    status_code: StatusCode,
    tmpl: web::Data<TinyTemplate<'template>>,
) -> HttpResponse {
    let response_body = tmpl
        .render(
            "index.html",
            &json!({
              "hostName": app_state.hostname,
              "catalogList": *app_state.catalog_list.lock().unwrap(),
              "serviceList": *app_state.service_list.lock().unwrap(),
              "errMsg": *app_state.error_msg.lock().unwrap(),
              "lastSrv": *app_state.last_srv.lock().unwrap()
            }),
        )
        .map_err(|err| error::ErrorInternalServerError(format!("Template error\n{}", err)))
        .unwrap();

    HttpResponse::build(status_code)
        .content_type("text/html; charset=utf-8")
        .body(response_body)
}

// ---------------------------------------------------------------------------------------------------------------------
#[cfg(test)]
pub mod unit_tests;

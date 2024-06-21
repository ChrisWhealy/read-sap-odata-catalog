pub mod auth;
pub mod err_handlers;

use crate::{auth::fetch_auth, err_handlers::error_handlers};
use anyhow::anyhow;
use parse_sap_atom_feed::{
    atom::{feed::Feed, AtomService},
    odata_error::ODataError,
};

use actix_web::{
    error, get, http::StatusCode, middleware, web, App, Error, HttpResponse, HttpServer, Result,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    str::{self, FromStr},
};
use tinytemplate::TinyTemplate;

include!(concat!(env!("OUT_DIR"), "/catalogservice.rs"));

use catalogservice::*;

static INDEX: &str = include_str!("../html/index.html");
static ERROR: &str = include_str!("../html/error.html");
static HOST_NAME: &[u8] = "sapes5.sapdevcenter.com".as_bytes();
static HOST_PATH: &[u8] = "/sap/opu/odata/iwfnd".as_bytes();
static SERVICE_NAME: &[u8] = "catalogservice;v=2".as_bytes();

// ---------------------------------------------------------------------------------------------------------------------
// Start web server
// ---------------------------------------------------------------------------------------------------------------------
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    log::info!("Starting HTTP server at http://localhost:8080");

    HttpServer::new(|| {
        let mut tt = TinyTemplate::new();
        tt.add_template("index.html", INDEX).unwrap();
        tt.add_template("error.html", ERROR).unwrap();

        App::new()
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
    tmpl: web::Data<TinyTemplate<'_>>,
    _query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let srv_doc_url = format!(
        "https://{}/{}/{}/",
        str::from_utf8(HOST_NAME).unwrap(),
        str::from_utf8(HOST_PATH).unwrap(),
        str::from_utf8(SERVICE_NAME).unwrap()
    );
    log::info!("Fetching service document");

    // Read service document
    let srv_doc = match fetch_odata_service_doc(&srv_doc_url).await {
        Ok(srv_doc) => srv_doc,
        Err(e) => {
            let ctx = set_context(
              str::from_utf8(HOST_NAME).unwrap(),
              None, None,
              Some(format!("{}\nThat's weird, the OData service CatalogService does not have a collection called CatalogCollection",e).as_ref()));

            return Ok(build_http_response(StatusCode::OK, tmpl, ctx, true));
        }
    };

    // From service document, extract CatalogCollection URL
    let catalog_collection = match srv_doc
        .workspace
        .collections
        .into_iter()
        .find(|c| c.href == "CatalogCollection")
    {
        Some(cat_coll) => cat_coll,
        None => {
            let ctx = set_context(str::from_utf8(HOST_NAME).unwrap(), None, None,
            Some("That's weird, the OData service CatalogService does not have a collection called CatalogCollection"));

            return Ok(build_http_response(StatusCode::OK, tmpl, ctx, true));
        }
    };

    // Read the available catalogs
    let feed_url = format!("{}{}", srv_doc_url, catalog_collection.href);
    let catalog_feed = match fetch_feed::<Catalog>(&feed_url).await {
        Ok(feed) => feed,
        Err(e) => {
            let ctx = set_context(
                str::from_utf8(HOST_NAME).unwrap(),
                None,
                None,
                Some(
                    format!(
                        "{}\nAn error occurred trying to read the CatalogCollection {}",
                        e, feed_url
                    )
                    .as_ref(),
                ),
            );

            return Ok(build_http_response(StatusCode::OK, tmpl, ctx, true));
        }
    };

    // From catalog feed, extract list of Catalog names
    if catalog_feed.entries.is_none() {
        let ctx = set_context(
            str::from_utf8(HOST_NAME).unwrap(),
            None,
            None,
            Some(format!("No service catalogs have been defined: {}", catalog_feed.id).as_ref()),
        );

        return Ok(build_http_response(StatusCode::OK, tmpl, ctx, true));
    }

    let mut catalog_list: Vec<String> = Vec::new();
    catalog_feed.entries.unwrap().into_iter().for_each(|c| {
        catalog_list.push(c.content.properties.unwrap().title);
    });

    let ctx = set_context(
        str::from_utf8(HOST_NAME).unwrap(),
        Some(catalog_list),
        None,
        None,
    );

    Ok(build_http_response(StatusCode::OK, tmpl, ctx, false))
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
    tmpl: web::Data<TinyTemplate<'template>>,
) -> Result<HttpResponse, Error> {
    log::info!("---> catalog_services()");
    let services_url = format!(
        "https://{}/{}/{}/CatalogCollection('{}')/Services",
        str::from_utf8(HOST_NAME).unwrap(),
        str::from_utf8(HOST_PATH).unwrap(),
        str::from_utf8(SERVICE_NAME).unwrap(),
        qs.catalog_name
    );

    // Read services in selected catalog
    let services_feed = match fetch_feed::<Service>(&services_url).await {
        Ok(feed) => feed,
        Err(e) => {
            let ctx = set_context(
                str::from_utf8(HOST_NAME).unwrap(),
                None,
                None,
                Some(
                    format!(
                        "{}\nAn error occurred trying to read the Services in catalog {}",
                        e, qs.catalog_name
                    )
                    .as_ref(),
                ),
            );

            log::error!("<--- catalog_services() ERROR");
            return Ok(build_http_response(StatusCode::OK, tmpl, ctx, true));
        }
    };

    // Build service list
    if services_feed.entries.is_none() {
        let ctx = set_context(
            str::from_utf8(HOST_NAME).unwrap(),
            None,
            None,
            Some(format!("No services found: {}", services_feed.id).as_ref()),
        );

        log::error!("<--- catalog_services() ERROR");
        return Ok(build_http_response(StatusCode::OK, tmpl, ctx, true));
    }

    let mut service_list: Vec<(String, String)> = Vec::new();
    services_feed.entries.unwrap().into_iter().for_each(|s| {
        service_list.push({
            let props = s.content.properties.unwrap().clone();
            (props.id, props.metadata_url)
        })
    });

    let ctx = set_context(
        str::from_utf8(HOST_NAME).unwrap(),
        None,
        Some(service_list),
        None,
    );

    log::info!("<--- catalog_services()");
    return Ok(build_http_response(StatusCode::OK, tmpl, ctx, false));
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
    tmpl: web::Data<TinyTemplate<'template>>,
) -> Result<HttpResponse, Error> {
    log::info!("---> fetch_metadata()");
    let client = reqwest::Client::new();

    let auth_chars = match fetch_auth() {
        Ok(auth_chars) => auth_chars,
        Err(err) => {
            let ctx = set_context(str::from_utf8(HOST_NAME).unwrap(), None, None, Some(&err));
            log::error!("<--- fetch_metadata() ERROR");
            return Ok(build_http_response(StatusCode::OK, tmpl, ctx, true));
        }
    };

    log::info!("GET: {}", qs.url);

    let response = match client
        .get(qs.url.clone())
        .header("Authorization", format!("Basic {}", auth_chars))
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let ctx = set_context(
                str::from_utf8(HOST_NAME).unwrap(),
                None,
                None,
                Some(&err.to_string()),
            );
            log::error!("<--- fetch_metadata() ERROR");
            return Ok(build_http_response(
                StatusCode::from_u16(err.status().unwrap().as_u16()).unwrap(),
                tmpl,
                ctx,
                true,
            ));
        }
    };

    let http_status_code = StatusCode::from_u16(response.status().as_u16()).unwrap();
    log::info!("HTTP Status code = {}", http_status_code);

    let raw_xml = response.text().await.unwrap();

    match http_status_code {
        StatusCode::OK => {
            log::info!("<--- fetch_metadata()");
            Ok(HttpResponse::build(http_status_code)
                .content_type("text/plain")
                .body(raw_xml))
        }
        StatusCode::INTERNAL_SERVER_ERROR => {
            let ctx = set_context(
                str::from_utf8(HOST_NAME).unwrap(),
                None,
                None,
                Some(&parse_odata_error(&raw_xml)),
            );
            log::error!("<--- fetch_metadata() ERROR");
            Ok(build_http_response(http_status_code, tmpl, ctx, true))
        }
        _ => {
            let ctx = set_context(
                str::from_utf8(HOST_NAME).unwrap(),
                None,
                None,
                Some(&raw_xml),
            );
            log::error!("<--- fetch_metadata() ERROR");
            Ok(build_http_response(http_status_code, tmpl, ctx, true))
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
            Err(e) => Err(anyhow!(e)),
        },
        _ => Err(anyhow!(parse_odata_error(&raw_xml))),
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
fn set_context(
    hostname: &str,
    catalog_list: Option<Vec<String>>,
    service_list: Option<Vec<(String, String)>>,
    error_msg: Option<&str>,
) -> serde_json::Value {
    json!({
      "hostName": hostname,
      "catalogList": catalog_list,
      "serviceList": service_list,
      "errMsg": error_msg
    })
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
fn parse_odata_error(raw_xml: &str) -> String {
    match ODataError::from_str(&raw_xml) {
        Ok(odata_error) => format!("{:#?}", odata_error.message),
        Err(e) => format!("{:#?}", e),
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
fn build_http_response<'template, C>(
    status_code: StatusCode,
    template: web::Data<TinyTemplate<'template>>,
    context: C,
    is_error: bool,
) -> HttpResponse
where
    C: Serialize,
{
    let template_name = if is_error { "error.html" } else { "index.html" };
    let response_body = template
        .render(template_name, &context)
        .map_err(|err| error::ErrorInternalServerError(format!("Template error\n{}", err)))
        .unwrap();

    HttpResponse::build(status_code)
        .content_type("text/html; charset=utf-8")
        .body(response_body)
}

// ---------------------------------------------------------------------------------------------------------------------
#[cfg(test)]
pub mod unit_tests;

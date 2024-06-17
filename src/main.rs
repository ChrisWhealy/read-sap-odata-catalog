pub mod auth;
pub mod err_handlers;

use crate::{auth::fetch_auth, err_handlers::error_handlers};
use parse_sap_atom_feed::{atom::feed::Feed, odata_error::ODataError, xml::sanitise_xml};

use actix_web::{
    error, get,
    http::{header::ContentType, StatusCode},
    middleware, web, App, Error, HttpResponse, HttpServer, Result,
};
use serde_json::json;
use std::{
    collections::HashMap,
    str::{self, FromStr},
};
use tinytemplate::TinyTemplate;

include!(concat!(env!("OUT_DIR"), "/catalogservice.rs"));

use catalogservice::*;

static INDEX: &str = include_str!("../html/index.html");
static HOST_PATH: &[u8] = "https://sapes5.sapdevcenter.com/sap/opu/odata/iwfnd".as_bytes();
static SERVICE_NAME: &[u8] = "catalogservice;v=2".as_bytes();

// ---------------------------------------------------------------------------------------------------------------------
// Serve document root
// ---------------------------------------------------------------------------------------------------------------------
async fn doc_root(
    tmpl: web::Data<TinyTemplate<'_>>,
    _query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let ctx = json!({
      "service_name": str::from_utf8(SERVICE_NAME).unwrap(),
      "option_list": CatalogserviceEntities::as_list()
    });

    let body = tmpl
        .render("index.html", &ctx)
        .map_err(|err| error::ErrorInternalServerError(format!("Template error\n{}", err)))?;

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(body))
}

fn parse_odata_error(raw_xml: &str) -> String {
    match ODataError::from_str(&raw_xml) {
        Ok(odata_error) => format!("{:#?}", odata_error),
        Err(e) => format!("{:#?}", e),
    }
}

fn parse_xml(es_name: &str, xml: &str) -> String {
    match es_name {
        "Annotations" => match Feed::<Annotation>::from_str(&xml) {
            Ok(parsed_feed) => format!("{parsed_feed:#?}"),
            Err(e) => format!("Error: {e:?}"),
        },
        "Vocabularies" => match Feed::<Vocabulary>::from_str(&xml) {
            Ok(parsed_feed) => format!("{parsed_feed:#?}"),
            Err(e) => format!("Error: {e:?}"),
        },
        "ServiceCollection" => match Feed::<Service>::from_str(&xml) {
            Ok(parsed_feed) => format!("{parsed_feed:#?}"),
            Err(e) => format!("Error: {e:?}"),
        },
        "TagCollection" => match Feed::<Tag>::from_str(&xml) {
            Ok(parsed_feed) => format!("{parsed_feed:#?}"),
            Err(e) => format!("Error: {e:?}"),
        },
        "EntitySetCollection" => match Feed::<EntitySet>::from_str(&xml) {
            Ok(parsed_feed) => format!("{parsed_feed:#?}"),
            Err(e) => format!("Error: {e:?}"),
        },
        "CatalogCollection" => match Feed::<Catalog>::from_str(&xml) {
            Ok(parsed_feed) => format!("{parsed_feed:#?}"),
            Err(e) => format!("Error: {e:?}"),
        },

        _ => format!("Error: invalid entity set name '{}'", es_name),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Serve entity set contents
// ---------------------------------------------------------------------------------------------------------------------
#[get("/{entity_set_name}")]
async fn entity_set(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let client = reqwest::Client::new();
    let entity_set_name = path.into_inner();

    println!("GET: /{}", entity_set_name);

    if !catalogservice::CatalogserviceEntities::as_list().contains(&&entity_set_name[..]) {
        return Ok(HttpResponse::NotFound().finish());
    }

    let http_response = match fetch_auth() {
        Ok(auth_chars) => {
            let url = format!(
                "{}/{}/{}?$top=100",
                str::from_utf8(HOST_PATH).unwrap(),
                str::from_utf8(SERVICE_NAME).unwrap(),
                entity_set_name
            );
            log::info!("Fetching URL {}", url);

            match client
                .get(url)
                .header("Authorization", format!("Basic {}", auth_chars))
                .send()
                .await
            {
                Ok(response) => {
                    let http_status_code = response.status();
                    log::info!("HTTP Status code = {}", http_status_code);

                    let raw_xml = response.text().await.unwrap();
                    // log::info!("Raw XML response\n{}", raw_xml);

                    let response_body = match http_status_code {
                        reqwest::StatusCode::OK => {
                            parse_xml(&entity_set_name, &sanitise_xml(String::from(raw_xml)))
                        }
                        _ => parse_odata_error(&raw_xml),
                    };

                    HttpResponse::Ok()
                        .content_type(ContentType::plaintext())
                        .body(response_body)
                }
                Err(err) => HttpResponse::BadRequest().body(format!("{:#?}", err)),
            }
        }
        Err(err) => HttpResponse::BadRequest().body(format!("{:#?}", err)),
    };

    Ok(http_response)
}

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

        App::new()
            .app_data(web::Data::new(tt))
            .wrap(middleware::Logger::default())
            .service(web::resource("/").route(web::get().to(doc_root)))
            .service(entity_set)
            .service(web::scope("").wrap(error_handlers()))
    })
        .bind(("0.0.0.0", 8080))?
        .run()
        .await
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
#[cfg(test)]
pub mod unit_tests;

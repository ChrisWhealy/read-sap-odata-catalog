use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
    str::FromStr,
    string::FromUtf8Error,
};
use chrono::naive::NaiveDateTime;
use parse_sap_atom_feed::atom::feed::{Feed};

use catalogservice::*;

static FEED_XML_BASE: &str =
    "https://SAPES5.SAPDEVCENTER.COM:443/sap/opu/odata/iwfnd/catalogservice;v=2/";

static ATOM_XML_NAMESPACE: &str = "http://www.w3.org/2005/Atom";

include!(concat!(env!("OUT_DIR"), "/catalogservice.rs"));

fn fetch_xml_as_string(filename: &str) -> Result<String, FromUtf8Error> {
    let mut xml_buffer: Vec<u8> = Vec::new();
    let test_data = File::open(Path::new(&format!("./test_data/{}", filename))).unwrap();
    let _file_size = BufReader::new(test_data).read_to_end(&mut xml_buffer);

    String::from_utf8(xml_buffer)
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
#[test]
pub fn should_parse_annotations() {
    static ENTITY_SET_NAME: &str = "Annotations";
    let base_service_name = format!("{}{}", FEED_XML_BASE, ENTITY_SET_NAME);

    match fetch_xml_as_string(&format!("{}.xml", ENTITY_SET_NAME)) {
        Ok(xml) => {
            // let clean_xml = sanitise_xml(xml);
            let feed = Feed::<Annotation>::from_str(&xml).unwrap();

            assert_eq!(feed.namespace, Some(String::from(ATOM_XML_NAMESPACE)));
            assert_eq!(feed.xml_base, Some(String::from(FEED_XML_BASE)));
            assert_eq!(feed.id, base_service_name);
            assert_eq!(feed.title, ENTITY_SET_NAME);

            assert_eq!(feed.links.len(), 1);
            assert_eq!(feed.links[0].href, ENTITY_SET_NAME);

            // Check contents of entity set
            if let Some(entries) = feed.entries {
                assert_eq!(entries.len(), 16);

                // If the `src` attribute is populated, then the
                assert_eq!(entries[0].content.src, Some(String::from("Annotations(TechnicalName='ZPDCDS_ANNO_MDL',Version='0001')/$value")));

                let props = entries[0].properties.clone().unwrap();
                assert_eq!(props.technical_name, "ZPDCDS_ANNO_MDL");
                assert_eq!(props.version, "0001");
                assert_eq!(props.description, "Generic Annotation Provider");
                assert_eq!(props.media_type, "application/xml");
            } else {
                assert!(
                    1 == 2,
                    "{}",
                    format!(
                        "Entity set {} should not be empty!",
                        String::from(ENTITY_SET_NAME)
                    )
                )
            }
        }
        Err(err) => println!("XML test data was not in UTF8 format: {}", err),
    };
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
#[test]
pub fn should_parse_service_collection() {
    static ENTITY_SET_NAME: &str = "ServiceCollection";
    let base_service_name = format!("{}{}", FEED_XML_BASE, ENTITY_SET_NAME);

    match fetch_xml_as_string(&format!("{}.xml", ENTITY_SET_NAME)) {
        Ok(xml) => {
            // let clean_xml = sanitise_xml(xml);
            let feed = Feed::<Service>::from_str(&xml).unwrap();

            assert_eq!(feed.namespace, Some(String::from(ATOM_XML_NAMESPACE)));
            assert_eq!(feed.xml_base, Some(String::from(FEED_XML_BASE)));
            assert_eq!(feed.id, base_service_name);
            assert_eq!(feed.title, ENTITY_SET_NAME);

            assert_eq!(feed.links.len(), 2);
            assert_eq!(feed.links[0].href, ENTITY_SET_NAME);

            // Check contents of entity set
            if let Some(entries) = feed.entries {
                assert_eq!(entries.len(), 60);

                let props = entries[2].content.properties.clone().unwrap();

                assert_eq!(props.id, "WDR_ADAPT_UI_SRV_0001");
                assert_eq!(props.description, "Adapt Web Dynpro Applications in FLP");
                assert_eq!(props.title, "WDR_ADAPT_UI_SRV");
                assert_eq!(props.author, "SAP");
                assert_eq!(props.technical_service_version, 1);
                assert_eq!(props.technical_service_name, "WDR_ADAPT_UI_SRV");
                assert_eq!(props.metadata_url, "https://SAPES5.SAPDEVCENTER.COM:443/sap/opu/odata/sap/WDR_ADAPT_UI_SRV/$metadata");
                assert_eq!(props.image_url, "");
                assert_eq!(props.service_url, "https://SAPES5.SAPDEVCENTER.COM:443/sap/opu/odata/sap/WDR_ADAPT_UI_SRV");
                assert_eq!(props.updated_date, NaiveDateTime::from_str("2018-03-23T08:17:44").unwrap());
                assert_eq!(props.release_status, "");
                assert_eq!(props.category, "");
                assert_eq!(props.is_sap_service, true);
            } else {
                assert!(
                    1 == 2,
                    "{}",
                    format!(
                        "Entity set {} should not be empty!",
                        String::from(ENTITY_SET_NAME)
                    )
                )
            }
        }
        Err(err) => println!("XML test data was not in UTF8 format: {}", err),
    };
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
#[test]
pub fn should_parse_entity_set_collection() {
    static ENTITY_SET_NAME: &str = "EntitySetCollection";
    let base_service_name = format!("{}{}", FEED_XML_BASE, ENTITY_SET_NAME);

    match fetch_xml_as_string(&format!("{}.xml", ENTITY_SET_NAME)) {
        Ok(xml) => {
            // let clean_xml = sanitise_xml(xml);
            let feed = Feed::<EntitySet>::from_str(&xml).unwrap();

            assert_eq!(feed.namespace, Some(String::from(ATOM_XML_NAMESPACE)));
            assert_eq!(feed.xml_base, Some(String::from(FEED_XML_BASE)));
            assert_eq!(feed.id, base_service_name);
            assert_eq!(feed.title, ENTITY_SET_NAME);

            assert_eq!(feed.links.len(), 1);
            assert_eq!(feed.links[0].href, ENTITY_SET_NAME);

            // Check contents of entity set
            if let Some(entries) = feed.entries {
                assert_eq!(entries.len(), 12);

                let props = entries[2].content.properties.clone().unwrap();

                assert_eq!(props.id, "BAGS");
                assert_eq!(props.srv_identifier, "ZFIORI_CATALOGS_0001");
                assert_eq!(props.description, "Bags");
                assert_eq!(props.technical_service_name, "FIORI_CATALOGS");
                assert_eq!(props.technical_service_version, "0001");
            } else {
                assert!(
                    1 == 2,
                    "{}",
                    format!(
                        "Entity set {} should not be empty!",
                        String::from(ENTITY_SET_NAME)
                    )
                )
            }
        }
        Err(err) => println!("XML test data was not in UTF8 format: {}", err),
    };
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
#[test]
pub fn should_parse_tag_collection() {
    static ENTITY_SET_NAME: &str = "TagCollection";
    let base_service_name = format!("{}{}", FEED_XML_BASE, ENTITY_SET_NAME);

    match fetch_xml_as_string(&format!("{}.xml", ENTITY_SET_NAME)) {
        Ok(xml) => {
            // let clean_xml = sanitise_xml(xml);
            let feed = Feed::<Tag>::from_str(&xml).unwrap();

            assert_eq!(feed.namespace, Some(String::from(ATOM_XML_NAMESPACE)));
            assert_eq!(feed.xml_base, Some(String::from(FEED_XML_BASE)));
            assert_eq!(feed.id, base_service_name);
            assert_eq!(feed.title, ENTITY_SET_NAME);

            assert_eq!(feed.links.len(), 1);
            assert_eq!(feed.links[0].href, ENTITY_SET_NAME);

            // Check contents of entity set
            if let Some(entries) = feed.entries {
                assert_eq!(entries.len(), 8);

                let props = entries[7].content.properties.clone().unwrap();

                assert_eq!(props.id, "CDS.SEPMRA_C_PO_SUPPLIER.SEPMRA_C_PO_SUPPLIER");
                assert_eq!(props.text, "CDS.SEPMRA_C_PO_SUPPLIER.SEPMRA_C_PO_Supplier");
                assert_eq!(props.occurrence, 1);
            } else {
                assert!(
                    1 == 2,
                    "{}",
                    format!(
                        "Entity set {} should not be empty!",
                        String::from(ENTITY_SET_NAME)
                    )
                )
            }
        }
        Err(err) => println!("XML test data was not in UTF8 format: {}", err),
    };
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
#[test]
pub fn should_parse_catalog_collection() {
    static ENTITY_SET_NAME: &str = "CatalogCollection";
    let base_service_name = format!("{}{}", FEED_XML_BASE, ENTITY_SET_NAME);

    match fetch_xml_as_string(&format!("{}.xml", ENTITY_SET_NAME)) {
        Ok(xml) => {
            // let clean_xml = sanitise_xml(xml);
            let feed = Feed::<Catalog>::from_str(&xml).unwrap();

            assert_eq!(feed.namespace, Some(String::from(ATOM_XML_NAMESPACE)));
            assert_eq!(feed.xml_base, Some(String::from(FEED_XML_BASE)));
            assert_eq!(feed.id, base_service_name);
            assert_eq!(feed.title, ENTITY_SET_NAME);

            assert_eq!(feed.links.len(), 1);
            assert_eq!(feed.links[0].href, ENTITY_SET_NAME);

            // Check contents of entity set
            if let Some(entries) = feed.entries {
                assert_eq!(entries.len(), 1);

                let props = entries[0].content.properties.clone().unwrap();

                assert_eq!(props.id, "ES5");
                assert_eq!(props.description, "Service Catalog");
                assert_eq!(props.image_url, "");
                assert_eq!(props.title, "ES5");
                assert_eq!(props.updated_date, NaiveDateTime::from_str("2024-06-17T12:45:42").unwrap());
                assert_eq!(props.url, "");
            } else {
                assert!(
                    1 == 2,
                    "{}",
                    format!(
                        "Entity set {} should not be empty!",
                        String::from(ENTITY_SET_NAME)
                    )
                )
            }
        }
        Err(err) => println!("XML test data was not in UTF8 format: {}", err),
    };
}

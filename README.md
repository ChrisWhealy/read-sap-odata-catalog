# Minimal App for Reading the Demo OData Services from SAP's Dev Centre Server

A minimal demo app that calls the OData service `CATALOGSERVICE` on SAP's public Dev Center server.

It does this in the following stages:

1. Runs a build script that consumes the XML metadata description of `CATALOGSERVICE` (see the functionality in crate [`parse-sap-odata`](https://crates.io/crates/parse-sap-odata))
1. Generates a file called `catalogservice.rs` containing the module `catalogservice`
2. Using the generated `struct`s and `enum`s, the `atom:Feed` information exposed as entitysets in this OData service can then be consumed.

In this minimal demo scenario, the parsed entity set data is simply returned to the browser as formatted, plain text.

## Prerequisites

You must already have a userid and password for the SAP Dev Center server `sapes5.sapdevcenter.com`

1. Clone this repo
2. `cd read_sap_odata_catalog`
3. Create a `.env` file containing your SAP DevCenter userid and password in the following format

   ```
   SAP_USER=<your userid>
   SAP_PASSWORD=<your password>
   ```

# Local Execution

Once the `.env` file has been created, you can start the app using `cargo run`.

Visit <http://localhost:8080> and you will see a simple drop down list containing the entity sets available on the `CATALOGSERVICE` OData service.

Select `ServiceCollection` and the first 100 OData services available on this server will be displayed in raw, plain text.

## Identifying and then Calling an OData Service

After listing the contents of the `CatalogCollection`, you will see a collection of entries, each structured as shown below.

Look for the value in the field `Entry` -> `content` -> `properties` -> `Service` -> `service_url`.
This is the base URL for that particular service.

The example shown below, this value is `https://SAPES5.SAPDEVCENTER.COM:443/sap/opu/odata/sap/SEPMRA_SHOP`

```js
Entry {
    etag: None,
    id: "https://SAPES5.SAPDEVCENTER.COM:443/sap/opu/odata/iwfnd/catalogservice;v=2/ServiceCollection('ZSEPMRA_SHOP_0001')",
    title: "ServiceCollection('ZSEPMRA_SHOP_0001')",
    updated: "2024-06-17T13:32:45Z",
    category: "",
    links: [
        AtomLink {
            xml_namespace_atom: Some(
                "http://www.w3.org/2005/Atom",
            ),
            mime_type: None,
            rel: "self",
            href: "ServiceCollection('ZSEPMRA_SHOP_0001')",
            title: Some(
                "Service",
            ),
        },
        AtomLink {
            xml_namespace_atom: Some(
                "http://www.w3.org/2005/Atom",
            ),
            mime_type: Some(
                "application/atom+xml;type=feed",
            ),
            rel: "http://schemas.microsoft.com/ado/2007/08/dataservices/related/EntitySets",
            href: "ServiceCollection('ZSEPMRA_SHOP_0001')/EntitySets",
            title: Some(
                "EntitySets",
            ),
        },
        AtomLink {
            xml_namespace_atom: Some(
                "http://www.w3.org/2005/Atom",
            ),
            mime_type: Some(
                "application/atom+xml;type=feed",
            ),
            rel: "http://schemas.microsoft.com/ado/2007/08/dataservices/related/TagCollection",
            href: "ServiceCollection('ZSEPMRA_SHOP_0001')/TagCollection",
            title: Some(
                "TagCollection",
            ),
        },
        AtomLink {
            xml_namespace_atom: Some(
                "http://www.w3.org/2005/Atom",
            ),
            mime_type: Some(
                "application/atom+xml;type=feed",
            ),
            rel: "http://schemas.microsoft.com/ado/2007/08/dataservices/related/Annotations",
            href: "ServiceCollection('ZSEPMRA_SHOP_0001')/Annotations",
            title: Some(
                "Annotations",
            ),
        },
    ],
    content: Content {
        content_type: Some(
            "application/xml",
        ),
        namespace_m: "http://schemas.microsoft.com/ado/2007/08/dataservices/metadata",
        namespace_d: "http://schemas.microsoft.com/ado/2007/08/dataservices",
        src: None,
        properties: Some(
            Service {
                author: "DDIC",
                category: "",
                description: "EPM Fiori Reference Apps Shop",
                id: "ZSEPMRA_SHOP_0001",
                image_url: "",
                is_sap_service: true,
                metadata_url: "https://SAPES5.SAPDEVCENTER.COM:443/sap/opu/odata/sap/SEPMRA_SHOP/$metadata",
                release_status: "",
                service_url: "https://SAPES5.SAPDEVCENTER.COM:443/sap/opu/odata/sap/SEPMRA_SHOP",
                technical_service_name: "ZSEPMRA_SHOP",
                technical_service_version: 1,
                title: "SEPMRA_SHOP",
                updated_date: 2021-01-20T12:26:32,
            },
        ),
    },
    properties: None,
}
```
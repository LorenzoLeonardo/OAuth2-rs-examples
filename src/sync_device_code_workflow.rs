use curl::easy::Easy;
use http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use http::method::Method;
use http::status::StatusCode;
use oauth2::basic::BasicClient;
use oauth2::devicecode::StandardDeviceAuthorizationResponse;
use oauth2::{http, AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, TokenUrl};
use oauth2::{HttpRequest, HttpResponse};
use std::fmt::Debug;
use std::io::Read;
///
/// Error type returned by failed curl HTTP requests.
///
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error returned by curl crate.
    #[error("curl request failed")]
    Curl(#[source] curl::Error),
    /// Non-curl HTTP error.
    #[error("HTTP error")]
    Http(#[source] http::Error),
    /// Other error.
    #[error("Other error: {}", _0)]
    Other(String),
}

///
/// Synchronous Custom HTTP client.
///
fn custom_http_client(request: HttpRequest) -> Result<HttpResponse, Error> {
    let mut easy = Easy::new();
    easy.url(&request.url.to_string()[..])
        .map_err(Error::Curl)?;

    eprintln!("Check request: {:?}", request);
    let mut headers = curl::easy::List::new();
    request
        .headers
        .iter()
        .map(|(name, value)| {
            headers
                .append(&format!(
                    "{}: {}",
                    name,
                    value.to_str().map_err(|_| Error::Other(format!(
                        "invalid {} header value {:?}",
                        name,
                        value.as_bytes()
                    )))?
                ))
                .map_err(Error::Curl)
        })
        .collect::<Result<_, _>>()?;

    easy.http_headers(headers).map_err(Error::Curl)?;

    if let Method::POST = request.method {
        easy.post(true).map_err(Error::Curl)?;
        easy.post_field_size(request.body.len() as u64)
            .map_err(Error::Curl)?;
    } else {
        assert_eq!(request.method, Method::GET);
    }

    let mut form_slice = &request.body[..];
    let mut data = Vec::new();
    {
        let mut transfer = easy.transfer();

        transfer
            .read_function(|buf| Ok(form_slice.read(buf).unwrap_or(0)))
            .map_err(Error::Curl)?;

        transfer
            .write_function(|new_data| {
                data.extend_from_slice(new_data);
                Ok(new_data.len())
            })
            .map_err(Error::Curl)?;

        transfer.perform().map_err(Error::Curl)?;
    }

    let status_code = easy.response_code().map_err(Error::Curl)? as u16;

    eprintln!("Check reponse body: {:?}", data.clone());
    eprintln!("Check reponse status code: {}", status_code.clone());

    Ok(HttpResponse {
        status_code: StatusCode::from_u16(status_code).map_err(|err| Error::Http(err.into()))?,
        headers: easy
            .content_type()
            .map_err(Error::Curl)?
            .map(|content_type| {
                Ok(vec![(
                    CONTENT_TYPE,
                    HeaderValue::from_str(content_type).map_err(|err| Error::Http(err.into()))?,
                )]
                .into_iter()
                .collect::<HeaderMap>())
            })
            .transpose()?
            .unwrap_or_else(HeaderMap::new),
        body: data,
    })
}

pub fn example_sync_devicecode_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let device_auth_url = DeviceAuthorizationUrl::new(
        "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode".to_string(),
    )?;
    let client = BasicClient::new(
        ClientId::new("client_id".to_string()),
        None,
        AuthUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string())?,
        Some(TokenUrl::new(
            "https://login.microsoftonline.com/common/v2.0/oauth2/token".to_string(),
        )?),
    )
    .set_device_authorization_url(device_auth_url);

    let details: StandardDeviceAuthorizationResponse = client
        .exchange_device_code()?
        .add_scope(Scope::new("read".to_string()))
        .request(custom_http_client)?;

    eprintln!(
        "Open this URL in your browser:\n{}\nand enter the code: {}",
        details.verification_uri().to_string(),
        details.user_code().secret().to_string()
    );

    let token_result = client.exchange_device_access_token(&details).request(
        custom_http_client,
        std::thread::sleep,
        None,
    )?;

    eprintln!("Token:{:?}", token_result);

    Ok(())
}

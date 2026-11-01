//! The transport: one `reqwest` client, at most two requests in flight, ASCII-escaped bodies
//! (docs/architecture.md §4.1–§4.3).

use std::borrow::Cow;
use std::fmt::Write;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Map, Value};
use tokio::sync::Semaphore;
use url::Url;

use crate::API_V2;
use crate::error::{EagleError, network_error};

/// Where Eagle listens, and the token (never printed, never logged).
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub base: Url,
    pub token: SecretString,
}

/// Eagle's local API client. The semaphore is held until the response is parsed, so at most
/// two requests are in flight at any moment (every call runs on Eagle's UI thread).
#[derive(Debug, Clone)]
pub struct EagleHttp {
    client: reqwest::Client,
    endpoint: Arc<Endpoint>,
    permits: Arc<Semaphore>,
}

impl EagleHttp {
    pub fn new(endpoint: Endpoint, over_tailscale: bool) -> Result<Self, EagleError> {
        let client = reqwest::Client::builder()
            // A connect timeout almost always means the firewall on the Eagle PC.
            .connect_timeout(Duration::from_secs(if over_tailscale { 8 } else { 3 }))
            // The whole request, including a 1000-item page.
            .timeout(Duration::from_secs(60))
            // Shorter than Node's 5 s keep-alive (correction 21).
            .pool_idle_timeout(Duration::from_secs(4))
            .pool_max_idle_per_host(2)
            .use_rustls_tls()
            .build()
            .map_err(|error| EagleError::Invalid(format!("HTTP client: {error}")))?;
        Ok(Self {
            client,
            endpoint: Arc::new(endpoint),
            permits: Arc::new(Semaphore::new(2)),
        })
    }

    /// GET with the token in the query (docs/spec.md §8.3 "Auth").
    pub async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, EagleError> {
        let url = self.url(path)?;
        let _permit = self
            .permits
            .acquire()
            .await
            .expect("semaphore never closes");
        let response = self
            .client
            .get(url)
            .query(query)
            .query(&[("token", self.endpoint.token.expose_secret())])
            .send()
            .await
            .map_err(|error| EagleError::Network(network_error(&error)))?;
        parse_jsend(path, response).await
    }

    /// POST with the token in the JSON body only: Eagle logs every POST's URL, and a query
    /// token would end up in its log file (docs/spec.md §8.3, correction 6). The body is
    /// escaped to ASCII so a multibyte character split across TCP chunks never corrupts
    /// (correction 7).
    pub async fn post(
        &self,
        path: &str,
        mut body: Map<String, Value>,
    ) -> Result<Value, EagleError> {
        body.insert(
            "token".to_owned(),
            Value::from(self.endpoint.token.expose_secret()),
        );
        let text = match serde_json::to_string(&body) {
            Ok(text) => escape_non_ascii(&text).into_owned(),
            Err(error) => return Err(EagleError::Invalid(format!("bad body: {error}"))),
        };
        let url = self.url(path)?;
        let _permit = self
            .permits
            .acquire()
            .await
            .expect("semaphore never closes");
        let response = self
            .client
            .post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .body(text)
            .send()
            .await
            .map_err(|error| EagleError::Network(network_error(&error)))?;
        parse_jsend(path, response).await
    }

    fn url(&self, path: &str) -> Result<Url, EagleError> {
        self.endpoint
            .base
            .join(path)
            .map_err(|error| EagleError::Invalid(format!("bad API path {path}: {error}")))
    }
}

/// `path` with the API prefix, e.g. `api/v2/app/info`.
pub fn v2(path: &str) -> String {
    format!("{API_V2}{path}")
}

/// In serialized JSON, non-ASCII characters occur only inside strings, so the escaped text is
/// always valid JSON (docs/architecture.md §4.2). Characters outside the BMP become a
/// surrogate pair of escapes.
pub fn escape_non_ascii(json: &str) -> Cow<'_, str> {
    if json.is_ascii() {
        return Cow::Borrowed(json);
    }
    let mut out = String::with_capacity(json.len() + 16);
    for c in json.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                write!(out, "\\u{unit:04x}").unwrap();
            }
        }
    }
    Cow::Owned(out)
}

/// JSend decoding (docs/spec.md §8.3, corrections 4 and 20).
///
/// Returns the `data` value of a `success` response. `NotFound` is decided by the typed
/// layer, not here, because only it knows that `false` means "not found".
pub(crate) async fn parse_jsend(
    path: &str,
    response: reqwest::Response,
) -> Result<Value, EagleError> {
    let http = response.status().as_u16();
    let body: Value = match response.json().await {
        Ok(body) => body,
        // Not JSON (rare): the body of library/info and folder/get may carry folder
        // passwords, so it is never logged.
        Err(_) => {
            return Err(EagleError::Rejected {
                http,
                message: format!("HTTP {http}"),
            });
        }
    };
    if body.get("status").and_then(Value::as_str) == Some("success") {
        return Ok(body.get("data").cloned().unwrap_or(Value::Null));
    }
    match http {
        401 => return Err(EagleError::Unauthorized),
        404 if path == v2_path("app/info") => return Err(EagleError::ApiTooOld),
        400 => return Err(EagleError::BadRequest),
        _ => {}
    }
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| body.get("data").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| format!("HTTP {http}"));
    Err(EagleError::Rejected { http, message })
}

fn v2_path(path: &str) -> String {
    format!("{API_V2}{path}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ascii_passes_through_unchanged() {
        let text = r#"{"name":"eagle-api-test-1"}"#;
        assert!(matches!(escape_non_ascii(text), Cow::Borrowed(_)));
        assert_eq!(escape_non_ascii(text), text);
    }

    #[test]
    fn non_ascii_becomes_unicode_escapes() {
        assert_eq!(
            escape_non_ascii(r#"{"annotation":"Тест"}"#),
            r#"{"annotation":"\u0422\u0435\u0441\u0442"}"#
        );
        // Outside the BMP: a surrogate pair.
        assert_eq!(escape_non_ascii("\"🎨\""), "\"\\ud83c\\udfa8\"");
    }

    #[test]
    fn v2_prefixes_paths() {
        assert_eq!(v2("app/info"), "api/v2/app/info");
    }

    #[tokio::test]
    async fn jsend_success_returns_data() {
        let server = wiremock::MockServer::start().await;
        let body = json!({ "status": "success", "data": { "version": "4.0.0" } });
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v2/app/info"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(serde_json::to_string(&body).unwrap()),
            )
            .mount(&server)
            .await;
        let http = EagleHttp::new(
            Endpoint {
                base: Url::parse(&format!("{}/", server.uri())).unwrap(),
                token: SecretString::from("test-token"),
            },
            false,
        )
        .unwrap();
        let data = http.get("api/v2/app/info", &[]).await.unwrap();
        assert_eq!(data, json!({ "version": "4.0.0" }));
    }

    #[tokio::test]
    async fn get_sends_the_token_in_the_query() {
        let server = wiremock::MockServer::start().await;
        let body = json!({ "status": "success", "data": 2 });
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v2/item/countAll"))
            .and(wiremock::matchers::query_param("token", "test-token"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(serde_json::to_string(&body).unwrap()),
            )
            .mount(&server)
            .await;
        let http = EagleHttp::new(
            Endpoint {
                base: Url::parse(&format!("{}/", server.uri())).unwrap(),
                token: SecretString::from("test-token"),
            },
            false,
        )
        .unwrap();
        assert_eq!(
            http.get("api/v2/item/countAll", &[]).await.unwrap(),
            json!(2)
        );
    }

    #[tokio::test]
    async fn post_sends_the_token_in_the_body_only() {
        let server = wiremock::MockServer::start().await;
        let body = json!({ "status": "success", "data": { "data": [], "total": 0, "offset": 0, "limit": 1000 } });
        let request = wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/v2/item/get"))
            .and(wiremock::matchers::body_partial_json(
                json!({ "token": "sekret" }),
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(serde_json::to_string(&body).unwrap()),
            )
            .mount_as_scoped(&server)
            .await;
        let http = EagleHttp::new(
            Endpoint {
                base: Url::parse(&format!("{}/", server.uri())).unwrap(),
                token: SecretString::from("sekret"),
            },
            false,
        )
        .unwrap();
        http.post("api/v2/item/get", Map::new()).await.unwrap();
        let received = request.received_requests().await;
        // No token in the URL: Eagle logs every POST's URL (docs/spec.md §8.3).
        assert!(
            !received[0]
                .url
                .query()
                .unwrap_or_default()
                .contains("token")
        );
    }

    #[tokio::test]
    async fn post_escapes_non_ascii_in_the_body() {
        let server = wiremock::MockServer::start().await;
        let body = json!({ "status": "success", "data": true });
        let request = wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/v2/item/update"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(serde_json::to_string(&body).unwrap()),
            )
            .mount_as_scoped(&server)
            .await;
        let http = EagleHttp::new(
            Endpoint {
                base: Url::parse(&format!("{}/", server.uri())).unwrap(),
                token: SecretString::new("".into()),
            },
            false,
        )
        .unwrap();
        let mut sent = Map::new();
        sent.insert("annotation".to_owned(), Value::from("Тестовая заметка"));
        http.post("api/v2/item/update", sent).await.unwrap();
        let received = request.received_requests().await;
        let text = std::str::from_utf8(&received[0].body).unwrap();
        // Pure ASCII on the wire, the Cyrillic note escaped (correction 7).
        assert!(text.is_ascii());
        assert!(text.contains(r"\u0422\u0435\u0441\u0442"));
    }

    #[tokio::test]
    async fn error_mapping_matches_the_spec() {
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/unauthorized"))
            .respond_with(
                ResponseTemplate::new(401)
                    .insert_header("content-type", "application/json")
                    .set_body_string(json!({ "status": "error" }).to_string()),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/api/v2/app/info"))
            .respond_with(
                ResponseTemplate::new(404)
                    .insert_header("content-type", "application/json")
                    .set_body_string(json!({ "status": "error" }).to_string()),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/other404"))
            .respond_with(
                ResponseTemplate::new(404)
                    .insert_header("content-type", "application/json")
                    .set_body_string(json!({ "status": "error" }).to_string()),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/bad-request"))
            .respond_with(
                ResponseTemplate::new(400)
                    .insert_header("content-type", "application/json")
                    .set_body_string(json!({ "status": "error" }).to_string()),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/validation"))
            .respond_with(
                ResponseTemplate::new(500)
                    .insert_header("content-type", "application/json")
                    .set_body_string(
                        json!({ "status": "error", "message": "name too long (max 10000)" })
                            .to_string(),
                    ),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/v1-reject"))
            .respond_with(
                ResponseTemplate::new(500)
                    .insert_header("content-type", "application/json")
                    .set_body_string(
                        json!({ "status": "error", "data": "wrong password" }).to_string(),
                    ),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/fail200"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(
                        json!({ "status": "fail", "message": "explicit failure" }).to_string(),
                    ),
            )
            .mount(&server)
            .await;

        let http = EagleHttp::new(
            Endpoint {
                base: Url::parse(&format!("{}/", server.uri())).unwrap(),
                token: SecretString::new("".into()),
            },
            false,
        )
        .unwrap();

        assert_eq!(
            http.get("unauthorized", &[]).await.unwrap_err(),
            EagleError::Unauthorized
        );
        assert_eq!(
            http.get("api/v2/app/info", &[]).await.unwrap_err(),
            EagleError::ApiTooOld
        );
        assert!(matches!(
            http.get("other404", &[]).await.unwrap_err(),
            EagleError::Rejected { http: 404, .. }
        ));
        assert_eq!(
            http.get("bad-request", &[]).await.unwrap_err(),
            EagleError::BadRequest
        );
        assert_eq!(
            http.get("validation", &[]).await.unwrap_err(),
            EagleError::Rejected {
                http: 500,
                message: "name too long (max 10000)".into()
            }
        );
        // v1 handlers reject with a bare string in `data` (correction 4).
        assert_eq!(
            http.get("v1-reject", &[]).await.unwrap_err(),
            EagleError::Rejected {
                http: 500,
                message: "wrong password".into()
            }
        );
        assert_eq!(
            http.get("fail200", &[]).await.unwrap_err(),
            EagleError::Rejected {
                http: 200,
                message: "explicit failure".into()
            }
        );
    }
}

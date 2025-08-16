use base64::Engine;
use crate::libs::cli_args;
use crate::libs::mach_db::Work;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, COOKIE};
use base64::engine::general_purpose::STANDARD;

#[derive(Debug)]
pub struct Prober {
    config: cli_args::Args,
    client: Client,
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Option<Vec<String>>,
    pub headers_length: i64,
    pub body: Option<Vec<u8>>,
    pub body_length: i64,
}

#[derive(Debug)]
pub struct ProbeResult {
    pub status: String,
    pub response: Response,
}

#[derive(Debug)]
pub enum ProbeError {
    UnsupportedMethod(String),
    RequestFailed(String),
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbeError::UnsupportedMethod(method) => write!(f, "Unsupported HTTP method: {}", method),
            ProbeError::RequestFailed(err) => write!(f, "Request failed: {}", err),
        }
    }
}

impl Prober {
    pub async fn new(config: &cli_args::Args) -> Result<Self, String> {
        let policy = if config.follow_redirects {
            if config.follow_redirects_depth == 0 {
                reqwest::redirect::Policy::limited(usize::MAX) // effectively unlimited
            } else {
                reqwest::redirect::Policy::limited(config.follow_redirects_depth as usize)
            }
        } else {
            reqwest::redirect::Policy::none()
        };

        let user_agent = if config.random_user_agent_scan {
            crate::libs::rng::user_agent(None)
        } else {
            config.user_agent.clone()
        };


        let mut headers_map = HeaderMap::new();

        for header in &config.headers {
            if let Some((key, value)) = header.split_once(':') {
                let header_name = HeaderName::from_bytes(key.trim().as_bytes())
                    .map_err(|e| format!("Invalid header name '{}': {}", key, e))?;
                let header_value = HeaderValue::from_str(value.trim())
                    .map_err(|e| format!("Invalid header value '{}': {}", value, e))?;
                headers_map.insert(header_name, header_value);
            } else {
                return Err(format!("Invalid header format '{}'. Use 'Key: Value'", header).into());
            }
        }

        // --- Build Cookie header from config.cookies ---
        if !config.cookies.is_empty() {
            let cookie_string = config
                .cookies
                .iter()
                .filter_map(|c| c.split_once(':'))
                .map(|(k, v)| format!("{}={}", k.trim(), v.trim()))
                .collect::<Vec<_>>()
                .join("; ");

            headers_map.insert(
                COOKIE,
                HeaderValue::from_str(&cookie_string)
                    .map_err(|e| format!("Invalid cookie header: {}", e))?,
            );
        }

        // --- Add Basic Auth if configured ---
        if !config.basic_auth.is_empty() {
            if let Some((username, password)) = config.basic_auth.split_once(':') {
                let credentials = format!("{}:{}", username, password); // password may be empty
                let encoded = STANDARD.encode(credentials);
                let value = format!("Basic {}", encoded);
                headers_map.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&value)
                        .map_err(|e| format!("Invalid basic auth header: {}", e))?,
                );
            } else {
                return Err(format!("Invalid basic_auth format '{}'. Use 'username:password'", config.basic_auth).into());
            }
        }

        let reqwest_client = Client::builder()
            .default_headers(headers_map)
            .user_agent(user_agent)
            .cookie_store(config.store_cookies)
            .redirect(policy)
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            config: config.clone(),
            client: reqwest_client,
        })
    }

    pub async fn probe_url(&self, work: &Work, random_agent: bool) -> Result<ProbeResult, ProbeError> {
        let url = &work.url;
        let method = &work.method;

        // Build the request
        let mut request_builder = match method.as_str() {
            "get" => self.client.get(url),
            "post" => self.client.post(url),
            "put" => self.client.put(url),
            "delete" => self.client.delete(url),
            "head" => self.client.head(url),
            other => {
                return Err(ProbeError::UnsupportedMethod(format!(
                    "Unsupported HTTP method: {}",
                    other
                )));
            }
        };

        if random_agent {
            // If random_user_agent_scan is true, set a random user agent
            let user_agent = crate::libs::rng::user_agent(None);
            request_builder = request_builder.header(reqwest::header::USER_AGENT, user_agent)
        }

        // Send the request
        let response = request_builder.send().await;

        let valid_response = match response {
            Ok(resp) => resp,
            Err(e) => {
                return Err(ProbeError::RequestFailed(format!(
                    "Failed to send request: {}",
                    e
                )));
            }
        };

        let response_status = valid_response.status().as_u16();

        let probe_status = match &self.config.success_status_codes.contains(&response_status) {
            true => "found",
            false => "not_found",
        }
        .to_string();

        // headers format --> Name: Value
        let (headers, headers_length) = match &self.config.save_response_headers {
            true => {
                let headers = valid_response
                    .headers()
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name.as_str(), value.to_str().unwrap_or("")))
                    .collect::<Vec<String>>();
                let headers_length = headers.len().clone();
                (Some(headers), headers_length as i64)
            }
            false => {
                let headers_length = valid_response
                    .headers()
                    .len();

                dbg!(headers_length);
                (None, headers_length as i64)
            },
        };
        // if save_response_body is true, we need to get body as bytes anyway but,
        // if its false, check for content length first
        // if content length is present, we can skip reading the body
        let (body, body_length) = match self.config.save_response_body {
            true => {
                let valid_body = valid_response.bytes().await;
                match valid_body {
                    Ok(bytes) => {
                        (Some(bytes.to_vec()), bytes.len() as i64)
                    }
                    Err(_) => {
                        (None, 0)
                    }
                }
            }
            false => {
                let content_length = valid_response.content_length();
                match content_length {
                    Some(len) => (None, len as i64),
                    None => {
                        let valid_body = valid_response.bytes().await;
                        match valid_body {
                            Ok(bytes) => (None, bytes.len() as i64),
                            Err(_) => (None, 0),
                        }
                    },
                }
            }
        };

        // Create and return the ProbeResult
        Ok(ProbeResult {
            status: probe_status,
            response: Response {
                status: response_status,
                headers,
                headers_length,
                body,
                body_length,
            },
        })
    }
}

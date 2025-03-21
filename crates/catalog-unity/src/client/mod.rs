//! Generic utilities reqwest based Catalog implementations

pub mod backoff;
#[allow(unused)]
pub mod pagination;
pub mod retry;
pub mod token;

use crate::client::retry::RetryConfig;
use crate::UnityCatalogError;
use deltalake_core::data_catalog::DataCatalogResult;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{ClientBuilder, Proxy};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::time::Duration;

fn map_client_error(e: reqwest::Error) -> super::DataCatalogError {
    super::DataCatalogError::Generic {
        catalog: "HTTP client",
        source: Box::new(e),
    }
}

static DEFAULT_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// HTTP client configuration for remote catalogs
#[derive(Debug, Clone, Default)]
pub struct ClientOptions {
    user_agent: Option<HeaderValue>,
    default_headers: Option<HeaderMap>,
    proxy_url: Option<String>,
    allow_http: bool,
    allow_insecure: bool,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    pool_idle_timeout: Option<Duration>,
    pool_max_idle_per_host: Option<usize>,
    http2_keep_alive_interval: Option<Duration>,
    http2_keep_alive_timeout: Option<Duration>,
    http2_keep_alive_while_idle: bool,
    http1_only: bool,
    http2_only: bool,
    retry_config: Option<RetryConfig>,
}

impl ClientOptions {
    /// Create a new [`ClientOptions`] with default values
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the User-Agent header to be used by this client
    ///
    /// Default is based on the version of this crate
    pub fn with_user_agent(mut self, agent: HeaderValue) -> Self {
        self.user_agent = Some(agent);
        self
    }

    /// Sets the default headers for every request
    pub fn with_default_headers(mut self, headers: HeaderMap) -> Self {
        self.default_headers = Some(headers);
        self
    }

    /// Sets what protocol is allowed. If `allow_http` is :
    /// * false (default):  Only HTTPS are allowed
    /// * true:  HTTP and HTTPS are allowed
    pub fn with_allow_http(mut self, allow_http: bool) -> Self {
        self.allow_http = allow_http;
        self
    }
    /// Allows connections to invalid SSL certificates
    /// * false (default):  Only valid HTTPS certificates are allowed
    /// * true:  All HTTPS certificates are allowed
    ///
    /// # Warning
    ///
    /// You should think very carefully before using this method. If
    /// invalid certificates are trusted, *any* certificate for *any* site
    /// will be trusted for use. This includes expired certificates. This
    /// introduces significant vulnerabilities, and should only be used
    /// as a last resort or for testing
    pub fn with_allow_invalid_certificates(mut self, allow_insecure: bool) -> Self {
        self.allow_insecure = allow_insecure;
        self
    }

    /// Only use http1 connections
    pub fn with_http1_only(mut self) -> Self {
        self.http1_only = true;
        self
    }

    /// Only use http2 connections
    pub fn with_http2_only(mut self) -> Self {
        self.http2_only = true;
        self
    }

    /// Set an HTTP proxy to use for requests
    pub fn with_proxy_url(mut self, proxy_url: impl Into<String>) -> Self {
        self.proxy_url = Some(proxy_url.into());
        self
    }

    /// Set a request timeout
    ///
    /// The timeout is applied from when the request starts connecting until the
    /// response body has finished
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set a timeout for only the connect phase of a Client
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Set the pool max idle timeout
    ///
    /// This is the length of time an idle connection will be kept alive
    ///
    /// Default is 90 seconds
    pub fn with_pool_idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_idle_timeout = Some(timeout);
        self
    }

    /// Set the maximum number of idle connections per host
    ///
    /// Default is no limit
    pub fn with_pool_max_idle_per_host(mut self, max: usize) -> Self {
        self.pool_max_idle_per_host = Some(max);
        self
    }

    /// Sets an interval for HTTP2 Ping frames should be sent to keep a connection alive.
    ///
    /// Default is disabled
    pub fn with_http2_keep_alive_interval(mut self, interval: Duration) -> Self {
        self.http2_keep_alive_interval = Some(interval);
        self
    }

    /// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will be closed.
    /// Does nothing if http2_keep_alive_interval is disabled.
    ///
    /// Default is disabled
    pub fn with_http2_keep_alive_timeout(mut self, interval: Duration) -> Self {
        self.http2_keep_alive_timeout = Some(interval);
        self
    }

    /// Enable HTTP2 keep alive pings for idle connections
    ///
    /// If disabled, keep-alive pings are only sent while there are open request/response
    /// streams. If enabled, pings are also sent when no streams are active
    ///
    /// Default is disabled
    pub fn with_http2_keep_alive_while_idle(mut self) -> Self {
        self.http2_keep_alive_while_idle = true;
        self
    }

    pub fn with_retry_config(mut self, cfg: RetryConfig) -> Self {
        self.retry_config = Some(cfg);
        self
    }

    pub(crate) fn client(&self) -> DataCatalogResult<ClientWithMiddleware> {
        let mut builder = ClientBuilder::new();

        match &self.user_agent {
            Some(user_agent) => builder = builder.user_agent(user_agent),
            None => builder = builder.user_agent(DEFAULT_USER_AGENT),
        }

        if let Some(headers) = &self.default_headers {
            builder = builder.default_headers(headers.clone())
        }

        if let Some(proxy) = &self.proxy_url {
            let proxy = Proxy::all(proxy).map_err(map_client_error)?;
            builder = builder.proxy(proxy);
        }

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout)
        }

        if let Some(timeout) = self.connect_timeout {
            builder = builder.connect_timeout(timeout)
        }

        if let Some(timeout) = self.pool_idle_timeout {
            builder = builder.pool_idle_timeout(timeout)
        }

        if let Some(max) = self.pool_max_idle_per_host {
            builder = builder.pool_max_idle_per_host(max)
        }

        if let Some(interval) = self.http2_keep_alive_interval {
            builder = builder.http2_keep_alive_interval(interval)
        }

        if let Some(interval) = self.http2_keep_alive_timeout {
            builder = builder.http2_keep_alive_timeout(interval)
        }

        if self.http2_keep_alive_while_idle {
            builder = builder.http2_keep_alive_while_idle(true)
        }

        if self.http1_only {
            builder = builder.http1_only()
        }

        if self.http2_only {
            builder = builder.http2_prior_knowledge()
        }

        if self.allow_insecure {
            builder = builder.danger_accept_invalid_certs(self.allow_insecure)
        }

        let inner_client = builder
            .https_only(!self.allow_http)
            .build()
            .map_err(UnityCatalogError::from)?;
        let retry_policy = self
            .retry_config
            .as_ref()
            .map(|retry| retry.into())
            .unwrap_or(ExponentialBackoff::builder().build_with_max_retries(3));

        let middleware = RetryTransientMiddleware::new_with_policy(retry_policy);
        Ok(reqwest_middleware::ClientBuilder::new(inner_client)
            .with(middleware)
            .build())
    }
}

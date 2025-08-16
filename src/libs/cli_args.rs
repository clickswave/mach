use clap::{Parser, ValueEnum};
use reqwest::Url;

#[derive(Clone, Debug, ValueEnum)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                HttpMethod::Get => "get",
                HttpMethod::Post => "post",
                HttpMethod::Put => "put",
                HttpMethod::Delete => "delete",
                HttpMethod::Head => "head",
            }
        )
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LogLevel::Debug => "debug",
                LogLevel::Info => "info",
                LogLevel::Warn => "warn",
                LogLevel::Error => "error",
            }
        )
    }
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Text,
    Csv,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                OutputFormat::Text => "text",
                OutputFormat::Csv => "csv",
            }
        )
    }
}

/// {n}
/// |-------------------------------------------------|{n}
/// |                     M A C H                     |{n}
/// |-------------------------------------------------|{n}
/// |          Stateful asset discovery tool          |{n}
/// |                                                 |{n}
/// |                 clickswave.org                  |{n}
/// |-------------------------------------------------|{n}
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Specify the URL you want to target.
    /// If a wordlist is not provided, default wordlist will be used.
    /// {n}
    #[arg(short, long, verbatim_doc_comment, required = true)]
    pub url: Vec<String>,

    /// Specify the wordlist's path to be used{n}
    #[arg(short, long, required = true)]
    pub wordlist_path: String,

    /// Specify what point of the url should be replaced with words. Defaults to the end of specified URL.
    #[arg(long, default_value_t = format!("::FUZZ::"))]
    pub fuzz_marker: String,

    /// Specify multiple cookies to be used for the enumeration task.
    /// Format: "Cookie-name: Value";
    #[arg(long)]
    pub cookies: Vec<String>,

    /// Specify muliple headers to be used for the enumeration task.
    /// Format: "Header-name: Value";
    #[arg(long)]
    pub headers: Vec<String>,

    /// Specify basic authentication credentials in the format "username:password".
    #[arg(long, default_value_t = format!(""))]
    pub basic_auth: String,

    /// Specify if cookies should be stored when received from the server.
    /// This is useful for maintaining session state across requests.
    #[arg(long, default_value_t = false)]
    pub store_cookies: bool,

    /// Specify success status codes for the enumeration task.
    /// Defaults to 200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301, 302, 303, 304, 305, 306, 307, 308.
    /// These codes are used to determine if a request was successful.
    #[arg(long, value_delimiter = ',', default_values_t = vec![200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301, 302, 303, 304, 305, 306, 307, 308])]
    pub success_status_codes: Vec<u16>,

    /// Specify if redirection should be followed during active enumeration
    #[arg(long, default_value_t = true)]
    pub follow_redirects: bool,

    /// Specify redirection depth to be followed during active enumeration
    #[arg(long, default_value_t = 5)]
    pub follow_redirects_depth: u64,

    /// Specify the http method
    #[arg(long, value_enum, default_value_t = HttpMethod::Get)]
    pub http_method: HttpMethod,

    /// Specify the interval in ms between each request for a task. Defaults to 0
    #[arg(short, long, default_value_t = 0)]
    pub interval: u64,

    /// Specify the number of tasks to use.
    /// Think of tasks as threads that will be used to enumerate the URLs.
    #[arg(short, long, default_value_t = 2)]
    pub tasks: usize,

    /// Specify if the enumeration task should start from scratch.
    /// This will delete any existing data related to the enumeration task and start from scratch.
    #[arg(long, default_value_t = false)]
    pub fresh_start: bool,

    /// Specify if you want to use a random user agent for the whole scan.
    /// This will override the `user_agent` option.
    /// All requests within a scan will use the same random user agent.
    #[arg(long, default_value_t = false)]
    pub random_user_agent_scan: bool,

    /// Specify if you want to use a random user agent for each request.
    /// This will override the `random_user_agent_scan` option.
    /// Each request will use a different user agent.
    #[arg(long, default_value_t = false)]
    pub random_user_agent_request: bool,

    /// Append a trailing slash to the URL if it does not have one
    #[arg(long, default_value_t = true)]
    pub append_slash: bool,

    /// Save response body for each request
    #[arg(long, default_value_t = false)]
    pub save_response_body: bool,

    /// Save response headers for each request
    #[arg(long, default_value_t = true)]
    pub save_response_headers: bool,

    /// Specify the user agent to be used for enumeration.
    #[arg(long, default_value_t = format!("mach/{}", env!("CARGO_PKG_VERSION")))]
    pub user_agent: String,

    /// Disable banner display on startup
    #[arg(long, default_value_t = false)]
    pub no_exit_banner: bool,

    /// Delete existing database and start from scratch.
    #[arg(long, default_value_t = false)]
    pub recreate_db: bool,

    /// Specify launch delay in seconds
    #[arg(long, default_value_t = 0)]
    pub launch_delay: i64,

    /// Set minimum log level to debug, info, warn, error
    #[arg(long, value_enum, default_value_t = LogLevel::Info)]
    pub log_level: LogLevel,

    /// Set output format to text or csv
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub output_format: OutputFormat,

    /// Specify the output file path
    #[arg(short, long, default_value_t = format!(""))]
    pub output_path: String,

    /// Specify the event polling timeout in milliseconds for the TUI
    #[arg(long, default_value_t = 1000)]
    pub event_poll_timeout: u64,

    /// [UNSTABLE]
    /// Specify if you want to enable pagination for the TUI
    /// Recommended to use when the workload is massive but will cause, ui issues due fetch delays on input
    #[arg(long, default_value_t = false)]
    pub enable_offset_pagination: bool,
}

impl Args {
    pub fn new() -> Result<Self, String> {
        let mut args = Args::parse();

        // sleep for launch delay
        if args.launch_delay > 0 {
            std::thread::sleep(std::time::Duration::from_secs(args.launch_delay as u64));
        }

        if args.fuzz_marker.is_empty() {
            return Err("Fuzz marker cannot be empty".to_string());
        }

        // Sanitize and normalize URLs
        let mut valid_urls = vec![];
        for mut url in args.url {
            // Ensure URL has a scheme
            if !url.starts_with("http://") && !url.starts_with("https://") {
                url = format!("http://{}", url);
            }

            // Try parsing to validate format
            let mut parsed = match Url::parse(&url) {
                Ok(u) => u.to_string(),
                Err(e) => {
                    eprintln!("Error: Invalid URL `{}` - {}", url, e);
                    return Err(format!("Invalid URL: {}", url));
                }
            };

            // Ensure fuzz marker exists in path
            if !parsed.as_str().contains(&args.fuzz_marker) {
                if parsed.ends_with('/') {
                    parsed.push_str(&args.fuzz_marker);
                } else {
                    parsed.push_str(&format!("/{}", args.fuzz_marker));
                }
            }

            // append slash
            if !parsed.ends_with('/') && args.append_slash {
                parsed.push('/');
            }

            valid_urls.push(parsed);
        }
        args.url = valid_urls;

        Ok(args)
    }

}
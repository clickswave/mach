use crate::libs::cli_args;
use serde_json::json;

mod libs;
mod prober;
mod scanner;
mod tui;
mod exporter;

const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
const ORGANIZATION: &str = "clickswave";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Determine the current user's username for constructing a platform-specific data path.
    // Set the `db_path` or `state.db_path` based on the OS: Windows, Unix/Linux, or macOS.
    // Paths follow the standard conventions for storing user-specific application data.
    let username = whoami::username()?;
    let db_path;
    #[cfg(windows)]
    {
        state.db_path =
            format!(r"C:\Users\{username}\AppData\Roaming\{ORGANIZATION}\{PACKAGE_NAME}");
    }
    #[cfg(unix)]
    {
        db_path = format!("/home/{username}/.local/share/{ORGANIZATION}/{PACKAGE_NAME}");
    }
    #[cfg(target_os = "macos")]
    {
        state.db_path =
            format!("/Users/{username}/Library/Application Support/{ORGANIZATION}/{PACKAGE_NAME}");
    }
    // Parse command-line arguments using Clap.
    // Creates a new instance of the `Args` struct based on user input.
    // This will automatically handle flags, options, and help messages.
    let config = match cli_args::Args::new() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Error parsing arguments: {}", e);
            return Err(e.into());
        }
    };
    // Recreates the database if the `--recreate-db` flag is set.
    if config.recreate_db {
        if std::path::Path::new(&db_path).exists() {
            if let Err(e) = std::fs::remove_dir_all(&db_path) {
                return Err(e.into());
            }
        }
    }
    // Ensure the database directory exists else create it.
    if !std::path::Path::new(&db_path).exists() {
        if let Err(e) = std::fs::create_dir_all(&db_path) {
            return Err(e.into());
        }
    }

    // Initialize the database connection using the MachDb struct.
    let wordlist_config = libs::wordlist_config::WordlistConfig::new(&config.wordlist_path).await?;
    // Initialize the custom `MachDb` struct with the given database path and package name.
    // This struct provides methods to interact with and prepare the application's database.
    // The initialization is asynchronous and may perform setup like migrations or checks.
    let mach_db = libs::mach_db::MachDb::init(&db_path, PACKAGE_NAME, &config).await?;
    // Create the necessary database tables if they do not exist.
    mach_db.create_tables().await?;
    // Convert the wordlist configuration into a JSON string and compute its SHA-512 hash.
    let scan_config_json = serde_json::to_string(&json!({
        "urls": &config.url,
        "wordlist_hash": &wordlist_config.hash,
        "method": &config.http_method.to_string(),
    }))?;
    let scan_config_hash = libs::sha::sha512_from_string(scan_config_json).await?;
    // Fetch or create the wordlist based on the provided configuration.
    let wordlist = match mach_db.find_wordlist(&wordlist_config.hash).await {
        Ok(wordlist) => wordlist,
        Err(sqlx::Error::RowNotFound) => mach_db.create_wordlist(&wordlist_config).await?,
        Err(e) => return Err(e.into()),
    };

    // Fetch the words associated with the wordlist.
    let words = mach_db.fetch_words(&wordlist.id).await?;
    // Attempt to find an existing scan configuration based on the computed hash.
    let mut scan = match mach_db.find_scan(&scan_config_hash).await {
        Ok(scan) => {
            if config.fresh_start {
                mach_db.fresh_start_scan(&scan.id).await?
            } else {
                scan
            }
        }
        Err(sqlx::Error::RowNotFound) => {
            let scan = mach_db
                .create_scan(
                    &scan_config_hash,
                    &wordlist.id,
                    &config.http_method.to_string(),
                )
                .await?;
            scan
        }
        Err(e) => return Err(e.into()),
    };

    let logger = mach_db
        .spawn_logger(&scan.id, &config.log_level.to_string())
        .await?;

    logger.debug("Logger spawned").await?;

    let urls = match mach_db.find_urls(&scan.id).await {
        Ok(urls) => urls,
        Err(sqlx::Error::RowNotFound) => mach_db.create_urls(&scan.id, &config.url).await?,
        Err(e) => return Err(e.into()),
    };

    if urls.is_empty() {
        mach_db.create_urls(&scan.id, &config.url).await?;
    }

    logger.debug("Fetched URLs").await?;

    if scan.status == "created" {
        logger.info("Populating scan entries").await?;

        mach_db.create_scan_entries(&urls, &scan, &words).await?;

        scan.status = mach_db.set_scan_status(&scan.id, "populated").await?;
    }

    mach_db.reset_halted_scan_entries(&scan.id).await?;
    logger
        .info("Halted scan entries have been re-queued")
        .await?;

    let scanner = scanner::Scanner::new(config, mach_db.clone(), logger, scan.id);

    scanner.spawn_tasks().await?;

    Ok(())
}







use crate::libs;
use crate::libs::wordlist_config::WordlistConfig;
use crate::scanner::{LogTotals, Logs, ScanResult, ScanResults};
use sqlx::{Executor, FromRow};
use std::fmt::Display;
use tokio::fs;

pub struct Work {
    pub url: String,
    pub entry_id: i64,
    pub method: String,
}

impl Display for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Work {{ url: {}, entry_id: {}, method: {} }}",
            self.url, self.entry_id, self.method
        )
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct ScanEntry {
    pub id: i64,
    pub scan_id: i64,
    pub url_id: i64,
    pub word_id: i64,
    pub status: String,
    pub request_status: i32,
    pub headers: Option<String>, // JSON encoded Vec<String>
    pub headers_length: i64,
    pub body: Option<Vec<u8>>,
    pub body_length: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct Scan {
    pub id: i64,
    pub config_hash: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub notifications: String,
    pub wordlist_id: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct Log {
    pub id: i64,
    pub scan_id: i64,
    pub message: String,
    pub level: String,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct Wordlist {
    pub id: i64,
    pub name: String,
    pub hash: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct Url {
    pub id: i64,
    pub url: String,
    pub scan_id: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct Word {
    pub id: i64,
    pub word: String,
    pub wordlist_id: i64,
}

#[derive(Clone)]
pub struct MachDb {
    pool: sqlx::SqlitePool,
    config: crate::libs::cli_args::Args,
}

#[derive(Clone)]
pub struct Logger {
    pool: sqlx::SqlitePool,
    scan_id: i64,
    min_log_level: String,
    log_levels: Vec<&'static str>,
}

impl Logger {
    fn accept_log_level(&self, log_level: &str) -> bool {
        let min_log_level_index = self
            .log_levels
            .iter()
            .position(|&x| x == self.min_log_level)
            .unwrap_or(0);
        let current_log_level_index = self
            .log_levels
            .iter()
            .position(|&x| x == log_level)
            .unwrap_or(0);
        current_log_level_index >= min_log_level_index
    }

    async fn insert_log(&self, level: &str, description: &str) -> Result<(), sqlx::Error> {
        if !self.accept_log_level(level) {
            return Ok(());
        }

        sqlx::query(
            format!(
                "INSERT INTO logs (scan_id, level, description) VALUES ('{scan_id}', '{level}', '{description}')",
                scan_id = self.scan_id,
                level = level,
                description = description,
            )
                .as_str(),
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn error(&self, message: &str) -> Result<(), sqlx::Error> {
        self.insert_log("error", message).await?;

        Ok(())
    }
    pub async fn info(&self, message: &str) -> Result<(), sqlx::Error> {
        self.insert_log("info", message).await?;

        Ok(())
    }
    pub async fn debug(&self, message: &str) -> Result<(), sqlx::Error> {
        self.insert_log("debug", message).await?;

        Ok(())
    }
    pub async fn warn(&self, message: &str) -> Result<(), sqlx::Error> {
        self.insert_log("warn", message).await?;

        Ok(())
    }
}

impl MachDb {
    pub async fn init(
        db_path: &str,
        package_name: &str,
        config: &crate::cli_args::Args,
    ) -> Result<Self, sqlx::Error> {
        // create db if not exists
        let db_exists =
            std::path::Path::new(format!("{db_path}/{package_name}.sqlite").as_str()).exists();
        if !db_exists {
            fs::write(format!("{db_path}/{package_name}.sqlite"), b"").await?;
        }

        let sqlite_pool =
            sqlx::SqlitePool::connect(format!("sqlite:{db_path}/{package_name}.sqlite").as_str())
                .await?;
        // Enable WAL mode
        sqlite_pool.execute("PRAGMA journal_mode=WAL;").await?;
        // sync normal
        sqlite_pool.execute("PRAGMA synchronous=NORMAL;").await?;

        Ok(Self {
            pool: sqlite_pool,
            config: config.clone(),
        })
    }

    pub async fn spawn_logger(
        &self,
        scan_id: &i64,
        log_level: &str,
    ) -> Result<Logger, sqlx::Error> {
        Ok(Logger {
            pool: self.pool.clone(),
            scan_id: scan_id.clone(),
            min_log_level: log_level.to_string(),
            log_levels: vec!["debug", "info", "warn", "error"],
        })
    }

    pub async fn create_tables(&self) -> Result<(), sqlx::Error> {
        // wordlists table
        sqlx::query(
            r#"
        CREATE TABLE IF NOT EXISTS wordlists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            hash TEXT UNIQUE
        )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // words table
        sqlx::query(
            r#"
        CREATE TABLE IF NOT EXISTS words (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            word TEXT NOT NULL,
            wordlist_id INTEGER NOT NULL,
            FOREIGN KEY (wordlist_id) REFERENCES wordlists(id)
        )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // urls table
        sqlx::query(
            r#"
        CREATE TABLE IF NOT EXISTS urls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL,
            scan_id INTEGER NOT NULL,
            FOREIGN KEY (scan_id) REFERENCES scans(id)
        )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // scans table
        sqlx::query(
            r#"
        CREATE TABLE IF NOT EXISTS scans (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            config_hash TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP),
            updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP),
            notifications TEXT NOT NULL DEFAULT '{}',
            wordlist_id INTEGER,
            method TEXT NOT NULL,
            FOREIGN KEY (wordlist_id) REFERENCES wordlists(id)
        )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // scan_entries table
        sqlx::query(
            r#"
        CREATE TABLE IF NOT EXISTS scan_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_id INTEGER,
            url_id INTEGER,
            word_id INTEGER,
            status TEXT,
            request_status INTEGER NOT NULL DEFAULT 0,
            headers TEXT, -- JSON encoded Vec<String>
            headers_length INTEGER NOT NULL,
            body BLOB,
            body_length INTEGER NOT NULL,
            FOREIGN KEY (scan_id) REFERENCES scans(id),
            FOREIGN KEY (url_id) REFERENCES urls(id),
            FOREIGN KEY (word_id) REFERENCES words(id)
        )
        "#,
        )
            .execute(&self.pool)
            .await?;


        // logs table
        sqlx::query(
            r#"
        CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_id TEXT NOT NULL,
            level TEXT NOT NULL DEFAULT 'debug',
            description TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP)
        )
        "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn find_wordlist(&self, wordlist_hash: &str) -> Result<Wordlist, sqlx::Error> {
        let find_wordlist = sqlx::query_as::<_, Wordlist>("SELECT * FROM wordlists WHERE hash = ?")
            .bind(wordlist_hash)
            .fetch_one(&self.pool)
            .await;

        match find_wordlist {
            Ok(wordlist) => Ok(wordlist),
            Err(sqlx::Error::RowNotFound) => Err(sqlx::Error::RowNotFound),
            Err(e) => Err(e),
        }
    }

    pub async fn create_words(
        &self,
        wordlist_id: &i64,
        words: &Vec<String>,
    ) -> Result<Vec<Word>, sqlx::Error> {
        const SQLITE_MAX_VARIABLES: usize = 900; // Set a safe batch size
        let mut chunk_iter = words.chunks(SQLITE_MAX_VARIABLES / 2); // (2 bindings per row)
        while let Some(chunk) = chunk_iter.next() {
            let mut query = String::from("INSERT INTO words (word, wordlist_id) VALUES ");
            let mut params: Vec<(String, i64)> = Vec::new();

            for (i, word) in chunk.iter().enumerate() {
                if i > 0 {
                    query.push_str(", ");
                }
                query.push_str("(?, ?)");
                params.push((word.clone(), wordlist_id.clone()));
            }

            let mut query_builder = sqlx::query(query.as_str());
            for (word, id) in params {
                query_builder = query_builder.bind(word).bind(id);
            }
            let _query_result = query_builder.execute(&self.pool).await;
        }

        // fetch all words for the wordlist
        let words: Vec<Word> =
            sqlx::query_as::<_, Word>("SELECT * FROM words WHERE wordlist_id = ?")
                .bind(wordlist_id)
                .fetch_all(&self.pool)
                .await?;

        Ok(words)
    }

    pub async fn create_wordlist(
        &self,
        wordlist_config: &WordlistConfig,
    ) -> Result<Wordlist, sqlx::Error> {
        // create new wordlist
        let wordlist = sqlx::query_as::<_, Wordlist>(
            "INSERT INTO wordlists (name, hash) VALUES (?, ?) RETURNING *",
        )
        .bind(&wordlist_config.name)
        .bind(&wordlist_config.hash)
        .fetch_one(&self.pool)
        .await?;
        // insert words to words table
        let words = libs::utils::read_lines(&wordlist_config.path).await?;

        let _ = &self.create_words(&wordlist.id, &words).await?;

        Ok(wordlist)
    }

    pub async fn fetch_words(&self, wordlist_id: &i64) -> Result<Vec<Word>, sqlx::Error> {
        let words = sqlx::query_as::<_, Word>("SELECT * FROM words WHERE wordlist_id = ?")
            .bind(wordlist_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(words)
    }

    pub async fn find_scan(&self, scan_hash: &str) -> Result<Scan, sqlx::Error> {
        let find_scan = sqlx::query_as::<_, Scan>("SELECT * FROM scans WHERE config_hash = ?")
            .bind(scan_hash)
            .fetch_one(&self.pool)
            .await?;

        Ok(find_scan)
    }

    pub async fn fetch_found_scan_entries(
        &self,
        scan_id: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ScanResult>, sqlx::Error> {
        let found_entries = match limit {
            0 => {
                sqlx::query_as::<_, ScanEntry>(
                    "SELECT * FROM scan_entries WHERE scan_id = ? AND status = 'found'",
                )
                    .bind(scan_id)
                    .fetch_all(&self.pool)
                    .await?
            },
            _ => {
                sqlx::query_as::<_, ScanEntry>(
                    "SELECT * FROM scan_entries WHERE scan_id = ? AND status = 'found' LIMIT ? OFFSET ?",
                )
                    .bind(scan_id)
                    .bind(limit as i64)
                    .bind(offset as i64)
                    .fetch_all(&self.pool)
                    .await?
            }
        };

        let mut found_urls = Vec::new();
        for entry in found_entries {
            let mut url = sqlx::query_scalar::<_, String>("SELECT url FROM urls WHERE id = ?")
                .bind(entry.url_id)
                .fetch_one(&self.pool)
                .await?;
            let word = sqlx::query_scalar::<_, String>("SELECT word FROM words WHERE id = ?")
                .bind(entry.word_id)
                .fetch_one(&self.pool)
                .await?;
            url = url.replace(&self.config.fuzz_marker, &word);

            found_urls.push(ScanResult {
                url,
                scan_status: entry.status.clone(),
                request_status: entry.request_status.to_string(),
                body_length: entry.body_length,
                headers_length: entry.headers_length,
            });
        }
        Ok(found_urls)
    }

    pub async fn fetch_not_found_scan_entries(
        &self,
        scan_id: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ScanResult>, sqlx::Error> {
        let not_found_entries = match limit {
            0 => {
                sqlx::query_as::<_, ScanEntry>(
                    "SELECT * FROM scan_entries WHERE scan_id = ? AND status = 'not_found'",
                )
                    .bind(scan_id)
                    .fetch_all(&self.pool)
                    .await?
            },
            _ => {
                sqlx::query_as::<_, ScanEntry>(
                    "SELECT * FROM scan_entries WHERE scan_id = ? AND status = 'not_found' LIMIT ? OFFSET ?",
                )
                    .bind(scan_id)
                    .bind(limit as i64)
                    .bind(offset as i64)
                    .fetch_all(&self.pool)
                    .await?
            }
        };

        let mut not_found_urls = Vec::new();
        for entry in not_found_entries {
            let mut url = sqlx::query_scalar::<_, String>("SELECT url FROM urls WHERE id = ?")
                .bind(entry.url_id)
                .fetch_one(&self.pool)
                .await?;
            let word = sqlx::query_scalar::<_, String>("SELECT word FROM words WHERE id = ?")
                .bind(entry.word_id)
                .fetch_one(&self.pool)
                .await?;
            url = url.replace(&self.config.fuzz_marker, &word);

            not_found_urls.push(ScanResult {
                url,
                scan_status: entry.status.clone(),
                request_status: entry.request_status.to_string(),
                body_length: entry.body_length,
                headers_length: entry.headers_length,
            });
        }
        Ok(not_found_urls)
    }

    pub async fn fetch_error_scan_entries(
        &self,
        scan_id: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ScanResult>, sqlx::Error> {
        let error_entries = match limit {
            0 => {
                sqlx::query_as::<_, ScanEntry>(
                    "SELECT * FROM scan_entries WHERE scan_id = ? AND status = 'error'",
                )
                    .bind(scan_id)
                    .fetch_all(&self.pool)
                    .await?
            },
            _ => {
                sqlx::query_as::<_, ScanEntry>(
                    "SELECT * FROM scan_entries WHERE scan_id = ? AND status = 'error' LIMIT ? OFFSET ?",
                )
                    .bind(scan_id)
                    .bind(limit as i64)
                    .bind(offset as i64)
                    .fetch_all(&self.pool)
                    .await?
            }
        };

        let mut error_urls = Vec::new();
        for entry in error_entries {
            let mut url = sqlx::query_scalar::<_, String>("SELECT url FROM urls WHERE id = ?")
                .bind(entry.url_id)
                .fetch_one(&self.pool)
                .await?;
            let word = sqlx::query_scalar::<_, String>("SELECT word FROM words WHERE id = ?")
                .bind(entry.word_id)
                .fetch_one(&self.pool)
                .await?;
            url = url.replace(&self.config.fuzz_marker, &word);

            error_urls.push(ScanResult {
                url,
                scan_status: entry.status.clone(),
                request_status: entry.request_status.to_string(),
                body_length: entry.body_length,
                headers_length: entry.headers_length,
            });
        }
        Ok(error_urls)
    }

    pub async fn fetch_total_scan_entries(
        &self,
        scan_id: i64,
    ) -> Result<(usize, usize, usize, usize), sqlx::Error> {

        let found_entries = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM scan_entries WHERE scan_id = ? AND status = 'found'",
        )
        .bind(scan_id)
        .fetch_one(&self.pool)
        .await?;

        let not_found_entries = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM scan_entries WHERE scan_id = ? AND status = 'not_found'",
        )
        .bind(scan_id)
        .fetch_one(&self.pool)
        .await?;

        let error_entries = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM scan_entries WHERE scan_id = ? AND status = 'error'",
        )
        .bind(scan_id)
        .fetch_one(&self.pool)
        .await?;

        let total_entries = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM scan_entries WHERE scan_id = ?",
        )
        .bind(scan_id)
        .fetch_one(&self.pool)
        .await?;

        Ok((
            found_entries as usize,
            not_found_entries as usize,
            error_entries as usize,
            total_entries as usize,
        ))
    }

    pub async fn get_scan_results(&self, scan_id: i64, limit: usize, offset: usize) -> Result<ScanResults, sqlx::Error> {
        let found = self.fetch_found_scan_entries(scan_id, limit, offset).await?;
        let not_found = self.fetch_not_found_scan_entries(scan_id, limit, offset).await?;
        let error = self.fetch_error_scan_entries(scan_id, limit, offset).await?;

        let (found_total, not_found_total, error_total, entries_total) = self.fetch_total_scan_entries(scan_id).await?;

        let result = ScanResults {
            found,
            not_found,
            error,
            totals: crate::scanner::ScanResultTotals {
                found: found_total,
                not_found: not_found_total,
                error: error_total,
                entries: entries_total,
            },
        };

        Ok(result)
    }

    pub async fn get_log_totals(&self) -> Result<LogTotals, sqlx::Error> {
        let debug_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs WHERE level = 'debug'")
            .fetch_one(&self.pool)
            .await?;
        let info_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs WHERE level = 'info'")
            .fetch_one(&self.pool)
            .await?;
        let warn_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs WHERE level = 'warn'")
            .fetch_one(&self.pool)
            .await?;
        let error_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs WHERE level = 'error'")
            .fetch_one(&self.pool)
            .await?;

        Ok(LogTotals {
            debug: debug_count as usize,
            info: info_count as usize,
            warn: warn_count as usize,
            error: error_count as usize,
            entries: (debug_count + info_count + warn_count + error_count) as usize,
        })
    }

    pub async fn get_logs(
        &self,
        scan_id: &i64,
        limit: usize,
        offset: usize,
    ) -> Result<Logs, sqlx::Error> {

        let logs = match limit {
            0 => {
                sqlx::query_as::<_, crate::scanner::Log>(
                    "SELECT * FROM logs WHERE scan_id = ? ORDER BY created_at DESC",
                )
                    .bind(scan_id)
                    .fetch_all(&self.pool)
                    .await?
            },
            _ => {
                sqlx::query_as::<_, crate::scanner::Log>(
                    "SELECT * FROM logs WHERE scan_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
                )
                    .bind(scan_id)
                    .bind(limit as i64)
                    .bind(offset as i64)
                    .fetch_all(&self.pool)
                    .await?
            }
        };

        let log_totals = self.get_log_totals().await?;

        Ok(Logs {
            logs,
            totals: log_totals,
        })
    }

    pub async fn update_work_status(
        &self,
        entry_id: i64,
        status: &str,
        response_status: &str,
        body: Option<Vec<u8>>,
        headers: Option<Vec<String>>,
        headers_length: i64,
        body_length: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE scan_entries SET status = ?, request_status = ?, body = ?, headers = ?, headers_length = ?, body_length = ? WHERE id = ?",
        )
            .bind(status)
            .bind(response_status)
            .bind(body)
            .bind(headers.map(|h| serde_json::to_string(&h).unwrap()))
            .bind(headers_length)
            .bind(body_length)
            .bind(entry_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn create_scan(
        &self,
        scan_config_hash: &str,
        wordlist_id: &i64,
        http_method: &str,
    ) -> Result<Scan, sqlx::Error> {
        // create new scan
        let scan = sqlx::query_as::<_, Scan>(
            // "INSERT INTO scans (config_hash, status) VALUES (?, 'created') RETURNING *",
            "INSERT INTO scans (config_hash, status, wordlist_id, method) VALUES (?, 'created', ?, ?) RETURNING *",
        )
            .bind(scan_config_hash)
            .bind(wordlist_id)
            .bind(http_method)
            .fetch_one(&self.pool)
            .await?;
        Ok(scan)
    }

    pub async fn create_urls(
        &self,
        scan_id: &i64,
        urls: &Vec<String>,
    ) -> Result<Vec<Url>, sqlx::Error> {
        const SQLITE_MAX_VARIABLES: usize = 900; // Set a safe batch size
        let mut chunk_iter = urls.chunks(SQLITE_MAX_VARIABLES / 2); // (2 bindings per row)
        while let Some(chunk) = chunk_iter.next() {
            let mut query = String::from("INSERT INTO urls (url, scan_id) VALUES ");
            let mut params: Vec<(String, i64)> = Vec::new();

            for (i, url) in chunk.iter().enumerate() {
                if i > 0 {
                    query.push_str(", ");
                }
                query.push_str("(?, ?)");
                params.push((url.clone(), scan_id.clone()));
            }

            let mut query_builder = sqlx::query(query.as_str());
            for (url, id) in params {
                query_builder = query_builder.bind(url).bind(id);
            }
            let _query_result = query_builder.execute(&self.pool).await;
        }

        let urls: Vec<Url> = sqlx::query_as::<_, Url>("SELECT * FROM urls WHERE scan_id = ?")
            .bind(scan_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(urls)
    }

    pub async fn find_urls(&self, scan_id: &i64) -> Result<Vec<Url>, sqlx::Error> {
        let urls = sqlx::query_as::<_, Url>("SELECT * FROM urls WHERE scan_id = ?")
            .bind(scan_id)
            .fetch_all(&self.pool)
            .await?;

        if urls.is_empty() {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(urls)
    }

    pub async fn fresh_start_scan(&self, scan_id: &i64) -> Result<Scan, sqlx::Error> {
        // set row in scans table to 'scan_created'
        sqlx::query(
            format!("UPDATE scans SET status = 'scan_created' WHERE id = '{scan_id}'").as_str(),
        )
        .execute(&self.pool)
        .await?;
        // clear any existing logs
        sqlx::query(format!("DELETE FROM logs WHERE scan_id = '{scan_id}'").as_str())
            .execute(&self.pool)
            .await?;
        // drop workload table
        sqlx::query(format!("DROP TABLE IF EXISTS {scan_id}").as_str())
            .execute(&self.pool)
            .await?;

        // get scan
        let scan = sqlx::query_as::<_, Scan>(
            format!("SELECT * FROM scans WHERE id = '{scan_id}'").as_str(),
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(scan)
    }

    pub async fn create_scan_entries(
        &self,
        urls: &Vec<Url>,
        scan: &Scan,
        words: &Vec<Word>,
    ) -> Result<(), sqlx::Error> {
        // for all urls create a scan entry for each word
        for url in urls {
            // insert scan entries
            const SQLITE_MAX_VARIABLES: usize = 900; // Set a safe batch size
            let mut chunk_iter = words.chunks(SQLITE_MAX_VARIABLES / 2); // (2 bindings per row)
            while let Some(chunk) = chunk_iter.next() {
                let mut query = String::from(
                    "INSERT INTO scan_entries (word_id, scan_id, url_id, status, headers_length, body_length, headers, body) VALUES ",
                );
                let mut params: Vec<(i64, i64, i64, String, i64, i64, Option<Vec<String>>, Option<Vec<u8>>)> = Vec::new();

                for (i, word) in chunk.iter().enumerate() {
                    if i > 0 {
                        query.push_str(", ");
                    }
                    query.push_str("(?, ?, ?, ?, ?, ?, ?, ?)");
                    params.push((
                        word.id,
                        scan.id,
                        url.id,
                        "queued".to_string(),
                        0, // headers_length
                        0, // body_length
                        None, // headers
                        None, // body
                    ));
                }

                let mut query_builder = sqlx::query(query.as_str());
                for (word_id, scan_id, url_id, status, headers_length, body_length, headers, body) in params {
                    query_builder = query_builder
                        .bind(word_id)
                        .bind(scan_id)
                        .bind(url_id)
                        .bind(status)
                        .bind(headers_length)
                        .bind(body_length)
                        .bind(headers.map(|h| serde_json::to_string(&h).unwrap()))
                        .bind(body)
                    ;
                }
                let result = query_builder.execute(&self.pool).await;
                if let Err(e) = result {
                    eprintln!("Error inserting scan entries: {:?}", e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    pub async fn set_scan_status(
        &self,
        scan_id: &i64,
        status: &str,
    ) -> Result<String, sqlx::Error> {
        sqlx::query(
            format!("UPDATE scans SET status = '{status}' WHERE id = '{scan_id}'").as_str(),
        )
        .execute(&self.pool)
        .await?;
        Ok(status.to_string())
    }

    pub async fn reset_halted_scan_entries(&self, scan_id: &i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE scan_entries SET status = 'queued' WHERE status = 'processing' AND scan_id = ?",
        )
        .bind(scan_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_work_one(&self, scan_id: &i64) -> Result<Work, sqlx::Error> {
        // fetch one entry from scan_entries where status is 'queued'
        let scan_entry = sqlx::query_as::<_, ScanEntry>(
            r#"
                WITH selected AS (
                    SELECT id
                    FROM scan_entries
                    WHERE status = 'queued' AND scan_id = ?
                    ORDER BY id
                    LIMIT 1
                )
                UPDATE scan_entries
                SET status = 'scanning'
                WHERE id IN (SELECT id FROM selected)
                RETURNING *
                "#,
        )
        .bind(scan_id) // <-- Bind your parameter here
        .fetch_one(&self.pool)
        .await?;

        // find url using id
        let url_string = sqlx::query_scalar::<_, String>("SELECT url FROM urls WHERE id = ?")
            .bind(scan_entry.url_id)
            .fetch_one(&self.pool)
            .await?;

        let word_string = sqlx::query_scalar::<_, String>("SELECT word FROM words WHERE id = ?")
            .bind(scan_entry.word_id)
            .fetch_one(&self.pool)
            .await?;

        let full_url = url_string.replace(&self.config.fuzz_marker, &word_string);

        let work = Work {
            url: full_url,
            entry_id: scan_entry.id,
            method: self.config.http_method.to_string(),
        };

        Ok(work)
    }
}

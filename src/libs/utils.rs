use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn read_lines(file_path: &str) -> Result<Vec<String>, std::io::Error> {
    let file = File::open(file_path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut result = Vec::new();

    while let Some(line) = lines.next_line().await? {
        if line.is_empty() {
            continue;
        };

        result.push(line);
    }

    Ok(result)
}
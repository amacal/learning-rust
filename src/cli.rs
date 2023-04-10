use std::io::Error;

use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::{self, JoinHandle};

use futures::stream::FuturesOrdered;
use futures::StreamExt;

use base64::{engine::general_purpose, Engine as _};
use structopt::StructOpt;
use thiserror::Error;

use crate::databricks::DatabricksApiError;
use crate::environment::DatabricksCredentialsError;

use super::databricks::DatabricksApiClient;
use super::environment::find_databricks_credentials;
use super::helpers::format_timestamp;

#[derive(StructOpt, Debug)]
pub struct DbfsList {
    #[structopt(help = "The absolute path of the DBFS file or directory.")]
    pub path: String,
}

#[derive(StructOpt, Debug)]
pub struct DbfsGet {
    #[structopt(help = "The absolute path of the DBFS file.")]
    pub path: String,
    #[structopt(help = "The absolute or relative path of the local file.")]
    pub output: String,
}

#[derive(StructOpt, Debug)]
pub enum Dbfs {
    #[structopt(name = "get", help = "Downloads the remote DBFS file.")]
    Get(DbfsGet),
    #[structopt(name = "list", help = "Lists the remote DBFS directory.")]
    List(DbfsList),
}

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Configuration is not correct: {0}")]
    Configuration(DatabricksCredentialsError),
    #[error("Communication with Databricks API failed: {0}")]
    DatabricksApi(DatabricksApiError),
    #[error("Returned data is not consistent: {0}")]
    DataConsistency(String),
    #[error("Local IO operation cannot be completed: {0}")]
    InputOutput(Error),
    #[error("Something happened in the application: {0}")]
    Internal(String),
}

fn create_api_client() -> Result<DatabricksApiClient, CliError> {
    let client = find_databricks_credentials()
        .map(|credentials| DatabricksApiClient::new(credentials.host, credentials.token))
        .map_err(|error| CliError::Configuration(error))?;

    Ok(client)
}

#[derive(Debug)]
struct DbfsGetChunkInfo {
    path: String,
    offset: usize,
    length: usize,
}

#[derive(Debug)]
struct DbfsGetChunkData {
    data: Vec<u8>,
}

async fn process_dbfs_get_chunk(
    client: DatabricksApiClient,
    info: DbfsGetChunkInfo,
) -> Result<DbfsGetChunkData, CliError> {
    let block = client
        .dbfs_read(&info.path, info.offset, info.length)
        .await
        .map_err(CliError::DatabricksApi)?;

    let decoded = general_purpose::STANDARD
        .decode(block.data)
        .map_err(|error| CliError::DataConsistency(error.to_string()))?;

    Ok(DbfsGetChunkData { data: decoded })
}

async fn complete_dbfs_get_chunk(
    file: &mut File,
    tasks: &mut FuturesOrdered<JoinHandle<Result<DbfsGetChunkData, CliError>>>,
    maximum: usize,
) -> Result<(), CliError> {
    while tasks.len() >= maximum {
        match tasks.next().await {
            Some(Ok(Ok(data))) => file.write_all(&data.data).await.map_err(CliError::InputOutput)?,
            Some(Ok(Err(error))) => return Err(error),
            Some(Err(_)) => return Err(CliError::Internal("Tasks interrupted".to_string())),
            None if maximum > 0 => return Err(CliError::Internal("Tasks exhausted".to_string())),
            None => break,
        }
    }

    Ok(())
}

pub async fn handle_dbfs_get(args: DbfsGet) -> Result<(), CliError> {
    let client = create_api_client()?;
    let file_info = client.dbfs_status(&args.path).await.map_err(CliError::DatabricksApi)?;

    let mut file = File::create(&args.output).await.map_err(CliError::InputOutput)?;
    let mut tasks = FuturesOrdered::new();

    let mut offset: usize = 0;
    let mut left = file_info.file_size;

    while left > 0 {
        let length = std::cmp::min(1048576, left);
        let next = DbfsGetChunkInfo {
            path: args.path.clone(),
            offset: offset,
            length: length,
        };

        tasks.push_back(task::spawn(process_dbfs_get_chunk(client.clone(), next)));
        complete_dbfs_get_chunk(&mut file, &mut tasks, 2).await?;

        left -= length;
        offset += length;
    }

    complete_dbfs_get_chunk(&mut file, &mut tasks, 0).await?;
    file.flush().await.map_err(CliError::InputOutput)?;

    Ok(())
}

pub async fn handle_dbfs_list(args: DbfsList) -> Result<(), CliError> {
    let client = create_api_client()?;
    let response = client.dbfs_list(args.path).await.map_err(CliError::DatabricksApi)?;

    let files: Result<Vec<(String, String, String)>, _> = response
        .files
        .unwrap_or_default()
        .iter()
        .map(|item| {
            let modified = format_timestamp(item.modification_time)
                .map_err(|error| CliError::Internal(error.to_string()))?;

            let file_size = match item.is_dir {
                true => String::from("DIR"),
                false => item.file_size.to_string(),
            };

            let path = item.path.clone();
            Ok((path, file_size, modified))
        })
        .collect();

    match files {
        Ok(files) => {
            for (path, file_size, modified) in files {
                println!("{:>12} {:>19} {}", file_size, modified, path)
            }
        }
        Err(err) => return Err(err),
    };

    Ok(())
}

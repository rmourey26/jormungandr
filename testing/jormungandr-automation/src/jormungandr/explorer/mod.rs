use crate::testing::configuration::get_explorer_app;

use self::{
    client::GraphQlClient,
    data::{
        address, all_blocks, all_stake_pools, all_vote_plans, blocks_by_chain_length, epoch,
        last_block, settings, stake_pool, transaction_by_id, Address, AllBlocks, AllStakePools,
        AllVotePlans, BlocksByChainLength, Epoch, LastBlock, Settings, StakePool, TransactionById,
    },
};
use graphql_client::GraphQLQuery;
use graphql_client::*;
use jormungandr_lib::crypto::hash::Hash;
use jormungandr_lib::interfaces::BlockDate;
use std::{
    process::{Command, Stdio},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
mod client;
// Macro here expand to something containing PUBLIC/PRIVATE fields that
// do not respect the naming convention
#[allow(clippy::upper_case_acronyms)]
mod data;
mod wrappers;

pub use wrappers::LastBlockResponse;

use data::PoolId;
use jortestkit::{file, process::Wait};
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

use super::get_available_port;

#[derive(Error, Debug)]
pub enum ExplorerError {
    #[error("graph client error")]
    ClientError(#[from] client::GraphQlClientError),
    #[error("json serializiation error")]
    SerializationError(#[from] serde_json::Error),
    #[error("request error")]
    ReqwestError(#[from] reqwest::Error),
}

#[derive(Clone)]
pub struct Explorer {
    client: GraphQlClient,
    print_log: bool,
    _process: Arc<ExplorerProcess>,
}

struct ExplorerProcess {
    handler: Option<std::process::Child>,
    logs_dir: Option<std::path::PathBuf>,
}

impl Drop for ExplorerProcess {
    fn drop(&mut self) {
        let output = if let Some(mut handler) = self.handler.take() {
            let _ = handler.kill();
            handler.wait_with_output().unwrap()
        } else {
            return;
        };

        if std::thread::panicking() {
            if let Some(logs_dir) = &self.logs_dir {
                println!(
                    "persisting explorer logs after panic: {}",
                    logs_dir.display()
                );

                std::fs::write(logs_dir.join("explorer.log"), output.stdout)
                    .unwrap_or_else(|e| eprint!("Could not write explorer logs to disk: {}", e));
            }
        }
    }
}

impl Explorer {
    pub fn new(node_address: String, logs_dir: Option<std::path::PathBuf>) -> Explorer {
        let print_log = true;

        let path = get_explorer_app();
        let explorer_port = get_available_port();
        let explorer_listen_address = format!("127.0.0.1:{}", explorer_port);

        let _process = Arc::new(ExplorerProcess {
            handler: Some(
                Command::new(path)
                    .args(&[
                        "--node",
                        node_address.as_ref(),
                        "--binding-address",
                        explorer_listen_address.as_ref(),
                        "--log-output",
                        "stdout",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("failed to execute explorer process"),
            ),
            logs_dir,
        });

        let mut wait_bootstrap = Wait::new(Duration::from_secs(1), 10);
        while !wait_bootstrap.timeout_reached() {
            if reqwest::blocking::Client::new()
                .head(format!("http://{}/", &explorer_listen_address))
                .send()
                .is_ok()
            {
                break;
            };

            wait_bootstrap.advance();
        }

        Explorer {
            client: GraphQlClient::new(explorer_listen_address),
            print_log,
            _process,
        }
    }

    pub fn uri(&self) -> String {
        self.client.base_url()
    }

    pub fn disable_logs(&mut self) {
        self.print_log = false;
        self.client.disable_print();
    }

    pub fn enable_logs(&mut self) {
        self.print_log = true;
        self.client.enable_print();
    }

    pub fn print_request<T: Serialize>(&self, query: &QueryBody<T>) {
        if !self.print_log {
            return;
        }

        println!("running query: {:?}, against: {}", query.query, self.uri());
    }

    pub fn address<S: Into<String>>(
        &self,
        bech32_address: S,
    ) -> Result<Response<address::ResponseData>, ExplorerError> {
        let query = Address::build_query(address::Variables {
            bech32: bech32_address.into(),
        });
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body: Response<address::ResponseData> = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn stake_pools(
        &self,
        limit: i64,
    ) -> Result<Response<all_stake_pools::ResponseData>, ExplorerError> {
        let query = AllStakePools::build_query(all_stake_pools::Variables { first: limit });
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn blocks(&self, limit: i64) -> Result<Response<all_blocks::ResponseData>, ExplorerError> {
        let query = AllBlocks::build_query(all_blocks::Variables { last: limit });
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn last_block(&self) -> Result<LastBlockResponse, ExplorerError> {
        let query = LastBlock::build_query(last_block::Variables);
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body = response.json()?;
        self.print_log(&response_body);
        Ok(LastBlockResponse::new(response_body))
    }

    pub fn blocks_at_chain_length(
        &self,
        length: u32,
    ) -> Result<Response<blocks_by_chain_length::ResponseData>, ExplorerError> {
        let query = BlocksByChainLength::build_query(blocks_by_chain_length::Variables {
            length: length.to_string(),
        });
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn epoch(
        &self,
        epoch_number: u32,
        limit: i64,
    ) -> Result<Response<epoch::ResponseData>, ExplorerError> {
        let query = Epoch::build_query(epoch::Variables {
            id: epoch_number.to_string(),
            blocks_limit: limit,
        });
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn stake_pool(
        &self,
        id: PoolId,
        limit: i64,
    ) -> Result<Response<stake_pool::ResponseData>, ExplorerError> {
        let query = StakePool::build_query(stake_pool::Variables { id, first: limit });
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn settings(&self) -> Result<Response<settings::ResponseData>, ExplorerError> {
        let query = Settings::build_query(settings::Variables);
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn vote_plans(
        &self,
        limit: i64,
    ) -> Result<Response<all_vote_plans::ResponseData>, ExplorerError> {
        let query = AllVotePlans::build_query(all_vote_plans::Variables { first: limit });
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn transaction(
        &self,
        hash: Hash,
    ) -> Result<Response<transaction_by_id::ResponseData>, ExplorerError> {
        let query = TransactionById::build_query(transaction_by_id::Variables {
            id: hash.to_string(),
        });
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        let response_body: Response<transaction_by_id::ResponseData> = response.json()?;
        self.print_log(&response_body);
        Ok(response_body)
    }

    pub fn current_time(&self) -> BlockDate {
        self.last_block().unwrap().block_date()
    }

    pub fn run<T: Serialize>(
        &self,
        query: QueryBody<T>,
    ) -> Result<reqwest::blocking::Response, ExplorerError> {
        self.print_request(&query);
        let response = self.client.run(query).map_err(ExplorerError::ClientError)?;
        self.print_log(&response);
        Ok(response)
    }

    fn print_log<T: std::fmt::Debug>(&self, response: &T) {
        if self.print_log {
            println!("Response: {:?}", &response);
        }
    }
}

#[allow(dead_code)]
pub fn compare_schema<P: AsRef<Path>>(actual_schema_path: P) {
    let expected_schema_path =
        PathBuf::from_str("./jormungandr-automation/resources/explorer/graphql/schema.graphql")
            .unwrap();

    if !file::have_the_same_content(actual_schema_path.as_ref(), &expected_schema_path) {
        file::copy_file(actual_schema_path.as_ref(), &expected_schema_path, true);
        println!("discrepancies detected, already replaced file with new content. Please commit to update schema");
    }
}

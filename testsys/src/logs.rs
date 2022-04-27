use crate::error::{Error, Result};

use clap::Parser;
use futures::StreamExt;
use model::test_manager::{CrdName, SelectionParams, TestManager};

/// Restart an object from a testsys cluster.
#[derive(Debug, Parser)]
pub(crate) struct Logs {
    /// The name of the test we want logs from.
    #[clap(long, conflicts_with = "resource")]
    test: Option<String>,

    /// The name of the test we want logs from.
    #[clap(long, conflicts_with = "test")]
    resource: Option<String>,

    /// Include logs from dependencies.
    #[clap(long, short)]
    include_resources: bool,

    /// Follow logs
    #[clap(long, short)]
    follow: bool,
}

impl Logs {
    pub(crate) async fn run(self, client: TestManager) -> Result<()> {
        let crd = match (self.test, self.resource) {
            (Some(test), None) => CrdName::Test(test),
            (None, Some(resource)) => CrdName::Resource(resource),
            _ => return Err(Error::new_with_context("Invalid arguments were provided. Exactly on of `--test` and `--resource` must be used.")),
        };
        let mut logs = client
            .logs(
                &SelectionParams::Name(crd),
                self.include_resources,
                self.follow,
            )
            .await
            .unwrap();
        while let Some(line) = logs.next().await {
            println!("{}", line);
        }

        Ok(())
    }
}

use crate::{add_secret_image, add_secret_map, error::Result};
use clap::Parser;
use model::test_manager::TestManager;

/// Add a secret to the cluster.
#[derive(Debug, Parser)]
pub(crate) struct AddSecret {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Parser)]
enum Command {
    /// Create a secret for image pulls.
    Image(add_secret_image::AddSecretImage),
    /// Create a secret from key value pairs.
    Map(add_secret_map::AddSecretMap),
}

impl AddSecret {
    pub(crate) async fn run(self, client: TestManager) -> Result<()> {
        match self.command {
            Command::Image(add_secret_image) => add_secret_image.run(client).await,
            Command::Map(add_secret_map) => add_secret_map.run(client).await,
        }
    }
}

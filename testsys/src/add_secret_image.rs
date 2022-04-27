use crate::error::{IntoError, Result};
use clap::Parser;
use model::test_manager::TestManager;

/// Add a secret to the testsys cluster for image pulls.
#[derive(Debug, Parser)]
pub(crate) struct AddSecretImage {
    /// Controller image pull username
    #[clap(long, short = 'u')]
    pull_username: String,

    /// Controller image pull password
    #[clap(long, short = 'p')]
    pull_password: String,

    /// Image uri
    #[clap(long = "image-uri", short)]
    image_uri: String,

    /// Controller image uri
    #[clap(long, short = 'n')]
    secret_name: String,
}

impl AddSecretImage {
    pub(crate) async fn run(self, client: TestManager) -> Result<()> {
        client
            .create_image_pull_secret(
                &self.secret_name,
                &self.pull_username,
                &self.pull_password,
                &self.image_uri,
            )
            .await
            .context("Unable to create pull secret")?;

        println!("The secret was added.");

        Ok(())
    }
}

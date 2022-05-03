use crate::error::{IntoError, Result};
use clap::Parser;
use model::test_manager::{ImageConfig, TestManager};

/// The install subcommand is responsible for putting all of the necessary components for testsys in
/// a k8s cluster.
#[derive(Debug, Parser)]
pub(crate) struct Install {
    /// Controller image pull secret
    #[clap(long = "controller-image-secret", short = 's')]
    secret: Option<String>,

    /// Controller image uri
    // Todo add default controller_uri after images are published.
    #[clap(long = "controller-uri")]
    controller_uri: String,
}

impl Install {
    pub(crate) async fn run(self, client: TestManager) -> Result<()> {
        let controller_image = match (self.secret, self.controller_uri) {
            (Some(secret), image) => ImageConfig::WithCreds { secret, image },
            (None, image) => ImageConfig::Image(image),
        };
        client.install(controller_image).await.context(
            "Unable to install testsys to the cluster. (Some artifacts may be left behind)",
        )?;

        println!("testsys components were successfully installed.");

        Ok(())
    }
}

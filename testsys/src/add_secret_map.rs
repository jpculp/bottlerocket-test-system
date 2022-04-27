use crate::error::{IntoError, Result};
use clap::Parser;
use model::test_manager::TestManager;
use model::SecretName;

/// Add a `Secret` with key value pairs.
#[derive(Debug, Parser)]
pub(crate) struct AddSecretMap {
    /// Name of the secret
    #[clap(short, long)]
    name: SecretName,

    /// Key value pairs for secrets. (Key=value)
    #[clap(parse(try_from_str = parse_key_val))]
    args: Vec<(String, String)>,
}

impl AddSecretMap {
    pub(crate) async fn run(self, client: TestManager) -> Result<()> {
        client
            .create_secret(&self.name, self.args)
            .await
            .context("Unable to create secret")?;
        println!("Successfully added '{}' to secrets.", self.name);
        Ok(())
    }
}

fn parse_key_val(s: &str) -> Result<(String, String)> {
    let mut iter = s.splitn(2, '=');
    let key = iter.next().context("Key is missing")?;
    let value = iter.next().context("Value is missing")?;
    Ok((key.to_string(), value.to_string()))
}

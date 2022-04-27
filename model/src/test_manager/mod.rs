use crate::{Resource, Test};
pub use delete::DeleteEvent;
pub use error::{Error, Result};
use kube::ResourceExt;
pub use manager::{read_manifest, TestManager};
use serde::{Deserialize, Serialize};
pub use status::Status;
use std::collections::HashMap;

mod delete;
mod error;
mod install;
mod logs;
mod manager;
mod status;

/// `CrdName` provides a way of determing which type of testsys object a name refers to.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum CrdName {
    Test(String),
    Resource(String),
}

impl CrdName {
    pub fn name(&self) -> &String {
        match self {
            CrdName::Test(name) => name,
            CrdName::Resource(name) => name,
        }
    }
}

/// `Crd` provides an interface to combine `Test` and `Resource` when actions can be performed on both.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Crd {
    Test(Test),
    Resource(Resource),
}

impl Crd {
    pub fn name(&self) -> Option<String> {
        match self {
            Self::Test(test) => test.metadata.name.to_owned(),
            Self::Resource(resource) => resource.metadata.name.to_owned(),
        }
    }
}

impl From<Crd> for CrdName {
    fn from(crd: Crd) -> Self {
        match crd {
            Crd::Test(test) => CrdName::Test(test.name()),
            Crd::Resource(resource) => CrdName::Resource(resource.name()),
        }
    }
}

/// `SelectionParams` are used to select a group (or single) object from a testsys cluster.
pub enum SelectionParams {
    // TODO add field selectors (Think kube-rs `ListParams`)
    Label(String),
    Name(CrdName),
    All,
}

impl Default for SelectionParams {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Serialize)]
pub(crate) struct DockerConfigJson {
    auths: HashMap<String, DockerConfigAuth>,
}

#[derive(Serialize)]
struct DockerConfigAuth {
    auth: String,
}

impl DockerConfigJson {
    pub(crate) fn new(username: &str, password: &str, registry: &str) -> DockerConfigJson {
        let mut auths = HashMap::new();
        let auth = base64::encode(format!("{}:{}", username, password));
        auths.insert(registry.to_string(), DockerConfigAuth { auth });
        DockerConfigJson { auths }
    }
}

/// `ImageConfig` represents an image uri, and the name of a pull secret (if needed).
pub enum ImageConfig {
    WithCreds { image: String, secret: String },
    Image(String),
}

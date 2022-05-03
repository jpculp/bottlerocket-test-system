/*!

This library provides the Kubernetes custom resource definitions and their API clients.

!*/

#![deny(
    clippy::expect_used,
    clippy::get_unwrap,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::panicking_unwrap,
    clippy::unwrap_in_result,
    clippy::unwrap_used
)]

pub use agent::{Agent, SecretName, SecretType, TaskState};
pub use configuration::Configuration;
pub use crd_ext::CrdExt;
pub use error::{Error, Result};
pub use resource::{
    DestructionPolicy, ErrorResources, Resource, ResourceAction, ResourceError, ResourceSpec,
    ResourceStatus,
};
pub use test::{
    AgentStatus, ControllerStatus, Outcome, Test, TestResults, TestSpec, TestStatus, TestUserState,
};

mod agent;
pub mod clients;
mod configuration;
pub mod constants;
mod crd_ext;
mod error;
mod resource;
mod schema_utils;
pub mod system;
mod test;
pub mod test_manager;

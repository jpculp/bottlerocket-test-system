use snafu::Snafu;
use std::string::FromUtf8Error;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
#[allow(clippy::large_enum_variant)]
pub enum Error {
    AssumeRole {
        role_arn: String,
        source: aws_sdk_sts::error::SdkError<aws_sdk_sts::operation::assume_role::AssumeRoleError>,
    },

    #[snafu(display(
        "Failed to attach policy '{}' to role '{}': {}",
        policy_arn,
        role_name,
        source
    ))]
    AttachRolePolicy {
        role_name: String,
        policy_arn: String,
        source: aws_sdk_iam::error::SdkError<
            aws_sdk_iam::operation::attach_role_policy::AttachRolePolicyError,
        >,
    },

    #[snafu(display("Failed to decode base64 blob: {}", source))]
    Base64Decode { source: base64::DecodeError },

    #[snafu(display("Unable to build instance information string filter: {}", source))]
    BuildInstanceInformationStringFilter {
        source: aws_sdk_ssm::error::BuildError,
    },

    #[snafu(display("Unable to build tag: {}", source))]
    BuildTag {
        source: aws_sdk_ssm::error::BuildError,
    },

    #[snafu(display("Could not convert '{}' secret to string: {}", what, source))]
    Conversion { what: String, source: FromUtf8Error },

    #[snafu(display("Failed to send create SSM command: {}", source))]
    CreateSsmActivation {
        source: aws_sdk_ssm::error::SdkError<
            aws_sdk_ssm::operation::create_activation::CreateActivationError,
        >,
    },

    #[snafu(display(
        "Unable to create role '{}' with policy '{}': {}",
        role_name,
        role_policy,
        source
    ))]
    CreateRole {
        role_name: String,
        role_policy: String,
        source: aws_sdk_iam::error::SdkError<aws_sdk_iam::operation::create_role::CreateRoleError>,
    },

    #[snafu(display("Credentials were missing for assumed role '{}'", role_arn))]
    CredentialsMissing { role_arn: String },

    #[snafu(display("Failed to setup environment variables: {}", what))]
    EnvSetup { what: String },

    #[snafu(display("Unable to get managed instance information: {}", source))]
    GetManagedInstanceInfo {
        source: aws_sdk_ssm::error::SdkError<
            aws_sdk_ssm::operation::describe_instance_information::DescribeInstanceInformationError,
        >,
    },

    #[snafu(display("Unable to get SSM role '{}': {}", role_name, source))]
    GetSSMRole {
        role_name: String,
        source: aws_sdk_iam::error::SdkError<aws_sdk_iam::operation::get_role::GetRoleError>,
    },

    #[snafu(display("{} was missing from {}", what, from))]
    Missing { what: String, from: String },

    #[snafu(display("Secret was missing: {}", source))]
    SecretMissing {
        source: agent_common::secrets::Error,
    },

    #[snafu(display("Failed to write file at '{}': {}", path, source))]
    WriteFile {
        path: String,
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

use super::{
    error, Crd, CrdName, DeleteEvent, DockerConfigJson, ImageConfig, Result, SelectionParams,
    Status,
};
use crate::clients::{AllowNotFound, CrdClient, ResourceClient, TestClient};
use crate::constants::{LABEL_COMPONENT, NAMESPACE, TESTSYS_RESULTS_FILE};
use crate::system::AgentType;
use crate::{Resource, SecretName, Test};
use futures::{Stream, StreamExt};
use k8s_openapi::api::core::v1::{Pod, Secret};
use kube::api::{ListParams, Patch, PatchParams, PostParams};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Config, Resource as KubeResource, ResourceExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use snafu::{OptionExt, ResultExt};
use std::fmt::Debug;
use std::{collections::BTreeMap, path::Path};
use tokio::io::AsyncWriteExt;

/// # Test Manager
///
/// The test manager provides operations for coordinating the creation, operation, deletion and
/// observation of `Test` and `Resource` objects. It understands the dependencies between `Test`s
/// and `Resource`s.
///
/// # Operations
///
/// Here are some of the things that you can do with the test manager:
/// - Delete a test and all of the tests and resources it depends on
/// - Get the logs from a test pod or one of its resource agent pods
/// - Get a structured summary of test results from multiple tests
///
/// # Clients
///
/// For direct, lower-level operations on the `Test` and `Resource` objects themselves, you can use
/// the [`TestClient`] and [`ResourceClient`] objects. These clients can be constructed
/// independently or obtained from the `TestManager` using `test_client()` and `resource_client()`
/// functions.
///
pub struct TestManager {
    pub k8s_client: Client,
}

impl TestManager {
    /// Create a `TestManager` from the path to a kubeconfig file.
    pub async fn new_from_kubeconfig_path(kubeconfig_path: &Path) -> Result<Self> {
        let kubeconfig = Kubeconfig::read_from(kubeconfig_path).context(error::ConfigReadSnafu)?;
        let config = Config::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions::default())
            .await
            .context(error::ClientCreateKubeconfigSnafu)?;
        Ok(TestManager {
            k8s_client: config.try_into().context(error::KubeSnafu {
                action: "create client from `Kubeconfig`",
            })?,
        })
    }

    /// Create a `TestManager` using the default `kube::Client`.
    pub async fn new() -> Result<Self> {
        Ok(TestManager {
            k8s_client: Client::try_default().await.context(error::KubeSnafu {
                action: "create client from `Kubeconfig`",
            })?,
        })
    }

    /// Create a `TestClient`
    pub fn test_client(&self) -> TestClient {
        TestClient::new_from_k8s_client(self.k8s_client.clone())
    }

    /// Create a `ResourceClient`
    pub fn resource_client(&self) -> ResourceClient {
        ResourceClient::new_from_k8s_client(self.k8s_client.clone())
    }

    /// Create a secret for image pulls using `DockerConfigJson`
    pub async fn create_image_pull_secret(
        &self,
        name: &str,
        username: &str,
        password: &str,
        image_url: &str,
    ) -> Result<Secret> {
        // Create docker config json for the image pull secret.
        let sec_str =
            serde_json::to_string_pretty(&DockerConfigJson::new(username, password, image_url))
                .context(error::JsonSerializeSnafu)?;
        let mut secret_tree = BTreeMap::new();
        secret_tree.insert(".dockerconfigjson".to_string(), sec_str);

        let object_meta = kube::api::ObjectMeta {
            name: Some(name.to_string()),
            ..Default::default()
        };

        // Create the secret we are going to add.
        let secret = Secret {
            data: None,
            immutable: None,
            metadata: object_meta,
            string_data: Some(secret_tree),
            type_: Some("kubernetes.io/dockerconfigjson".to_string()),
        };

        self.create_or_update(true, &secret, "controller pull secret")
            .await?;
        Ok(secret)
    }

    pub async fn create_secret<I>(&self, name: &SecretName, data: I) -> Result<Secret>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let object_meta = kube::api::ObjectMeta {
            name: Some(name.to_string()),
            ..Default::default()
        };

        // Create the secret we are going to add.
        let secret = Secret {
            data: None,
            immutable: None,
            metadata: object_meta,
            string_data: Some(data.into_iter().collect()),
            type_: None,
        };
        self.create_or_update(true, &secret, "secret").await?;
        Ok(secret)
    }

    /// Install testsys to a cluster.
    pub async fn install(&self, controller_config: ImageConfig) -> Result<()> {
        self.create_namespace().await?;
        self.create_crd().await?;
        self.create_roles(AgentType::Test).await?;
        self.create_roles(AgentType::Resource).await?;
        self.create_service_accts(AgentType::Test).await?;
        self.create_service_accts(AgentType::Resource).await?;
        self.create_controller_service_acct().await?;

        // Add the controller to the cluster
        let (image, secret) = match controller_config {
            ImageConfig::WithCreds { secret, image } => (image, Some(secret)),
            ImageConfig::Image(image) => (image, None),
        };
        self.create_deployment(image, secret).await?;

        Ok(())
    }

    /// Restart a crd object by deleting the crd from the cluster and adding a copy of it with its status cleared.
    pub async fn restart_test(&self, name: &str) -> Result<()> {
        let test_client = TestClient::new_from_k8s_client(self.k8s_client.clone());
        let mut test = test_client
            .get(name)
            .await
            .context(error::ClientSnafu { action: "get test" })?;
        // Created objects are not allowed to have `resource_version` set.
        test.metadata.resource_version = None;
        test.status = None;
        test_client.delete(name).await.context(error::ClientSnafu {
            action: "delete test",
        })?;
        test_client.wait_for_deletion(name).await;
        test_client.create(test).await.context(error::ClientSnafu {
            action: "create new test",
        })?;
        Ok(())
    }

    /// Add a testsys crd (`Test`, `Resource`) to the cluster.
    pub async fn create_object(&self, crd: Crd) -> Result<Crd> {
        match &crd {
            Crd::Test(test) => self.create_test(test.clone()).await?,
            Crd::Resource(resource) => self.create_resource(resource.clone()).await?,
        }
        Ok(crd)
    }

    /// List all testsys objects following `SelectionParams`
    pub async fn list(&self, selection_params: &SelectionParams) -> Result<Vec<Crd>> {
        Ok(match selection_params {
            SelectionParams::All => {
                let mut objects = Vec::new();
                let list_params = Default::default();
                let tests = self.test_client().api().list(&list_params).await.context(
                    error::KubeSnafu {
                        action: "list tests from label params",
                    },
                )?;
                objects.extend(tests.into_iter().map(Crd::Test));
                let resources = self
                    .resource_client()
                    .api()
                    .list(&list_params)
                    .await
                    .context(error::KubeSnafu {
                        action: "list resources from label params",
                    })?;
                objects.extend(resources.into_iter().map(Crd::Resource));
                objects
            }
            SelectionParams::Label(label) => {
                let mut objects = Vec::new();
                let list_params = ListParams {
                    label_selector: Some(label.to_string()),
                    ..Default::default()
                };
                let tests = self.test_client().api().list(&list_params).await.context(
                    error::KubeSnafu {
                        action: "list tests from label params",
                    },
                )?;
                objects.extend(tests.into_iter().map(Crd::Test));
                let resources = self
                    .resource_client()
                    .api()
                    .list(&list_params)
                    .await
                    .context(error::KubeSnafu {
                        action: "list resources from label params",
                    })?;
                objects.extend(resources.into_iter().map(Crd::Resource));
                objects
            }
            SelectionParams::Name(CrdName::Test(test_name)) => {
                vec![Crd::Test(
                    self.test_client()
                        .get(test_name)
                        .await
                        .context(error::ClientSnafu { action: "get test" })?,
                )]
            }
            SelectionParams::Name(CrdName::Resource(resource_name)) => {
                vec![Crd::Resource(
                    self.resource_client().get(resource_name).await.context(
                        error::ClientSnafu {
                            action: "get resource",
                        },
                    )?,
                )]
            }
        })
    }

    /// Delete all testsys `Test`s and `Resource`s from a cluster.
    pub async fn delete_all(&self) -> Result<impl Stream<Item = Result<DeleteEvent>>> {
        let deletion_order = self.all_objects_deletion_order().await?;
        Ok(self.delete_sorted_resources(deletion_order))
    }

    /// Delete resources from a testsys cluster based on `SelectionParams`. If `include_dependencies` all objects
    /// that each item depends on will also be deleted.
    pub async fn delete(
        &self,
        selection_params: &SelectionParams,
        include_dependencies: bool,
    ) -> Result<impl Stream<Item = Result<DeleteEvent>>> {
        let mut objects = self.list(selection_params).await?;
        if include_dependencies {
            objects = self.add_dependencies_to_vec(objects).await?;
        }
        Ok(self.delete_sorted_resources(Self::vec_to_deletion_order(objects)))
    }

    /// Delete the resource after a failed deletion attempt.
    /// Warning: the physical resources may not be deleted.
    /// /// The finalizers will be removed from the resource and the resource will be deleted.
    /// The k8s job for resource deletion will also be deleted.
    /// This should only be used if a resource has already failed to delete.
    /// All tests will be deleted normally.
    pub async fn force_delete_resource(&self, selection_params: &SelectionParams) -> Result<()> {
        let objects = self.list(selection_params).await?;
        for object in objects {
            match object {
                Crd::Test(test) => {
                    self.test_client()
                        .delete(test.name())
                        .await
                        .context(error::ClientSnafu {
                            action: "delete test",
                        })?;
                }
                Crd::Resource(resource) => {
                    self.resource_client()
                        .force_delete(resource.name())
                        .await
                        .context(error::ClientSnafu {
                            action: "delete test",
                        })?;
                }
            };
        }
        Ok(())
    }

    /// Collect the status of all testsys objects meeting `selection_params`. If `include_controller` the status of the controller
    /// will also be recorded. The `Status` returned can be used to print a table containing each objects status (including rerun tests)
    /// or to print a json representation containing all included objects as well as the controller status.
    pub async fn status(
        &self,
        selection_params: &SelectionParams,
        include_controller: bool,
    ) -> Result<Status> {
        let controller_status = if include_controller {
            let pod_api: Api<Pod> = self.namespaced_api();
            let pods = pod_api
                .list(&ListParams {
                    label_selector: Some(format!("{}={}", LABEL_COMPONENT, "controller")),
                    ..Default::default()
                })
                .await
                .context(error::KubeSnafu {
                    action: "get controller pod",
                })?
                .items;
            if let Some(pod) = pods.first() {
                pod.status.clone()
            } else {
                None
            }
        } else {
            None
        };
        let crds = self.list(selection_params).await?;

        Ok(Status::new(controller_status, crds))
    }

    /// Retrieve the logs of all testsys objects meeting `selection_params`. If `include_resources` all dependencies of the objects
    /// will also be included in the logs.
    pub async fn logs(
        &self,
        selection_params: &SelectionParams,
        include_dependencies: bool,
        follow: bool,
    ) -> Result<impl Stream<Item = String>> {
        let mut objects = self.list(selection_params).await?;
        if include_dependencies {
            objects = self.add_dependencies_to_vec(objects).await?;
        }
        self.stream_logs(
            objects.into_iter().map(|object| object.into()).collect(),
            follow,
        )
        .await
    }

    /// Write the results from a testsys `Test` to a given `destination`. The results are in the form of a tarball containing all files placed in
    /// the test agents output directory.
    pub async fn write_test_results(&self, test_name: &str, destination: &Path) -> Result<()> {
        let pod_name = self
            .get_pods(&CrdName::Test(test_name.to_string()))
            .await?
            .pop()
            .context(error::NotFoundSnafu { what: "test" })?
            .name();

        let pods: Api<Pod> = self.namespaced_api();
        let mut cat = pods
            .exec(
                &pod_name,
                vec!["cat", TESTSYS_RESULTS_FILE],
                &Default::default(),
            )
            .await
            .context(error::KubeSnafu {
                action: "get results tarball",
            })?;
        let mut cat_out =
            tokio_util::io::ReaderStream::new(cat.stdout().context(error::NotFoundSnafu {
                what: "results stdout",
            })?);

        let mut out_file = tokio::fs::File::create(destination)
            .await
            .context(error::FileSnafu { path: destination })?;
        while let Some(data) = cat_out.next().await {
            out_file
                .write(&data.context(error::IoSnafu {
                    action: "get results line",
                })?)
                .await
                .context(error::IoSnafu {
                    action: "write results",
                })?;
        }
        out_file.flush().await.context(error::IoSnafu {
            action: "flush results",
        })?;
        Ok(())
    }

    // =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^=

    /// Retry attempts for creating or updating an object.
    const MAX_RETRIES: i32 = 3;
    /// Timeout for object creation/update retries.
    const BACKOFF_MS: u64 = 500;

    /// Create or update an existing k8s object
    pub(super) async fn create_or_update<T>(
        &self,
        namespaced: bool,
        data: &T,
        what: &str,
    ) -> Result<()>
    where
        T: KubeResource + Clone + DeserializeOwned + Serialize + Debug,
        <T as KubeResource>::DynamicType: Default,
    {
        let mut error = None;

        for _ in 0..Self::MAX_RETRIES {
            match self.create_or_update_internal(namespaced, data, what).await {
                Ok(()) => return Ok(()),
                Err(e) => error = Some(e),
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(Self::BACKOFF_MS)).await;
        }
        match error {
            None => Ok(()),
            Some(error) => Err(error),
        }
    }

    async fn create_or_update_internal<T>(
        &self,
        namespaced: bool,
        data: &T,
        what: &str,
    ) -> Result<()>
    where
        T: KubeResource + Clone + DeserializeOwned + Serialize + Debug,
        <T as KubeResource>::DynamicType: Default,
    {
        let api = if namespaced {
            self.namespaced_api::<T>()
        } else {
            self.api::<T>()
        };
        // If the data already exists, update it with the new one using a `Patch`. If not create a new one.
        match api.get(&data.name()).await {
            Ok(deployment) => {
                api.patch(
                    &deployment.name(),
                    &PatchParams::default(),
                    &Patch::Merge(data),
                )
                .await
            }
            Err(_err) => api.create(&PostParams::default(), data).await,
        }
        .context(error::CreateSnafu { what })?;

        Ok(())
    }

    /// Creates a non namespaced api of type `T`
    pub(super) fn api<T>(&self) -> Api<T>
    where
        T: KubeResource,
        <T as KubeResource>::DynamicType: Default,
    {
        Api::<T>::all(self.k8s_client.clone())
    }

    /// Creates a namespaced api of type `T`
    pub(super) fn namespaced_api<T>(&self) -> Api<T>
    where
        T: KubeResource,
        <T as KubeResource>::DynamicType: Default,
    {
        Api::<T>::namespaced(self.k8s_client.clone(), NAMESPACE)
    }

    /// Returns a list containing all dependencies for each object in a `Vec<Crd>` including the objects themselves
    async fn add_dependencies_to_vec(&self, objects: Vec<Crd>) -> Result<Vec<Crd>> {
        let mut dependencies = Vec::new();
        let mut to_be_visited = objects;
        while let Some(crd) = to_be_visited.pop() {
            dependencies.push(crd.clone());
            let resources = match crd {
                Crd::Test(test) => test.spec.resources,
                Crd::Resource(resource) => resource.spec.depends_on.unwrap_or_default(),
            };
            for resource in resources {
                if let Some(resource_spec) = self
                    .resource_client()
                    .get(resource)
                    .await
                    .allow_not_found(|_| ())
                    .context(error::ClientSnafu {
                        action: "get resource",
                    })?
                {
                    to_be_visited.push(Crd::Resource(resource_spec));
                }
            }
        }

        Ok(dependencies)
    }

    /// Get all pods in a cluster that are doing work for a testsys crd.
    pub(super) async fn get_pods(&self, crd: &CrdName) -> Result<Vec<Pod>> {
        let pod_api: Api<Pod> = self.namespaced_api();
        Ok(match crd {
            CrdName::Test(test) => {
                pod_api
                    .list(&ListParams {
                        label_selector: Some(format!("job-name={}", test)),
                        ..Default::default()
                    })
                    .await
                    .context(error::KubeSnafu { action: "get pods" })?
                    .items
            }
            CrdName::Resource(resource) => {
                let mut pods = Vec::new();
                pods.append(
                    &mut pod_api
                        .list(&ListParams {
                            label_selector: Some(format!("job-name={}-creation", resource)),
                            ..Default::default()
                        })
                        .await
                        .context(error::KubeSnafu { action: "get pods" })?
                        .items,
                );
                pods.append(
                    &mut pod_api
                        .list(&ListParams {
                            label_selector: Some(format!("job-name={}-destruction", resource)),
                            ..Default::default()
                        })
                        .await
                        .context(error::KubeSnafu { action: "get pods" })?
                        .items,
                );
                pods
            }
        })
    }

    /// Add a testsys test to the cluster.
    async fn create_test(&self, test: Test) -> Result<()> {
        let test_client = self.test_client();
        test_client.create(test).await.context(error::ClientSnafu {
            action: "create new test",
        })?;
        Ok(())
    }

    /// Add a testsys resource to the cluster.
    async fn create_resource(&self, resource: Resource) -> Result<()> {
        let resource_client = self.resource_client();
        resource_client
            .create(resource)
            .await
            .context(error::ClientSnafu {
                action: "create new resource",
            })?;
        Ok(())
    }
}

/// Takes a path to a yaml manifest of testsys crds (`Test` and `Resource`) and creates a set of `Crd`s through deserialization.
/// These can be added using `TestManager::create_object`
pub fn read_manifest(path: &Path) -> Result<Vec<Crd>> {
    let mut crds = Vec::new();
    // Create the resource objects from its path.
    let manifest_string = std::fs::read_to_string(path).context(error::FileSnafu { path })?;
    for crd_doc in serde_yaml::Deserializer::from_str(&manifest_string) {
        let value = serde_yaml::Value::deserialize(crd_doc).context(error::SerdeYamlSnafu {
            action: "deserialize manifest",
        })?;
        let crd: Crd = serde_yaml::from_value(value).context(error::SerdeYamlSnafu {
            action: "deserialize manifest",
        })?;
        crds.push(crd);
    }
    Ok(crds)
}

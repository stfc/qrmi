// This code is part of Qiskit.
//
// (C) Copyright IBM 2026
// (C) Copyright UKRI-STFC (Hartree Centre) 2026
//
// This code is licensed under the Apache License, Version 2.0. You may
// obtain a copy of this license in the LICENSE.txt file in the root directory
// of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
//
// Any modifications or derivative works of this code must retain this
// copyright notice, and modified files need to carry a notice indicating
// that they have been altered from the originals.

//! [`ResourceProvider`] implementation for IBM Qiskit Runtime Service.

mod provider_filter;

use crate::ibm::models::BackendConfiguration;
use crate::ibm::IBMQuantumComputeService;
use crate::models::{AccountUsage, UsageMetric, ACCOUNT_USAGE_SCHEMA_VERSION};
use crate::resource_provider::ResourceProvider;
use crate::{QrmiError, QuantumResource, Result};
use anyhow::Context;
use async_trait::async_trait;
use futures::future::join_all;
use log::warn;
use provider_filter::BackendFilter;
use quantum_compute_client::apis::{auth, backends_api, configuration, instances_api};
use quantum_compute_client::models as compute_models;
use std::collections::HashMap;
use std::env;

/// A [`ResourceProvider`] that discovers backends available through IBM Quantum Compute
/// Service.
///
/// Constructed from a [`ResourceDef`] with `is_dynamic: true`.
///
/// # Config file example
///
/// ```json
/// {
///     "resources": [
///         {
///             "name": "ibm_inst1",
///             "type": "ibm-quantum-compute-service",
///             "is_dynamic": true,
///             "environment": {
///                 "QRMI_IBM_QCS_ENDPOINT":     "https://quantum.cloud.ibm.com/api/v1",
///                 "QRMI_IBM_QCS_IAM_ENDPOINT": "https://iam.cloud.ibm.com",
///                 "QRMI_IBM_QCS_IAM_APIKEY":   "my_apikey",
///                 "QRMI_IBM_QCS_SERVICE_CRN":  "my_instance"
///             }
///         }
///     ]
/// }
/// ```
pub struct IBMQuantumComputeServiceProvider {
    config: configuration::Configuration,
    api_key: String,
    iam_endpoint: String,
    provider_env: HashMap<String, String>,
}

impl IBMQuantumComputeServiceProvider {
    /// Constructs a new provider from an environment variable map.
    ///
    /// # Required keys
    ///
    /// - `QRMI_IBM_QCS_ENDPOINT`
    /// - `QRMI_IBM_QCS_IAM_ENDPOINT`
    /// - `QRMI_IBM_QCS_IAM_APIKEY`
    /// - `QRMI_IBM_QCS_SERVICE_CRN`
    pub fn new(environment: &HashMap<String, String>) -> Result<Self> {
        let get = |key: &str| -> Result<String> {
            environment
                .get(key)
                .cloned()
                .ok_or_else(|| QrmiError::MissingConfigKey(key.to_string()))
        };

        let qrs_endpoint = get("QRMI_IBM_QCS_ENDPOINT")?;
        let iam_endpoint = get("QRMI_IBM_QCS_IAM_ENDPOINT")?;
        let api_key = get("QRMI_IBM_QCS_IAM_APIKEY")?;
        let service_crn = get("QRMI_IBM_QCS_SERVICE_CRN")?;

        let mut config = configuration::Configuration::new();
        config.base_path = qrs_endpoint;
        config.bearer_access_token = None;
        config.crn = Some(service_crn);

        Ok(Self {
            config,
            api_key,
            iam_endpoint,
            provider_env: environment.clone(),
        })
    }

    fn inject_backend_env(&self, backend_name: &str) {
        for (key, value) in &self.provider_env {
            env::set_var(format!("{backend_name}_{key}"), value);
        }
    }

    async fn is_backend_online(config: &configuration::Configuration, name: &str) -> bool {
        match backends_api::get_backend_status(config, name, None).await {
            Ok(resp) => resp
                .status
                .as_deref()
                .map(BackendFilter::is_online_status)
                .unwrap_or(false),
            Err(e) => {
                warn!("Failed to get status for backend {:?}: {:?}", name, e);
                false
            }
        }
    }
}

fn account_usage_from_instance_usage(
    instance_usage: &compute_models::GetUsage200Response,
) -> AccountUsage {
    let mut usage = vec![UsageMetric {
        name: "ibm.instance.usage_consumed".to_string(),
        value: instance_usage.usage_consumed_seconds,
        unit: "seconds".to_string(),
        semantics: "IBM Quantum Compute Service usage consumed in the current service-instance usage period."
            .to_string(),
    }];

    if let Some(value) = instance_usage.usage_limit_seconds {
        usage.push(UsageMetric {
            name: "ibm.instance.usage_limit".to_string(),
            value,
            unit: "seconds".to_string(),
            semantics: "IBM Quantum Compute Service usage limit during the current service-instance usage period."
                .to_string(),
        });
    }
    if let Some(value) = instance_usage.usage_allocation_seconds {
        usage.push(UsageMetric {
            name: "ibm.instance.usage_allocation".to_string(),
            value,
            unit: "seconds".to_string(),
            semantics: "IBM Quantum Compute Service usage allocation for the configured service instance during the current usage period."
                .to_string(),
        });
    }

    let (period_start, period_end) = instance_usage
        .usage_period
        .as_deref()
        .map(|period| (period.start_time.clone(), period.end_time.clone()))
        .unwrap_or((None, None));

    let mut provider_metadata = std::collections::BTreeMap::new();
    provider_metadata.insert("ibm.plan_id".to_string(), instance_usage.plan_id.clone());
    if let Some(value) = instance_usage.usage_limit_reached {
        provider_metadata.insert("ibm.usage_limit_reached".to_string(), value.to_string());
    }
    if let Some(value) = instance_usage.time_available_at.as_ref() {
        provider_metadata.insert("ibm.time_available_at".to_string(), value.clone());
    }

    AccountUsage {
        schema_version: ACCOUNT_USAGE_SCHEMA_VERSION,
        provider_type: "ibm-quantum-compute-service".to_string(),
        account_id: instance_usage.instance_id.clone(),
        period_start,
        period_end,
        usage,
        provider_metadata,
    }
}
#[async_trait]
impl ResourceProvider for IBMQuantumComputeServiceProvider {
    async fn account_usage(&self) -> Result<AccountUsage> {
        let mut config = self.config.clone();
        let mut token_expiration: u64 = 0;
        let mut token_lifetime: u64 = 0;

        auth::check_token(
            &self.api_key,
            &self.iam_endpoint,
            &mut config.bearer_access_token,
            &mut token_expiration,
            &mut token_lifetime,
        )
        .await
        .map_err(|err| {
            QrmiError::Other(anyhow::anyhow!(
                "failed to authenticate before retrieving IBM Quantum Compute Service account usage: {err:#}"
            ))
        })?;

        let response = instances_api::get_usage(&config, Some("2025-01-01"))
            .await
            .map_err(|err| {
                QrmiError::Other(anyhow::anyhow!(
                    "failed to retrieve IBM Quantum Compute Service service-instance account usage: {err:?}"
                ))
            })?;

        Ok(account_usage_from_instance_usage(&response))
    }

    /// Returns backends available through IBM Quantum Compute Service,
    /// filtered and sorted by `queue_length` (ascending).
    ///
    /// # Filter string format
    ///
    /// `key=value` pairs joined by `&`. Supported keys:
    ///
    /// - `num_qubits=<N>`      — only backends with `n_qubits >= N`
    /// - `max_shots=<N>`       — only backends with `max_shots >= N`
    /// - `name=<glob>`         — only backends whose name matches the glob pattern
    /// - `is_simulator=<bool>` — include/exclude simulators (default: `false`)
    /// - `status=online`       — only online backends
    ///
    /// Example: `Some("num_qubits=127&name=ibm_*&status=online")`
    ///
    /// # Sorting
    ///
    /// Results are sorted by `queue_length` ascending (least busy first).
    async fn resources(
        &self,
        filters: Option<String>,
    ) -> Result<Vec<Box<dyn QuantumResource + Send + Sync>>> {
        let filter = BackendFilter::parse(filters.as_deref().unwrap_or(""))?;

        let mut config = self.config.clone();
        let mut token_expiration: u64 = 0;
        let mut token_lifetime: u64 = 0;

        auth::check_token(
            &self.api_key,
            &self.iam_endpoint,
            &mut config.bearer_access_token,
            &mut token_expiration,
            &mut token_lifetime,
        )
        .await
        .context("token renewal failed")?;

        let response = backends_api::list_backends(&config, Some("2025-01-01"))
            .await
            .context("failed to list backends")?;

        // Apply name and is_simulator filters at list stage, keep queue_length for sorting.
        let candidates: Vec<_> = response
            .devices
            .unwrap_or_default()
            .into_iter()
            .filter(|d| filter.matches_device(d))
            .filter(|d| filter.matches_name(&d.name))
            .collect();

        // Apply async status filter if requested.
        let candidates = if filter.needs_status_check() {
            let checks: Vec<_> = candidates
                .iter()
                .map(|d| Self::is_backend_online(&config, &d.name))
                .collect();
            let results = join_all(checks).await;
            candidates
                .into_iter()
                .zip(results)
                .filter_map(|(d, online)| if online { Some(d) } else { None })
                .collect::<Vec<_>>()
        } else {
            candidates
        };

        // Fetch BackendConfiguration for each candidate in parallel.
        let config_futures: Vec<_> = candidates
            .iter()
            .map(|d| backends_api::get_backend_configuration(&config, &d.name, Some("2025-01-01")))
            .collect();

        let configs = join_all(config_futures).await;

        // Sort by queue_length ascending before constructing resources.
        let mut candidates_with_config: Vec<_> = candidates.into_iter().zip(configs).collect();

        candidates_with_config.sort_by_key(|(d, _)| d.queue_length);

        let resources: Vec<Box<dyn QuantumResource + Send + Sync>> = candidates_with_config
            .into_iter()
            .filter_map(|(device, config_result)| {
                let raw = match config_result {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Skipping backend {:?}: get_backend_configuration failed: {:?}", device.name, e);
                        return None;
                    }
                };

                let backend_config: BackendConfiguration = match serde_json::from_value(
                    serde_json::to_value(&raw).unwrap_or_default()
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(
                            "Skipping backend {:?}: failed to deserialize BackendConfiguration: {:?}\nRaw: {:?}",
                            device.name, e, raw
                        );
                        return None;
                    }
                };

                if !filter.matches_config(&backend_config) {
                    return None;
                }

                self.inject_backend_env(&device.name);

                match IBMQuantumComputeService::new(&device.name) {
                    Ok(r) => Some(Box::new(r) as Box<dyn QuantumResource + Send + Sync>),
                    Err(e) => {
                        warn!(
                            "Skipping backend {:?}: failed to construct IBMQuantumComputeService: {:?}",
                            device.name, e
                        );
                        None
                    }
                }
            })
            .collect();

        Ok(resources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ibm_instance_usage_maps_to_account_usage() {
        let mut provider_usage = compute_models::GetUsage200Response::new(
            "instance-123".to_string(),
            "plan-1".to_string(),
            12.0,
        );
        provider_usage.usage_period =
            Some(Box::new(compute_models::GetUsage200ResponseUsagePeriod {
                start_time: Some("2026-09-01T00:00:00Z".to_string()),
                end_time: Some("2026-09-29T00:00:00Z".to_string()),
            }));
        provider_usage.usage_limit_seconds = Some(60.0);
        provider_usage.usage_allocation_seconds = Some(120.0);
        provider_usage.usage_limit_reached = Some(true);
        provider_usage.time_available_at = Some("2026-09-14T12:00:00Z".to_string());

        let record = account_usage_from_instance_usage(&provider_usage);

        assert_eq!(record.schema_version, ACCOUNT_USAGE_SCHEMA_VERSION);
        assert_eq!(record.provider_type, "ibm-quantum-compute-service");
        assert_eq!(record.account_id, "instance-123");
        assert_eq!(record.period_start.as_deref(), Some("2026-09-01T00:00:00Z"));
        assert_eq!(record.period_end.as_deref(), Some("2026-09-29T00:00:00Z"));
        assert_eq!(record.usage.len(), 3);
        assert_eq!(
            record
                .provider_metadata
                .get("ibm.plan_id")
                .map(String::as_str),
            Some("plan-1")
        );
        assert_eq!(
            record
                .provider_metadata
                .get("ibm.usage_limit_reached")
                .map(String::as_str),
            Some("true")
        );
        assert_eq!(
            record
                .provider_metadata
                .get("ibm.time_available_at")
                .map(String::as_str),
            Some("2026-09-14T12:00:00Z")
        );

        let consumed = record
            .usage
            .iter()
            .find(|metric| metric.name == "ibm.instance.usage_consumed")
            .expect("consumed usage metric should exist");
        assert_eq!(consumed.value, 12.0);
        assert_eq!(consumed.unit, "seconds");
        assert!(!consumed.semantics.is_empty());
    }

    #[test]
    fn missing_optional_ibm_instance_usage_stays_absent() {
        let provider_usage = compute_models::GetUsage200Response::new(
            "instance-123".to_string(),
            "plan-1".to_string(),
            0.0,
        );

        let record = account_usage_from_instance_usage(&provider_usage);

        assert!(record.period_start.is_none());
        assert!(record.period_end.is_none());
        assert_eq!(record.usage.len(), 1);
        assert!(record
            .usage
            .iter()
            .all(|metric| metric.name != "ibm.instance.usage_limit"));
        assert!(record
            .usage
            .iter()
            .all(|metric| metric.name != "ibm.instance.usage_allocation"));
        assert!(!record
            .provider_metadata
            .contains_key("ibm.usage_limit_reached"));
        assert!(!record
            .provider_metadata
            .contains_key("ibm.time_available_at"));
    }
}

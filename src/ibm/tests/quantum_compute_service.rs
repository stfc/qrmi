// This code is part of Qiskit.
//
// (C) Copyright IBM, Pasqal 2026
// (C) Copyright UKRI-STFC (Hartree Centre) 2026
//
// This code is licensed under the Apache License, Version 2.0. You may
// obtain a copy of this license in the LICENSE.txt file in the root directory
// of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
//
// Any modifications or derivative works of this code must retain this
// copyright notice, and modified files need to carry a notice indicating
// that they have been altered from the originals.

use super::super::IBMQuantumComputeService;
use super::task_usage_from_job_metrics;
use crate::models::{AccountingStatus, ResourceType};
use crate::QuantumResource;
use quantum_compute_client::apis::configuration;
use quantum_compute_client::models;

#[tokio::test]
async fn resource_id_and_type_match_backend() {
    const BACKEND_NAME: &str = "ibm_torino";

    // Create a service instance with dummy configuration for testing
    // Note: The configuration values are not used in this test since we are only
    // testing the resource_id and resource_type methods.
    // hence their values don't matter
    let mut qrmi = IBMQuantumComputeService {
        config: configuration::Configuration::new(),
        backend_name: BACKEND_NAME.to_string(),
        session_id: None,
        calibration_id: None,
        timeout_secs: None,
        session_mode: "dedicated".to_string(),
        session_max_ttl: 28800,
        api_key: "dummy".to_string(),
        iam_endpoint: "http://127.0.0.1:8080".to_string(),
        token_expiration: 0,
        token_lifetime: 0,
    };

    let resource_id = qrmi
        .resource_id()
        .await
        .expect("resource_id should succeed");
    let resource_type = qrmi
        .resource_type()
        .await
        .expect("resource_type should succeed");

    assert_eq!(resource_id, BACKEND_NAME);
    assert_eq!(resource_type, ResourceType::IBMQuantumComputeService);
}
#[test]
fn current_job_metrics_map_to_task_usage_without_changing_units() {
    let job_metrics: models::JobMetrics = serde_json::from_str(
        r#"{
            "timestamps": {
                "created": "2026-09-12T10:00:00Z",
                "running": "2026-09-12T10:00:05Z",
                "finished": "2026-09-12T10:00:15Z"
            },
            "usage": {
                "qpu_charge_time_seconds": 20,
                "status": "complete"
            },
            "circuits_execution_time_ns": 20000000,
            "qiskit_version": "2.3.0"
        }"#,
    )
    .expect("current job metrics response should deserialize");

    let usage = task_usage_from_job_metrics("job-123", &job_metrics);

    assert_eq!(usage.task_id, "job-123");
    assert_eq!(usage.created.as_deref(), Some("2026-09-12T10:00:00Z"));
    assert_eq!(usage.running.as_deref(), Some("2026-09-12T10:00:05Z"));
    assert_eq!(usage.finished.as_deref(), Some("2026-09-12T10:00:15Z"));
    assert_eq!(usage.accounting_status, Some(AccountingStatus::Final));
    assert_eq!(usage.metrics.len(), 2);

    assert_eq!(usage.metrics[0].name, "ibm.job.qpu_charge_time");
    assert_eq!(usage.metrics[0].value, 20.0);
    assert_eq!(usage.metrics[0].unit, "seconds");
    assert!(!usage.metrics[0].semantics.is_empty());

    assert_eq!(usage.metrics[1].name, "ibm.job.circuits_execution_time");
    assert_eq!(usage.metrics[1].value, 20_000_000.0);
    assert_eq!(usage.metrics[1].unit, "nanoseconds");
    assert!(!usage.metrics[1].semantics.is_empty());
}

#[test]
fn explicit_zero_job_metric_is_preserved() {
    let job_metrics: models::JobMetrics = serde_json::from_str(
        r#"{
            "usage": {
                "qpu_charge_time_seconds": 2,
                "status": "complete"
            },
            "circuits_execution_time_ns": 0
        }"#,
    )
    .expect("zero-valued current job metrics response should deserialize");

    let usage = task_usage_from_job_metrics("job-zero", &job_metrics);

    assert_eq!(usage.accounting_status, Some(AccountingStatus::Final));
    assert_eq!(usage.metrics.len(), 2);
    assert_eq!(usage.metrics[0].name, "ibm.job.qpu_charge_time");
    assert_eq!(usage.metrics[0].value, 2.0);
    assert_eq!(usage.metrics[1].name, "ibm.job.circuits_execution_time");
    assert_eq!(usage.metrics[1].value, 0.0);
}

#[test]
fn pending_job_metrics_are_pending() {
    let job_metrics: models::JobMetrics = serde_json::from_str(
        r#"{
            "usage": {
                "qpu_charge_time_seconds": 3.5,
                "status": "pending"
            }
        }"#,
    )
    .expect("pending job metrics response should deserialize");

    let usage = task_usage_from_job_metrics("job-123", &job_metrics);

    assert_eq!(usage.accounting_status, Some(AccountingStatus::Pending));
}

#[test]
fn missing_job_metrics_stay_missing() {
    let job_metrics: models::JobMetrics = serde_json::from_str(
        r#"{
            "timestamps": {
                "created": "2026-09-12T10:00:00Z"
            }
        }"#,
    )
    .expect("partial job metrics response should deserialize");

    let usage = task_usage_from_job_metrics("job-123", &job_metrics);

    assert_eq!(usage.created.as_deref(), Some("2026-09-12T10:00:00Z"));
    assert!(usage.running.is_none());
    assert!(usage.finished.is_none());
    assert!(usage.accounting_status.is_none());
    assert!(usage.metrics.is_empty());
}

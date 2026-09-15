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
use serde::{Deserialize, Serialize};
#[cfg(feature = "pyo3")]
use {
    pyo3::prelude::*,
    pyo3_stub_gen::{define_stub_info_gatherer, derive::*},
};

use super::UsageMetric;

/// Completeness of provider-reported task accounting data.
///
/// Absence is represented by `None` on [`TaskUsage::accounting_status`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "pyo3",
    pyclass(eq, eq_int, hash, frozen, skip_from_py_object),
    gen_stub_pyclass_enum
)]
pub enum AccountingStatus {
    /// Provider says accounting data is still being finalized.
    Pending,
    /// Provider says accounting data is final.
    Final,
}

/// Provider-reported usage information for one task.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "pyo3",
    pyclass(eq, get_all, skip_from_py_object),
    gen_stub_pyclass
)]
pub struct TaskUsage {
    /// Provider task identifier.
    pub task_id: String,
    /// Timestamp at which the task was created, if reported.
    pub created: Option<String>,
    /// Timestamp at which the task started running, if reported.
    pub running: Option<String>,
    /// Timestamp at which the task finished, if reported.
    pub finished: Option<String>,
    /// Provider-reported accounting completeness, if reported.
    pub accounting_status: Option<AccountingStatus>,
    /// Authoritative provider-reported usage quantities.
    pub metrics: Vec<UsageMetric>,
}

#[cfg(feature = "pyo3")]
define_stub_info_gatherer!(stub_info);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_accounting_status_stays_absent() {
        let record = TaskUsage {
            task_id: "task-1".to_string(),
            created: None,
            running: None,
            finished: None,
            accounting_status: None,
            metrics: Vec::new(),
        };

        let value = serde_json::to_value(&record).expect("record should serialize");
        assert!(value["accounting_status"].is_null());
        assert_eq!(value["metrics"].as_array().map(Vec::len), Some(0));
    }
}

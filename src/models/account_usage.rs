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
use std::collections::BTreeMap;
#[cfg(feature = "pyo3")]
use {
    pyo3::prelude::*,
    pyo3_stub_gen::{define_stub_info_gatherer, derive::*},
};

use super::UsageMetric;

/// Schema version for serialized [`AccountUsage`] records.
pub const ACCOUNT_USAGE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "pyo3",
    pyclass(eq, get_all, skip_from_py_object),
    gen_stub_pyclass
)]
pub struct AccountUsage {
    /// Logical schema version for serialized records.
    pub schema_version: u16,
    /// QRMI provider/resource type that produced this record.
    pub provider_type: String,
    /// Opaque provider-side accounting-scope identifier.
    pub account_id: String,
    /// Provider-reported usage-period start, if available.
    pub period_start: Option<String>,
    /// Provider-reported usage-period end, if available.
    pub period_end: Option<String>,
    /// Authoritative provider-reported numeric accounting quantities.
    pub usage: Vec<UsageMetric>,
    /// Namespaced provider facts that do not belong in the common model.
    pub provider_metadata: BTreeMap<String, String>,
}

#[cfg(feature = "pyo3")]
define_stub_info_gatherer!(stub_info);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_usage_serializes_schema_and_namespaced_metadata() {
        let mut provider_metadata = BTreeMap::new();
        provider_metadata.insert("provider.plan_id".to_string(), "plan-1".to_string());

        let record = AccountUsage {
            schema_version: ACCOUNT_USAGE_SCHEMA_VERSION,
            provider_type: "provider-type".to_string(),
            account_id: "scope-1".to_string(),
            period_start: Some("2026-09-01T00:00:00Z".to_string()),
            period_end: Some("2026-09-02T00:00:00Z".to_string()),
            usage: vec![UsageMetric {
                name: "provider.usage".to_string(),
                value: 42.0,
                unit: "seconds".to_string(),
                semantics: "Provider-defined consumed usage.".to_string(),
            }],
            provider_metadata,
        };

        let value = serde_json::to_value(&record).expect("record should serialize");
        assert_eq!(value["schema_version"], ACCOUNT_USAGE_SCHEMA_VERSION);
        assert_eq!(value["account_id"], "scope-1");
        assert_eq!(value["usage"][0]["unit"], "seconds");
        assert_eq!(value["provider_metadata"]["provider.plan_id"], "plan-1");
    }
}

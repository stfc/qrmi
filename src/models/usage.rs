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

/// One authoritative provider-reported accounting/usage quantity.
///
/// `name` may be provider-namespaced when QRMI cannot establish a stable
/// cross-provider equivalence. `semantics` records what the provider says the
/// quantity means so consumers do not infer equivalence from a name or unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "pyo3",
    pyclass(eq, get_all, skip_from_py_object),
    gen_stub_pyclass
)]
pub struct UsageMetric {
    /// Stable or provider-namespaced metric name.
    pub name: String,
    /// Numeric value exactly as represented by the provider client.
    pub value: f64,
    /// Provider unit, or `count` for count metrics.
    pub unit: String,
    /// Provider-defined meaning of this quantity.
    pub semantics: String,
}

#[cfg(feature = "pyo3")]
define_stub_info_gatherer!(stub_info);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_metric_serializes_semantics() {
        let metric = UsageMetric {
            name: "provider.metric".to_string(),
            value: 42.0,
            unit: "nanoseconds".to_string(),
            semantics: "Provider-defined execution quantity.".to_string(),
        };

        let value = serde_json::to_value(&metric).expect("metric should serialize");
        assert_eq!(value["name"], "provider.metric");
        assert_eq!(value["unit"], "nanoseconds");
        assert_eq!(value["semantics"], "Provider-defined execution quantity.");
    }
}

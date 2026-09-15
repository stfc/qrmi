# -*- coding: utf-8 -*-
# This code is part of Qiskit.
#
# (C) Copyright IBM 2026
# (C) Copyright UKRI-STFC (Hartree Centre) 2026
#
# This code is licensed under the Apache License, Version 2.0. You may
# obtain a copy of this license in the LICENSE.txt file in the root directory
# of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
#
# Any modifications or derivative works of this code must retain this
# copyright notice, and modified files need to carry a notice indicating
# that they have been altered from the originals.

import qrmi


def test_accounting_usage_api_is_exported():
    assert hasattr(qrmi, "UsageMetric")
    assert hasattr(qrmi, "AccountingStatus")
    assert hasattr(qrmi, "TaskUsage")
    assert hasattr(qrmi, "AccountUsage")
    assert hasattr(qrmi.QuantumResource, "task_usage")
    assert hasattr(qrmi.ResourceProvider, "account_usage")

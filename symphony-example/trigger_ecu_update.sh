#!/bin/bash

# SPDX-FileCopyrightText: 2025 Contributors to the Eclipse Foundation
#
# See the NOTICE file(s) distributed with this work for additional
# information regarding copyright ownership.
# 
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# SPDX-License-Identifier: Apache-2.0

symphony_api_url="http://localhost:8082/v1alpha2/"

token=$(curl -s -X POST -H "Content-Type: application/json" -d '{"username":"admin","password":""}' "${symphony_api_url}users/auth" | jq -r '.accessToken')
authorization_header="Authorization: Bearer $token"

curl -sS -X POST -H "$authorization_header" -H "Content-Type: application/json" --data @./target.json "${symphony_api_url}targets/registry/ecu-updater-target"

# Prompt user to press Enter to continue after the target has been registered
read -r -p "Target registered. Press Enter to remove..."

curl -sS -X DELETE -H "$authorization_header" "${symphony_api_url}targets/registry/ecu-updater-target"

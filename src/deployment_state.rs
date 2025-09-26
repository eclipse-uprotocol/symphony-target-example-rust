/********************************************************************************
 * Copyright (c) 2025 Contributors to the Eclipse Foundation
 *
 * See the NOTICE file(s) distributed with this work for additional
 * information regarding copyright ownership.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Apache License Version 2.0 which is available at
 * https://www.apache.org/licenses/LICENSE-2.0
 *
 * SPDX-License-Identifier: Apache-2.0
 ********************************************************************************/
use std::{collections::HashMap, sync::RwLock};

use log::info;
use symphony::models::{ComponentResultSpec, ComponentSpec, DeploymentSpec, State};

#[derive(Default)]
pub(crate) struct DeploymentState {
    // component name -> component
    components: RwLock<HashMap<String, ComponentSpec>>,
}

impl DeploymentState {
    pub(crate) fn get_status(
        &self,
        references: Vec<ComponentSpec>,
        _deployment_spec: DeploymentSpec,
    ) -> Vec<ComponentSpec> {
        let mut result = vec![];
        if let Ok(components_read) = self.components.read() {
            references.iter().for_each(|spec| {
                if let Some(v) = components_read.get(&spec.name) {
                    result.push(v.clone());
                }
            });
        }
        result
    }

    pub(crate) fn update_components(
        &self,
        components_to_update: Vec<ComponentSpec>,
        _deployment_spec: DeploymentSpec,
    ) -> HashMap<String, ComponentResultSpec> {
        let mut result = HashMap::new();
        components_to_update.iter().for_each(|spec| {
            if let Ok(mut components_write) = self.components.write() {
                if let Some(fw_image_url) = spec.properties.as_ref().and_then(|props| props.get("fw-image")) {
                    info!("installing firmware [name: {}, FW Image: {}]", spec.name, fw_image_url);
                    components_write.insert(spec.name.clone(), spec.clone());
                    result.insert(
                        spec.name.clone(),
                        ComponentResultSpec {
                            status: State::OK,
                            message: "component updated successfully".to_string(),
                        },
                    );
                } else {
                    // this should better be handled by configuring the Target Provider
                    // with a corresponding ComponentValidationRule
                    result.insert(
                        spec.name.clone(),
                        ComponentResultSpec {
                            status: State::InvalidArgument,
                            message: "Firmware ComponentSpec must contain fw-image property".to_string(),
                        },
                    );
                }
            } else {
                result.insert(
                    spec.name.clone(),
                    ComponentResultSpec {
                        status: State::InternalError,
                        message: "failed to acquire lock for updating component".to_string(),
                    },
                );
            }
        });
        result
    }

    pub(crate) fn delete_components(
        &self,
        components_to_delete: Vec<ComponentSpec>,
        _deployment_spec: DeploymentSpec,
    ) -> HashMap<String, ComponentResultSpec> {
        let mut result = HashMap::new();
            components_to_delete.iter().for_each(|spec| {
            if let Ok(mut components_write) = self.components.write() {
                info!("removing firmware [{}]", spec.name);
                components_write.remove(&spec.name);
                result.insert(
                    spec.name.clone(),
                    ComponentResultSpec {
                        status: State::Deleted,
                        message: "component deleted successfully".to_string(),
                    },
                );
            } else {
                result.insert(
                    spec.name.clone(),
                    ComponentResultSpec {
                        status: State::InternalError,
                        message: "failed to acquire lock for deleting component".to_string(),
                    },
                );
            }
        });
        result
    }
}

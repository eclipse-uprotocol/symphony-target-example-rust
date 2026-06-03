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

use async_trait::async_trait;
use tracing::info;
use symphony::models::{ComponentResultSpec, ComponentSpec, DeploymentSpec, State};

use up_rust::symphony::DeploymentTarget;

#[derive(Default)]
pub(crate) struct EcuTarget {
    // component name -> component
    components: RwLock<HashMap<String, ComponentSpec>>,
}

#[async_trait]
impl DeploymentTarget for EcuTarget {
    async fn get(
        &self,
        references: Vec<ComponentSpec>,
        _deployment_spec: DeploymentSpec,
    ) -> Result<Vec<ComponentSpec>, Box<dyn core::error::Error>> {
        let mut result = vec![];
        if let Ok(components_read) = self.components.read() {
            references.iter().for_each(|spec| {
                if let Some(v) = components_read.get(&spec.name) {
                    result.push(v.clone());
                }
            });
        } else {
            return Err("failed to acquire lock for reading components".into());
        }
        Ok(result)
    }

    async fn update(
        &self,
        components_to_update: Vec<ComponentSpec>,
        _deployment_spec: DeploymentSpec,
    ) -> Result<HashMap<String, ComponentResultSpec>, Box<dyn core::error::Error>> {
        let mut result = HashMap::new();
        components_to_update.iter().for_each(|spec| {
            if let Ok(mut components_write) = self.components.write() {
                if let Some(fw_image_url) = spec
                    .properties
                    .as_ref()
                    .and_then(|props| props.get("fw-image"))
                {
                    info!(
                        "installing firmware [name: {}, FW Image: {}]",
                        spec.name, fw_image_url
                    );
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
                            message: "Firmware ComponentSpec must contain fw-image property"
                                .to_string(),
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
        Ok(result)
    }

    async fn delete(
        &self,
        components_to_delete: Vec<ComponentSpec>,
        _deployment_spec: DeploymentSpec,
    ) -> Result<HashMap<String, ComponentResultSpec>, Box<dyn core::error::Error>> {
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
        Ok(result)
    }
}

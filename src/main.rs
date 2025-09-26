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

/*!
This example illustrates how uProtocol's _Communication Level API_ can be used to implement
an Eclipse Symphony deployment Target which can be invoked by means of Symphony's uProtocol
Target Provider.

The example implements a simple in-memory deployment state and supports the following operations:
- Get the status of a set of components
- Apply updates to a set of components
- Delete a set of components

The operations are exposed as uProtocol service endpoints using an in-memory RPC server.
The example supports two different transports: Zenoh and MQTT 5. The transport can be
selected via command line arguments.
 */
use std::{sync::Arc, time::Duration};

use backon::{ExponentialBuilder, Retryable};
use clap::{Parser, command};
use clap_num::maybe_hex;
use log::{debug, info, log_enabled, warn};
use serde_json::Value;
use symphony::models::{ComponentSpec, DeploymentSpec};
use up_rust::{
    LocalUriProvider, StaticUriProvider, UAttributes, UCode, UPayloadFormat,
    communication::{
        InMemoryRpcServer, RequestHandler, RpcServer, ServiceInvocationError, UPayload,
    },
};
use up_transport_mqtt5::{Mqtt5TransportOptions, MqttClientOptions};
use up_transport_zenoh::UPTransportZenoh;

mod deployment_state;

pub(crate) const METHOD_GET_RESOURCE_ID: u16 = 0x0001;
pub(crate) const METHOD_UPDATE_RESOURCE_ID: u16 = 0x0002;
pub(crate) const METHOD_DELETE_RESOURCE_ID: u16 = 0x0003;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[arg(long, value_name = "NAME", env = "UP_AUTHORITY", default_value = "ecu-updater.app")]
    authority: String,
    #[arg(long, value_name = "ID", env = "UP_ENTITY_ID", default_value = "0x0000A100", value_parser = maybe_hex::<u32>)]
    uentity_id: u32,
    #[arg(long, value_name = "VERSION", env = "UP_ENTITY_VERSION", default_value = "0x01", value_parser = maybe_hex::<u8>)]
    uentity_version: u8,
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Use Zenoh as transport
    Zenoh,
    /// Use MQTT 5 as transport
    Mqtt5 {
        #[command(flatten)]
        options: MqttClientOptions,
    },
}

async fn get_transport(
    cli: Cli,
) -> Result<Arc<dyn up_rust::UTransport>, Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Zenoh => {
            info!("Using default Zenoh transport");
            let transport = UPTransportZenoh::builder(cli.authority)?
                .build()
                .await
                .map(Arc::new)?;
            Ok(transport)
        }
        Commands::Mqtt5 { options } => {
            info!(
                "Using MQTT 5 transport with broker URI: {}",
                options.broker_uri
            );
            let transport_options = Mqtt5TransportOptions {
                mqtt_client_options: options,
                mode: up_transport_mqtt5::TransportMode::InVehicle,
                ..Default::default()
            };
            let transport =
                up_transport_mqtt5::Mqtt5Transport::new(transport_options, cli.authority)
                    .await
                    .map(Arc::new)?;
            (|| transport.connect())
                .retry(
                    ExponentialBuilder::default().with_total_delay(Some(Duration::from_secs(10))),
                )
                .notify(|error, sleep_duration| {
                    info!("Attempt to connect to MQTT broker failed [error: {error}], retrying in {sleep_duration:?}");
                })
                .when(|err| {
                    // no need to keep retrying if authentication or permission is denied
                    err.get_code() != UCode::UNAUTHENTICATED
                        && err.get_code() != UCode::PERMISSION_DENIED
                })
                .await?;
            info!("Connected to MQTT5 broker");
            Ok(transport)
        }
    }
}

fn extract_request_data(
    request_payload: Option<UPayload>,
) -> Result<Value, ServiceInvocationError> {
    let Some(req_payload) = request_payload
        .filter(|req_payload| req_payload.payload_format() == UPayloadFormat::UPAYLOAD_FORMAT_JSON)
    else {
        return Err(ServiceInvocationError::InvalidArgument(
            "request has no JSON payload".to_string(),
        ));
    };

    serde_json::from_slice(req_payload.payload().to_vec().as_slice()).map_err(|err| {
        warn!("failed to deserialize request payload: {:?}", err);
        ServiceInvocationError::InvalidArgument(
            "request payload is not a valid UTF-8 string".to_string(),
        )
    })
}

pub(crate) struct GetOperation {
    state: Arc<deployment_state::DeploymentState>,
}

#[async_trait::async_trait]
impl RequestHandler for GetOperation {
    // expects a DeploymentSpec in the request and returns an array of ComponentSpecs
    async fn handle_request(
        &self,
        _resource_id: u16,
        message_attributes: &UAttributes,
        request_payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        let request_data = extract_request_data(request_payload)?;
        info!(
            "processing GET request [from: {}]",
            message_attributes
                .source
                .as_ref()
                .unwrap_or_default()
                .to_uri(true),
        );
        if log_enabled!(log::Level::Debug) {
            debug!(
                "payload: {}",
                serde_json::to_string_pretty(&request_data).expect("failed to serialize Value")
            );
        }
        let deployment_spec: DeploymentSpec =
            serde_json::from_value(request_data["deployment"].clone()).map_err(|err| {
                info!("request does not contain DeploymentSpec: {err}");
                ServiceInvocationError::InvalidArgument(
                    "request does not contain DeploymentSpec".to_string(),
                )
            })?;
        let component_specs: Vec<ComponentSpec> =
            serde_json::from_value(request_data["components"].clone()).map_err(|err| {
                info!("request does not contain ComponentSpec array: {err}");
                ServiceInvocationError::InvalidArgument(
                    "request does not contain ComponentSpec array".to_string(),
                )
            })?;

        let result = self.state.get_status(component_specs, deployment_spec);
        let serialized_response_data = serde_json::to_vec(&result).map_err(|err| {
            warn!("error serializing ComponentSpec: {err}");
            ServiceInvocationError::Internal("failed to create response payload".to_string())
        })?;
        if log_enabled!(log::Level::Debug) {
            eprintln!(
                "returning response: {}",
                serde_json::to_string_pretty(&result).expect("failed to serialize Value")
            );
        }
        let response_payload = UPayload::new(
            serialized_response_data,
            UPayloadFormat::UPAYLOAD_FORMAT_JSON,
        );
        Ok(Some(response_payload))
    }
}

pub(crate) struct ApplyOperation {
    state: Arc<deployment_state::DeploymentState>,
}

#[async_trait::async_trait]
impl RequestHandler for ApplyOperation {
    async fn handle_request(
        &self,
        resource_id: u16,
        message_attributes: &UAttributes,
        request_payload: Option<UPayload>,
    ) -> Result<Option<UPayload>, ServiceInvocationError> {
        let request_data = extract_request_data(request_payload)?;
        info!(
            "processing request [method: {}, from: {}]",
            message_attributes
                .sink
                .as_ref()
                .unwrap_or_default()
                .to_uri(true),
            message_attributes
                .source
                .as_ref()
                .unwrap_or_default()
                .to_uri(true),
        );
        if log_enabled!(log::Level::Debug) {
            let json =
                serde_json::to_string_pretty(&request_data).expect("failed to serialize Value");
            debug!("payload: {}", json);
        }

        let deployment_spec: DeploymentSpec =
            serde_json::from_value(request_data["deployment"].clone()).map_err(|err| {
                info!("request does not contain DeploymentSpec: {err}");
                ServiceInvocationError::InvalidArgument(
                    "request does not contain DeploymentSpec".to_string(),
                )
            })?;

        let affected_components: Vec<ComponentSpec> =
            serde_json::from_value(request_data["components"].clone()).map_err(|err| {
                info!("request does not contain ComponentSpec array: {err}");
                ServiceInvocationError::InvalidArgument(
                    "request does not contain ComponentSpec array".to_string(),
                )
            })?;

        let result = match resource_id {
            METHOD_UPDATE_RESOURCE_ID => self
                .state
                .update_components(affected_components, deployment_spec),
            METHOD_DELETE_RESOURCE_ID => self
                .state
                .delete_components(affected_components, deployment_spec),
            _ => {
                return Err(ServiceInvocationError::Unimplemented(
                    "no such operation".to_string(),
                ));
            }
        };

        let serialized_response_data = serde_json::to_vec(&result).map_err(|err| {
            warn!("error serializing HashMap: {err}");
            ServiceInvocationError::Internal("failed to create response payload".to_string())
        })?;

        let response_payload = UPayload::new(
            serialized_response_data,
            UPayloadFormat::UPAYLOAD_FORMAT_JSON,
        );
        Ok(Some(response_payload))
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();
    let uri_provider = Arc::new(StaticUriProvider::new(
        cli.authority.clone(),
        cli.uentity_id,
        cli.uentity_version,
    ));
    let transport = get_transport(cli).await?;

    let deployment_state = Arc::new(deployment_state::DeploymentState::default());
    // create the RpcServer using the local transport
    let rpc_server = InMemoryRpcServer::new(transport.clone(), uri_provider.clone());
    // and register endpoints for the service operations
    let get_op = Arc::new(GetOperation {
        state: deployment_state.clone(),
    });
    let apply_op = Arc::new(ApplyOperation {
        state: deployment_state.clone(),
    });

    rpc_server
        .register_endpoint(None, METHOD_GET_RESOURCE_ID, get_op)
        .await?;
    rpc_server
        .register_endpoint(None, METHOD_UPDATE_RESOURCE_ID, apply_op.clone())
        .await?;
    rpc_server
        .register_endpoint(None, METHOD_DELETE_RESOURCE_ID, apply_op)
        .await?;
    info!(
        "ECU Updater service is up and running [local URI: {}]",
        uri_provider.get_source_uri().to_uri(true)
    );
    info!(
        "GET    method URI: {}",
        uri_provider.get_resource_uri(METHOD_GET_RESOURCE_ID).to_uri(true)
    );
    info!(
        "UPDATE method URI: {}",
        uri_provider.get_resource_uri(METHOD_UPDATE_RESOURCE_ID).to_uri(true)
    );
    info!(
        "DELETE method URI: {}",
        uri_provider.get_resource_uri(METHOD_DELETE_RESOURCE_ID).to_uri(true)
    );
    tokio::signal::ctrl_c().await?;
    info!("Received SIGTERM, shutting down ...");
    Ok(())
}

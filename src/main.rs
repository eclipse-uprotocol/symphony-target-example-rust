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
use std::{str::FromStr, sync::Arc, time::Duration};

use backon::{ExponentialBuilder, Retryable};
use clap::Parser;
use clap_num::maybe_hex;
use tracing::info;
use up_rust::{
    LocalUriProvider, StaticUriProvider, UCode, UUri,
    communication::InMemoryRpcServer,
};
use up_transport_mqtt5::{Mqtt5TransportOptions, MqttClientOptions};
use up_transport_zenoh::{UPTransportZenoh, zenoh_config::Config};

mod ecu_target;

pub(crate) const METHOD_GET_RESOURCE_ID: u16 = 0x0001;
pub(crate) const METHOD_UPDATE_RESOURCE_ID: u16 = 0x0002;
pub(crate) const METHOD_DELETE_RESOURCE_ID: u16 = 0x0003;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[arg(
        long,
        value_name = "URI",
        env = "UP_LOCAL_ADDRESS",
        value_parser = UUri::from_str,
        conflicts_with_all = ["authority", "uentity_id", "uentity_version"],
    )]
    local_address: Option<UUri>,
    #[arg(
        long,
        value_name = "NAME",
        env = "UP_AUTHORITY",
        default_value = "ecu-updater.app"
    )]
    authority: String,
    #[arg(
        long,
        value_name = "ID",
        env = "UP_ENTITY_ID",
        default_value = "0x0000A100",
        value_parser = maybe_hex::<u32>
    )]
    uentity_id: u32,
    #[arg(
        long,
        value_name = "VERSION",
        env = "UP_ENTITY_VERSION",
        default_value = "0x01",
        value_parser = maybe_hex::<u8>
    )]
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
                .with_config(Config::default())
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

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    let uri_provider = match &cli.local_address {
        Some(local_address) => Arc::new(StaticUriProvider::try_from(local_address)?),
        None => Arc::new(StaticUriProvider::new(
            cli.authority.clone(),
            cli.uentity_id,
            cli.uentity_version,
        )),
    };
    let transport = get_transport(cli).await?;

    let deployment_target = Arc::new(ecu_target::EcuTarget::default());
    // create the RpcServer using the local transport
    let rpc_server = InMemoryRpcServer::new(transport.clone(), uri_provider.clone());
    // and register endpoints for the service operations
    up_rust::symphony::register_target_provider_endpoints(
        &rpc_server,
        deployment_target.clone(),
    ).await?;

    info!(
        "ECU Updater service is up and running [local URI: {}]",
        uri_provider.get_source_uri().to_uri(true)
    );
    info!(
        "GET    method URI: {}",
        uri_provider
            .get_resource_uri(METHOD_GET_RESOURCE_ID)
            .to_uri(true)
    );
    info!(
        "UPDATE method URI: {}",
        uri_provider
            .get_resource_uri(METHOD_UPDATE_RESOURCE_ID)
            .to_uri(true)
    );
    info!(
        "DELETE method URI: {}",
        uri_provider
            .get_resource_uri(METHOD_DELETE_RESOURCE_ID)
            .to_uri(true)
    );
    tokio::signal::ctrl_c().await?;
    info!("Received SIGTERM, shutting down ...");
    Ok(())
}

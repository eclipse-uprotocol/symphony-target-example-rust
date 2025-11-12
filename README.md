# Eclipse Symphony Target Example

This repository contains an example implementation of a remote [Eclipse Symphony&trade;](https://github.com/eclipse-symphony/symphony) _Target_ which can be used by means of Symphony's uProtocol Target Provider.

The example application is implemented as a uProtocol service and simulates functionality for updating firmware of vehicle ECUs. The exposed API operates on some simple in-memory deployment state and allows the Symphony control plane to (remotely) query the currently installed firmware versions and to update firmware on ECUs.

The Symphony uProtocol Target Provider expects target implementations to implement the [Target Provider uService contract](./uservice/asyncapi.yaml).
The example application uses [uProtocol's _Communication Level API_](https://github.com/eclipse-uprotocol/up-spec/blob/main/up-l2/api.adoc) to expose the three operations defined in the contract.

## Getting Started

The [symphony-example](./symphony-example/) folder contains a [Docker Compose](https://docs.docker.com/compose/) file which illustrates the usage of the ECU Updater together with a Symphony API server using [Eclipse Zenoh&trade;](https://zenoh.io) as the uProtocol transport layer.

## Building from Source Code

The example application is implemented in Rust and therefore requires a [Rust toolchain to be installed](https://rustup.rs/) for building.

```bash
cargo build
```

## Running from the Command Line

The application supports using either Eclipse Zenoh or MQTT 5 for exchanging messages. The transport can be selected on the command line.

```bash
./target/debug/ecu-updater
```

will display all available command line options.

In order to use the MQTT 5 based transport and connect to a local MQTT 5 broker (`mqtt://localhost:1883`) which doesn't require authentication:

```bash
./target/debug/ecu-updater mqtt5
```

In order to enable informational log statements being printed to the console, the `RUST_LOG` environment variable can be used:

```bash
RUST_LOG=INFO ./target/debug/ecu-updater mqtt5
```

To enable debug logging for the app, use:

```bash
RUST_LOG=INFO,ecu_updater=DEBUG ./target/debug/ecu-updater mqtt5
```

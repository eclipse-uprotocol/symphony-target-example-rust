# Eclipse Symphony Target Example

This repository contains an example implementation of a remote [Eclipse Symphony&trade;](https://github.com/eclipse-symphony/symphony) _target_ which can be used by means of Symphony's uProtocol Target Provider.

The example application simulates functionality for updating firmware of vehicle ECUs. The exposed API operates on some simple in-memory deployment state and allows the Symphony control plane to (remotely) query the currently installed firmware versions and to update firmware on ECUs.

The Symphony uProtocol Target Provider expects target implementations to implement the [Target Provider uService contract](./uservice/asyncapi.yaml).
The example application uses [uProtocol's _Communication Level API_](https://github.com/eclipse-uprotocol/up-spec/blob/main/up-l2/api.adoc) to expose the three operations defined in the contract.

## Getting Started

The example application is implemented in Rust and therefore requires a [Rust toolchain to be installed](https://rustup.rs/) for building.

```bash
cargo build
```

The application supports using either [Eclipse Zenoh&trade;](https://zenoh.io) or MQTT 5 for exchanging messages. The transport can be selected on the command line.

```bash
./target/debug/ecu-updater
```

will display all available command line options.

In order to use the MQTT 5 based transport and connect to a local MQTT 5 broker (`mqtt://localhost:1883`) that doesn't require authentication:

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

## Deploying FW to ECUs via Symphony

1. Start an MQTT 5 broker that accepts unauthenticated connections from clients (listening on `mqtt://localhost:1883`).
2. Start the ECU Updater app:
   ```bash
   RUST_LOG=INFO,ecu_updater=DEBUG ./target/debug/ecu-updater mqtt5
   ```
3. Start the Symphony API (listening on `http://localhost:8282/v1alpha2`)
4. Adapt the content of the [target.json](./target.json) file.
   Make sure to replace the value of the `libFile` property with the absolute path to the dynamic library that contains the uProtocol Target Provider.

   Also make sure to set the `getMethodUri` to the URI that the ECU Updater exposes the _GET_ method on. The application prints this URI to the console during startup. The value in the JSON object above is the default address and does not need to be adapted unless you have explicitly set another address on the command line.
5. Create the target via Symphony's HTTP API:
   ```bash
   curl -X POST \
     -H "Content-Type: application/json" \
     -d @target.json \
     http://localhost:8082/v1alpha2/targets/registry/Vehicle_ECUs
   ```

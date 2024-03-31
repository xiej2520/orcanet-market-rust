# Rust Market Server

## Team Sea Chicken 🐔

An implementation of the OrcaNet market server, built using Rust and
[gRPC with Tonic](https://github.com/hyperium/tonic).

## Requirements

The `setup.sh` script provided should install dependencies and build the project
(tested on Ubuntu 20.04). Otherwise,

1. Install [Rust](https://www.rust-lang.org/tools/install)
2. Install protoc:

   `apt install protobuf-compiler`

   (May require more a [more recent version](https://grpc.io/docs/protoc-installation/#install-pre-compiled-binaries-any-os))

## Running

The default application built by this project will run both the market server
with gRPC port 50051, and a Kademlia node. The `dht_client` binary will only
run the Kademlia node. Parameters need to be provided to get the server to work
with the Kademlia network.

### Parameters

The market server and the `dht_client` binary share the same parameters, which
are used to configure the Kademlia node running on the application.

* `bootstrap-peers`
  * Space separated list of Multiaddr peer nodes to connect to in order to
  bootstrap the node onto a Kademlia network.
  * *If this is not provided, the application will start a new Kademlia network*
* `private-key`
  * Private key in order for the node to be set up as a Kademlia server node.
  * The application will print out the peer id derived from this key.
  * This must be provided in order for the node to **act as a server node**,
  otherwise it will only act as a client node (it can only query the network,
  and not provide data).
* `listen-address`
  * Multiaddr that the application will listen on to act as a Kademlia server node.
  * By default, *if `private-key` is provided*, the node will listen on
  `/ip4/0.0.0.0/tcp/6881`


### Connect to existing network

To connect to an existing Kademlia network, provide the `bootstrap-peers` parameter
with a space separated list of Multiaddrs. `private-key` and `listen-address`
can optionally be provided to have the node also serve data to the network.

```Shell
cargo run -- --bootstrap-peers /ip4/{ip_addr}/tcp/{port}/p2p/{peer id} ...
```

### Start a new Kademlia network

To start a new Kademlia network, first create a public/private key pair

```Shell
openssl genrsa -out private.pem 2048
openssl pkcs8 -in private.pem -inform PEM -topk8 -out private.pk8 -outform DER -nocrypt

rm private.pem      # optional
```

Then run either the market server or `dht_client` with both the `private-key`
and `listen-address` parameters provided.

```Shell
cargo run -- --private-key private.pk8 --listen-address /ip4/0.0.0.0/tcp/6881
cargo run --bin dht_client -- --private-key private.pk8 # by default, /ip4/0.0.0.0/tcp/6881
```

### Run test client

```Shell
cargo run --bin test_client
```

(currently the Go test client is interoperable)

## API
Detailed gRPC endpoints are in `market/market.proto`

- Holders of a file can register the file using the RegisterFile RPC.
  - Provide a User with 5 fields: 
    - `id`: some string to identify the user.
    - `name`: a human-readable string to identify the user
    - `ip`: a string of the public ip address
    - `port`: an int32 of the port
    - `price`: an int64 that details the price per mb of outgoing files
  - Provide a fileHash string that is the hash of the file
  - Returns nothing

- Then, clients can search for holders using the CheckHolders RPC
  - Provide a fileHash to identify the file to search for
  - Returns a list of Users that hold the file.

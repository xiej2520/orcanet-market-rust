# Rust Market Server

## Team Sea Chicken 🐔

An implementation of the OrcaNet market server, built using Rust and
[gRPC with Tonic](https://github.com/hyperium/tonic).

## Setup

1. Install [Rust](https://www.rust-lang.org/tools/install)
2. Install protoc:

   `apt install protobuf-compiler`

   (May require more a [more recent version](https://grpc.io/docs/protoc-installation/#install-pre-compiled-binaries-any-os))

## Running

The market server requires a bootstrap Kademlia node to connect to. Skip this
step if you want to connect to an existing network.

To create a Kademlia network node, first create a public/private key pair

```Shell
openssl genrsa -out private.pem 2048
openssl pkcs8 -in private.pem -inform PEM -topk8 -out private.pk8 -outform DER -nocrypt

rm private.pem      # optional
```

Then start the swarm node

```Shell
cargo run --bin dht_swarm_start -- --private-key private.pk8 --listen-address /ip4/0.0.0.0/tcp/6881
```

Now we can start a market server

```Shell
cargo run -- --bootstrap-peers /ip4/{ip_addr}/tcp/{port}/p2p/{public key}
```

To run a test client

```Shell
cargo run --bin test_client
```

(currently the Go test client is interoperable)

To run more Kademlia nodes for testing

```Shell
cargo run --bin dht_client -- --bootstrap-peers /ip4/{ip_addr}/tcp/{port}/p2p/{public key}
```

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

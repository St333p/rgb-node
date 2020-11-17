RGB-NODE DEMO
===

### introduction
This document contains a textual version of the [rgb-node beta demo]( https://www.youtube.com/watch?v=t_EtUf4601A), updated to operate with a later stage of development (v0.1.0) that allows slightly improved usability. It is meant to demonstrate current node's functionality node and its interface.

Two different setups are available:
- [local installation](#local)
- [docker](#docker)

Once either of them is complete, you can proceed with the actual [demo](#demo)

## Local

#### Requirements
- [cargo](https://doc.rust-lang.org/book/ch01-01-installation.html#installation)
- [git](https://git-scm.com/downloads)

Furthermore, you will need to install a number of system dependencies:
```bash=
sudo apt install -y build-essential pkg-config libzmq3-dev libssl-dev libpq-dev libsqlite3-dev cmake
```
### Build & Run
We can proceed with the compilation of binaries:
```bash=
git clone https://github.com/LNP-BP/rgb-node.git
cd rgb-node
git checkout <somewhere>
cargo install --path . --all-features
```
And then run a couple of nodes into separate terminals
```bash=
rgbd -vvvv -d ./data0
rgbd -vvvv -d ./data1
```
and setup aliases to ease calls to command-line interfaces:
```bash=
cd doc/demo-beta.1
alias rgb0-cli="rgb-cli -d ./data0"
alias rgb1-cli="rgb-cli -d ./data1"
```

## Docker

#### Requirements
- [git](https://git-scm.com/downloads)
- [docker](https://docs.docker.com/get-docker/)
- [docker-compose](https://docs.docker.com/compose/install/)

### Build & Run
Clone the repository
```bash=
git clone https://github.com/LNP-BP/rgb-node.git
cd rgb-node
git checkout <somewhere>
```
and run a couple of nodes in docker
```bash=
# build docker image (takes a while...)
docker build -t rgb-node:demo .
cd doc/demo-beta.1
docker-compose up [-d]
```
To get their respective logs you can run, for instance:
```bash=
docker-compose logs [-f] rgb-node-0
```
Finally we can setup aliases to ease calls to command-line interfaces:
```bash=
alias rgb0-cli="docker exec -it rgb-node-0 rgb-cli"
alias rgb1-cli="docker exec -it rgb-node-1 rgb-cli"
```

## Demo
In this demo, `rgb-node-0` takes the part of the issuer and transfers some of the newly mminted asset to a user, `rgb-node-1`.

In order to get an idea of the functionality exposed by `rgb-cli`, you can run for instance:
```bash=
rgb0-cli help
rgb0-cli fungible help
rgb0-cli fungible list help
rgb0-cli genesis help
```
### premise

RGB-node does not handle wallet-related functionalities, it just performs RGB-specific tasks over data that will be provided by an external wallet such as [bitcoind](https://github.com/bitcoin/bitcoin). In particular, in order to demonstrate a basic workflow with issuance and a transfer, we will need:
- an `issuance_utxo` to which `rgb-node-0` will bind newly issued asset
- a `change_utxo` on which `rgb-node-0` receives asset change
- a `receive_utxo` on which `rgb-node-1` receives assets
- a partially signed bitcoin transaction (`transfer_psbt`), whose output pubkey will be tweaked to include a commitment to the transfer.

For the purposes of this demo, since `rgb-node` has no knowledge of the blockchain, we can use "fake" data generated with a testnet or regtest bitcoin node. The following hardcoded utxos (that will be used later) will also work:

- `issuance_utxo`: `5aa2d0a8098371ee12b4b59f43ffe6a2de637341258af65936a5baa01da49e9b:0`
- `change_utxo`: `0c05cea88d0fca7d16ed6a26d622e7ea477f2e2ff25b9c023b8f06de08e4941a:1`
- `receive_utxo`: `79d0191dab03ffbccc27500a740f20a75cb175e77346244a567011d3c86d2b0b:0`
- an example `transfer_psbt` can be found in the `doc/demo-beta.1/test` folder

### Asset issuance
To issue an asset, run:
```bash=
rgb0-cli fungible issue USDT "USD Tether" 1000@5aa2d0a8098371ee12b4b59f43ffe6a2de637341258af65936a5baa01da49e9b:0
```
This will create a new genesis that includes asset metadata and the allocation of the initial amount to the `<issuance_utxo>`. You can look into it by running:
```bash=
# retrieve <contract-it> with:
rgb0-cli genesis list
# export the genesis contract (use -f to select output format)
rgb0-cli genesis export <contract-id>
```
You can list known fungible assets with:
```bash=
rgb0-cli fungible list
```
which also outputs its `asset-id`, which is needed to create invoices.

### Generate invoice
In order to receive the new USDT, `rgb-node-1` needs to generate an invoice for it:
```bash=
rgb1-cli fungible invoice <asset-id> 100 \
79d0191dab03ffbccc27500a740f20a75cb175e77346244a567011d3c86d2b0b:0
```
This outputs `invoice` and `blinding_factor`.

To be able to accept transfers related to this invoice, we will need the original `receive_utxo` and the `blinding_factor` that was used to include it in the invoice.

### Transfer
To transfer some amounts of asset to `rgb-node-1` to pay the new invoice, `rgb-node-0` needs to create a consignment and commit to it into a bitcoin transaction. So we will need the invoice and a partially signed bitcoin transaction that we will modify to include the commitment. Furthermore, `-i` and `-a` options allow to provide an input utxo from which to take asset and an allocation for the change in the form `<amount>@<utxo>`.

```bash=
# NB: pass the invoice between quotes to avoid misinterpretation of the & character into it
rgb-cli fungible transfer "<invoice>" test/source_tx.psbt test/consignment.rgb test/witness.psbt \
-i 79d0191dab03ffbccc27500a740f20a75cb175e77346244a567011d3c86d2b0b:0 \
-a 900@0c05cea88d0fca7d16ed6a26d622e7ea477f2e2ff25b9c023b8f06de08e4941a:1
```
This will write the consignment file and the psbt including the tweak (which is called *witness transaction*) at the provided paths.

At this point the witness transaction should be signed and broadcasted, while the consignment is sent offchain to the peer.

### Accept
To accept an incoming transfer you need to provide `rgb-node-1` with the consignment file received from `rgb-node-0`, the `receive_utxo` and the corresponding `blinding_factor` that were defined at invoice creation.
```bash=
rgb1-cli fungible accept test/consignment.rgb \
79d0191dab03ffbccc27500a740f20a75cb175e77346244a567011d3c86d2b0b:0 \
<blinding_factor>
```
Now you are able to see the new allocation of 100 asset units at `<receive_utxo>` by running (in the `known_allocations` field):
```bash=
rgb1-cli fungible list -l
```

use rgbstd::interface::{rgb20, ContractBuilder, FungibleAllocation, Rgb20};

use std::convert::Infallible;
use std::fs;

use amplify::hex::FromHex;
use bp::{Chain, Outpoint, Tx, Txid};
use rgb_schemata::{nia_rgb20, nia_schema};
use rgbstd::containers::BindleContent;
use rgbstd::contract::WitnessOrd;
use rgbstd::persistence::{Inventory, Stock};
use rgbstd::resolvers::ResolveHeight;
use rgbstd::stl::{
    Amount, ContractData, DivisibleAssetSpec, Precision, RicardianContract, Timestamp,
};
use rgbstd::validation::{ResolveTx, TxResolverError};
use strict_encoding::StrictDumb;

struct DumbResolver;

impl ResolveTx for DumbResolver {
    fn resolve_tx(&self, _txid: Txid) -> Result<Tx, TxResolverError> {
        Ok(Tx::strict_dumb())
    }
}

impl ResolveHeight for DumbResolver {
    type Error = Infallible;
    fn resolve_height(&mut self, _txid: Txid) -> Result<WitnessOrd, Self::Error> {
        Ok(WitnessOrd::OffChain)
    }
}

#[rustfmt::skip]
fn main() {
    let name = "USDT";
    let decimal = Precision::CentiMicro; // Decimal: 8
    let desc = "USD Tether Token";
    let spec = DivisibleAssetSpec::with("RGB20", name, decimal, Some(desc)).unwrap();
    let terms = RicardianContract::default();
    let contract_data = ContractData { terms, media: None };
    let created = Timestamp::now();
    let txid = "9fef7f1c9cd1dd6b9ac5c0a050103ad874346d4fdb7bcf954de2dfe64dd2ce05";
    let beneficiary = Outpoint::new(Txid::from_hex(txid).unwrap(), 0);

    const ISSUE: u64 = 1_000_000_000;

    let contract = ContractBuilder::with(rgb20(), nia_schema(), nia_rgb20())
        .expect("schema fails to implement RGB20 interface")
        .set_chain(Chain::Testnet3)
        .add_global_state("spec", spec)
        .expect("invalid nominal")
        .add_global_state("created", created)
        .expect("invalid creation date")
        .add_global_state("issuedSupply", Amount::from(ISSUE))
        .expect("invalid issued supply")
        .add_global_state("data", contract_data)
        .expect("invalid contract text")
        .add_fungible_state("assetOwner", beneficiary, ISSUE)
        .expect("invalid asset amount")
        .issue_contract()
        .expect("contract doesn't fit schema requirements");

    let contract_id = contract.contract_id();
    debug_assert_eq!(contract_id, contract.contract_id());

    let bindle = contract.bindle();
    eprintln!("{bindle}");
    bindle.save("contracts/rgb20-usdt.contract.rgb")
        .expect("unable to save contract");
    fs::write("contracts/rgb20-usdt.contract.rgba", bindle.to_string())
        .expect("unable to save contract");

    // Let's create some stock - an in-memory stash and inventory around it:
    let mut stock = Stock::default();
    stock.import_iface(rgb20()).unwrap();
    stock.import_schema(nia_schema()).unwrap();
    stock.import_iface_impl(nia_rgb20()).unwrap();

    // Noe we verify our contract consignment and add it to the stock
    let verified_contract = match bindle.unbindle().validate(&mut DumbResolver) {
        Ok(consignment) => consignment,
        Err(consignment) => {
            panic!(
                "can't produce valid consignment. Report: {}",
                consignment.validation_status()
                    .expect("status always present upon validation")
            );
        }
    };
    stock.import_contract(verified_contract, &mut DumbResolver).unwrap();

    // Reading contract state through the interface from the stock:
    let contract = stock
        .contract_iface(contract_id, rgb20().iface_id())
        .unwrap();
    let contract = Rgb20::from(contract);
    let allocations = contract.fungible("assetOwner", &None).unwrap();
    eprintln!("{}", serde_json::to_string(&contract.spec()).unwrap());

    for FungibleAllocation { owner, witness, value } in allocations {
        eprintln!("amount={value}, owner={owner}, witness={witness}");
    }
    eprintln!("totalSupply={}", contract.total_supply());
    eprintln!("created={}", contract.created().to_local().unwrap());
}

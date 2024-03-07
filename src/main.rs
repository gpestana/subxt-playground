use parity_scale_codec::Decode;
use subxt::{utils::AccountId32, OnlineClient, PolkadotConfig};

use std::fs::OpenOptions;
use std::io::prelude::*;

// CONFIGS
const SKIP: usize = 0;
const CHAIN: &'static str = "kusama";
#[subxt::subxt(runtime_metadata_path = "./artifacts/kusama_metadata.scale")]
pub mod runtime {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = OnlineClient::<PolkadotConfig>::from_url(format!(
        "wss://{}-try-runtime-node.parity-chains.parity.io:443",
        CHAIN
    ))
    .await?;

    let last_block = api.blocks().at_latest().await?;

    let file_path = format!("./{}-{}.data", CHAIN, last_block.number());
    println!(
        ">> Starting to process 0x{:?}; saving to {:?}",
        last_block.hash(),
        file_path
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(file_path)
        .unwrap();

    let _ = writeln!(file, "0x{:?}", last_block.hash());

    let storage_query = runtime::storage().staking().bonded_iter();
    let mut results = api
        .storage()
        .at(last_block.hash())
        .iter(storage_query)
        .await?;

    let mut i = 0;
    let mut count_double = 0;
    let mut count_stash = 0;
    let mut count_controller = 0;
    let mut count_none = 0;
    let mut count_migrated = 0;

    while let Some(Ok((key, value))) = results.next().await {
        if i < SKIP {
            i += 1;
            if i % 100 == 0 {
                print!("{:?}..", i);
            }
            continue;
        }
        let stash = account_from_key(key);
        let controller: AccountId32 = value.into();

        let ledger_controller = api
            .storage()
            .at(last_block.reference().hash())
            .fetch(&runtime::storage().staking().ledger(&controller))
            .await?;
        let ledger_stash = api
            .storage()
            .at(last_block.reference().hash())
            .fetch(&runtime::storage().staking().ledger(&stash))
            .await?;

        println!(
            "> {}   double: {}, stash: {}, controller: {}, migrated: {}, none: {}",
            i, count_double, count_stash, count_controller, count_migrated, count_none
        );

        match (ledger_controller, ledger_stash) {
            (Some(controller_ledger), Some(stash_ledger)) => {
                if stash != controller {
                    println!("----------- double bonded -----------");
                    let _ = writeln!(
                        file,
                        "DOUBLE:\ncontroller: {:?}\nstash: {:?}\ncontroller_ledger: {:?}\nstash_ledger: {:?}",
                        hex::encode(&controller),
                        hex::encode(&stash),
                        controller_ledger,
                        stash_ledger,
                    )
                    .map_err(|e| {
                        println!("error printing to file");
                        e
                    });
                    count_double += 1;
                } else {
                    count_migrated += 1;
                }
            }
            (Some(_), None) => count_controller += 1,
            (None, Some(_)) => count_stash += 1,
            (None, None) => {
                println!("----------- none -----------");
                let _ = writeln!(
                    file,
                    "NONE:\ncontroller: {:?}\nstash: {:?}",
                    hex::encode(&controller),
                    hex::encode(&stash)
                )
                .map_err(|e| {
                    println!("error printing to file");
                    e
                });

                count_none += 1;
            }
        }

        i += 1;
    }

    Ok(())
}

fn account_from_key(q: Vec<u8>) -> AccountId32 {
    let acc = q.into_iter().rev().take(32).rev().collect::<Vec<_>>();
    <AccountId32>::decode(&mut &acc[..]).unwrap()
}

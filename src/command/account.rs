// Copyright 2020-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use clap::{Parser, Subcommand};
use iota_wallet::{
    account::{
        types::{AccountAddress, Transaction},
        AccountHandle,
    },
    iota_client::{
        bee_block::output::{NftId, TokenId},
        request_funds_from_faucet,
    },
    AddressAndNftId, AddressNativeTokens, AddressWithAmount, AddressWithMicroAmount, NativeTokenOptions, NftOptions,
    U256,
};

use crate::error::Error;

#[derive(Debug, Parser)]
#[clap(version, long_about = None)]
#[clap(propagate_version = true)]
pub struct AccountCli {
    #[clap(subcommand)]
    pub command: AccountCommand,
}

#[derive(Debug, Subcommand)]
pub enum AccountCommand {
    /// List the account addresses.
    Addresses,
    /// Print the account balance.
    Balance,
    /// Consolidate all basic outputs into one address.
    Consolidate,
    /// Exit from the account prompt.
    Exit,
    /// Request funds from the faucet to the latest address, `url` is optional, default is `http://localhost:14265/api/plugins/faucet/v1/enqueue`
    Faucet {
        url: Option<String>,
        address: Option<String>,
    },
    /// Mint a native token: `mint-native-token 100 "0x..." (foundry metadata)`
    MintNativeToken {
        maximum_supply: String,
        foundry_metadata: Option<String>,
    },
    /// Mint an nft to an optional bech32 encoded address: `mint-nft
    /// rms1qztwng6cty8cfm42nzvq099ev7udhrnk0rw8jt8vttf9kpqnxhpsx869vr3 "immutable metadata" "metadata"`
    MintNft {
        address: Option<String>,
        immutable_metadata: Option<String>,
        metadata: Option<String>,
    },
    /// Generate a new address.
    NewAddress,
    /// Send an amount to a bech32 encoded address: `send
    /// rms1qztwng6cty8cfm42nzvq099ev7udhrnk0rw8jt8vttf9kpqnxhpsx869vr3 1000000`
    Send { address: String, amount: u64 },
    /// Send an amount below the storage deposit minimum to a bech32 address: `send
    /// rms1qztwng6cty8cfm42nzvq099ev7udhrnk0rw8jt8vttf9kpqnxhpsx869vr3 1`
    SendMicro { address: String, amount: u64 },
    /// Send native tokens to a bech32 address: `send-native
    /// rms1qztwng6cty8cfm42nzvq099ev7udhrnk0rw8jt8vttf9kpqnxhpsx869vr3
    /// 08e3a2f76cc934bc0cc21575b4610c1d7d4eb589ae0100000000000000000000000000000000 10`
    SendNativeToken {
        address: String,
        token_id: String,
        amount: String,
    },
    /// Send an nft to a bech32 encoded address
    SendNft { address: String, nft_id: String },
    /// Sync the account with the Tangle.
    Sync,
    /// List the account transactions.
    Transactions,
}

/// `addresses` command
pub async fn addresses_command(account_handle: &AccountHandle) -> Result<(), Error> {
    let addresses = account_handle.list_addresses().await?;

    if addresses.is_empty() {
        log::info!("No addresses found");
    } else {
        for address in addresses {
            print_address(account_handle, &address).await?;
        }
    }

    Ok(())
}

// `balance` command
pub async fn balance_command(account_handle: &AccountHandle) -> Result<(), Error> {
    log::info!("{:?}", account_handle.balance().await?);

    Ok(())
}

// `consolidate` command
pub async fn consolidate_command(account_handle: &AccountHandle) -> Result<(), Error> {
    log::info!("Consolidating outputs.");

    account_handle.consolidate_outputs(true, None).await?;

    Ok(())
}

// `faucet` command
pub async fn faucet_command(
    account_handle: &AccountHandle,
    url: Option<String>,
    address: Option<String>,
) -> Result<(), Error> {
    let address = if let Some(address) = address {
        address
    } else {
        match account_handle.list_addresses().await?.last() {
            Some(address) => address.address().to_bech32(),
            None => return Err(Error::NoAddressForFaucet),
        }
    };
    let faucet_url = match &url {
        Some(faucet_url) => faucet_url,
        None => "http://localhost:14265/api/plugins/faucet/v1/enqueue",
    };

    log::info!("{}", request_funds_from_faucet(faucet_url, &address).await?);

    Ok(())
}

// `mint-native-token` command
pub async fn mint_native_token_command(
    account_handle: &AccountHandle,
    // todo: enable this when there is support to mint additional tokens for an existing token
    // circulating_supply: String,
    maximum_supply: String,
    foundry_metadata: Option<String>,
) -> Result<(), Error> {
    let native_token_options = NativeTokenOptions {
        account_address: None,
        circulating_supply: U256::from_dec_str(&maximum_supply).map_err(|e| Error::Miscellanous(e.to_string()))?,
        maximum_supply: U256::from_dec_str(&maximum_supply).map_err(|e| Error::Miscellanous(e.to_string()))?,
        foundry_metadata: foundry_metadata
            .map(|s| prefix_hex::decode(&s))
            .transpose()
            .map_err(|e| Error::Miscellanous(e.to_string()))?,
    };

    let transfer_result = account_handle.mint_native_token(native_token_options, None).await?;

    log::info!("Native token minting transaction sent: {transfer_result:?}");

    Ok(())
}

// `mint-nft` command
pub async fn mint_nft_command(
    account_handle: &AccountHandle,
    address: Option<String>,
    immutable_metadata: Option<String>,
    metadata: Option<String>,
) -> Result<(), Error> {
    let immutable_metadata = immutable_metadata.map(|immutable_metadata| immutable_metadata.as_bytes().to_vec());
    let metadata = metadata.map(|metadata| metadata.as_bytes().to_vec());
    let nft_options = vec![NftOptions {
        address,
        immutable_metadata,
        metadata,
    }];
    let transfer_result = account_handle.mint_nfts(nft_options, None).await?;

    log::info!("NFT minting transaction sent: {transfer_result:?}");

    Ok(())
}

// `new-address` command
pub async fn new_address_command(account_handle: &AccountHandle) -> Result<(), Error> {
    let address = account_handle.generate_addresses(1, None).await?;

    print_address(account_handle, &address[0]).await?;

    Ok(())
}

// `send` command
pub async fn send_command(account_handle: &AccountHandle, address: String, amount: u64) -> Result<(), Error> {
    let outputs = vec![AddressWithAmount { address, amount }];
    let transfer_result = account_handle.send_amount(outputs, None).await?;

    log::info!("Transaction created: {transfer_result:?}");

    Ok(())
}

// `send-micro` command
pub async fn send_micro_command(account_handle: &AccountHandle, address: String, amount: u64) -> Result<(), Error> {
    let outputs = vec![AddressWithMicroAmount {
        address,
        amount,
        return_address: None,
        expiration: None,
    }];

    let transfer_result = account_handle.send_micro_transaction(outputs, None).await?;

    log::info!("Micro transaction created: {transfer_result:?}");

    Ok(())
}

// `send-native-token` command
pub async fn send_native_token_command(
    account_handle: &AccountHandle,
    address: String,
    token_id: String,
    amount: String,
) -> Result<(), Error> {
    let outputs = vec![AddressNativeTokens {
        address,
        native_tokens: vec![(
            TokenId::from_str(&token_id)?,
            U256::from_dec_str(&amount).map_err(|e| Error::Miscellanous(e.to_string()))?,
        )],
        ..Default::default()
    }];
    let transfer_result = account_handle.send_native_tokens(outputs, None).await?;

    log::info!("Transaction created: {transfer_result:?}");

    Ok(())
}

// `send-nft` command
pub async fn send_nft_command(account_handle: &AccountHandle, address: String, nft_id: String) -> Result<(), Error> {
    let outputs = vec![AddressAndNftId {
        address,
        nft_id: NftId::from_str(&nft_id)?,
    }];
    let transfer_result = account_handle.send_nft(outputs, None).await?;

    log::info!("Transaction created: {transfer_result:?}");

    Ok(())
}

// `sync` command
pub async fn sync_command(account_handle: &AccountHandle) -> Result<(), Error> {
    let sync = account_handle.sync(None).await?;

    log::info!("Synced: {sync:?}");

    Ok(())
}

/// `transactions` command
pub async fn transactions_command(account_handle: &AccountHandle) -> Result<(), Error> {
    let transactions = account_handle.list_transactions().await?;

    if transactions.is_empty() {
        log::info!("No transactions found");
    } else {
        transactions.iter().for_each(print_transaction);
    }

    Ok(())
}

// `set-alias` command
// pub async fn set_alias_command(account_handle: &AccountHandle) -> Result<()> {
//     if let Some(matches) = matches.subcommand_matches("set-alias") {
//         let alias = matches.value_of("alias")?;
//         account_handle.set_alias(alias).await?;
//     }
//     Ok(())
// }

fn print_transaction(transaction: &Transaction) {
    log::info!("{transaction:?}");
    // if let Some(MessagePayload::Transaction(tx)) = message.payload() {
    //     let TransactionEssence::Regular(essence) = tx.essence();
    //     println!("--- Value: {:?}", essence.value());
    // }
    // println!("--- Timestamp: {:?}", message.timestamp());
    // println!(
    //     "--- Broadcasted: {}, confirmed: {}",
    //     message.broadcasted(),
    //     match message.confirmed() {
    //         Some(c) => c.to_string(),
    //         None => "unknown".to_string(),
    //     }
    // );
}

pub async fn print_address(account_handle: &AccountHandle, address: &AccountAddress) -> Result<(), Error> {
    let mut log = format!("Address {}: {}", address.key_index(), address.address().to_bech32());

    if *address.internal() {
        log = format!("{log}\nChange address");
    }

    let addresses_with_balance = account_handle.list_addresses_with_unspent_outputs().await?;

    if let Ok(index) = addresses_with_balance.binary_search_by_key(&(address.key_index(), address.internal()), |a| {
        (a.key_index(), a.internal())
    }) {
        log = format!("{log}\nBalance: {}", addresses_with_balance[index].amount());
        log = format!("{log}\nOutputs: {:#?}", addresses_with_balance[index].output_ids());
    }

    log::info!("{log}");

    Ok(())
}

// Copyright 2020-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::{Args, Parser, Subcommand};
use iota_wallet::{
    account::{OutputsToCollect, SyncOptions},
    account_manager::AccountManager,
    iota_client::{secret::SecretManager, utils::generate_mnemonic},
    ClientOptions,
};

use crate::{account::account_prompt, error::Error};

#[derive(Debug, Parser)]
#[clap(version, long_about = None)]
#[clap(propagate_version = true)]
pub struct AccountManagerCli {
    #[clap(subcommand)]
    pub command: Option<AccountManagerCommand>,
    pub account: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum AccountManagerCommand {
    /// Initialize the wallet with a mnemonic and node url, if nothing is provided, a new mnemonic will be generated and "http://localhost:14265" used.
    Init(MnemonicAndUrl),
    /// Create a new account with an optional alias.
    New { alias: Option<String> },
    /// Set the node to use.
    SetNode { url: String },
    /// Sync all accounts.
    Sync,
}

#[derive(Debug, Args)]
pub struct MnemonicAndUrl {
    #[clap(short, long)]
    pub mnemonic: Option<String>,
    #[clap(short, long)]
    pub node: Option<String>,
}

pub async fn init_command(
    secret_manager: SecretManager,
    storage_path: String,
    mnemonic_url: MnemonicAndUrl,
) -> Result<AccountManager, Error> {
    let account_manager = AccountManager::builder()
        .with_secret_manager(secret_manager)
        .with_client_options(
            ClientOptions::new()
                .with_node(mnemonic_url.node.as_deref().unwrap_or("http://localhost:14265"))?
                .with_node_sync_disabled(),
        )
        .with_storage_path(&storage_path)
        .finish()
        .await?;

    let mnemonic = match mnemonic_url.mnemonic {
        Some(mnemonic) => mnemonic,
        None => generate_mnemonic()?,
    };
    log::info!("IMPORTANT: write this mnemonic phrase in a safe place.");
    log::info!(
        "It is the only way to recover your account if you ever forget your password and/or lose the stronghold file."
    );
    // Specific target to easily exclude it from the archive logger output.
    log::info!(target:"mnemonic", "{mnemonic}");

    if let SecretManager::Stronghold(secret_manager) = &mut *account_manager.get_secret_manager().write().await {
        secret_manager.store_mnemonic(mnemonic).await?;
    } else {
        panic!("cli-wallet only supports Stronghold-backed secret managers at the moment.");
    }
    log::info!("Mnemonic stored successfully");

    Ok(account_manager)
}

pub async fn new_command(manager: &AccountManager, alias: Option<String>) -> Result<(), Error> {
    let mut builder = manager.create_account();

    if let Some(alias) = alias {
        builder = builder.with_alias(alias);
    }

    let account_handle = builder.finish().await?;

    log::info!("Created account \"{}\"", account_handle.read().await.alias());

    account_prompt(account_handle).await?;

    Ok(())
}

pub async fn set_node_command(manager: &AccountManager, url: String) -> Result<(), Error> {
    manager
        .set_client_options(ClientOptions::new().with_node(&url)?.with_node_sync_disabled())
        .await?;

    Ok(())
}

pub async fn sync_command(manager: &AccountManager) -> Result<(), Error> {
    let total_balance = manager
        .sync(Some(SyncOptions {
            try_collect_outputs: OutputsToCollect::All,
            ..Default::default()
        }))
        .await?;

    log::info!("Synchronized all accounts: {:?}", total_balance);

    Ok(())
}

use crate::utils::{get_new_pk_and_authenticator, initialize_client};
use std::{fs, path::Path};

use miden_client::{
    ClientError, Felt,
    account::{AccountStorageMode, AccountType},
    transaction::{TransactionKernel, TransactionRequestBuilder},
};

use miden_objects::{
    account::{AccountBuilder, AccountComponent, StorageSlot},
    assembly::Assembler,
};

use rand::Rng;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use tokio::time::Duration;

pub async fn add_u256() -> Result<(), ClientError> {
    // Initialize client
    let mut client = initialize_client().await?;
    println!("Client initialized successfully.");

    // Fetch latest block from node
    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating contract.");

    // Load the MASM file for the contract
    let file_path = Path::new("./masm/accounts/add_u256.masm");
    let account_code = fs::read_to_string(file_path).unwrap();

    // Prepare assembler (debug mode = true)
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Compile the account code into `AccountComponent` with one storage slot
    let component = AccountComponent::compile(
        account_code,
        assembler,
        vec![StorageSlot::Value([
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
        ])],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the contract
    let init_seed = ChaCha20Rng::from_entropy().r#gen();

    // Anchor block of the account
    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Build the new `Account` with the component
    let (contract, seed) = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(component.clone())
        .build()
        .unwrap();

    println!("contract hash: {:?}", contract.hash().to_hex());
    println!("contract id: {:?}", contract.id().to_hex());
    println!("account_storage: {:?}", contract.storage());

    // Since anyone should be able to write to the contract, auth_secret_key is not required.
    // However, to import to the client, we must generate a random value.
    let (_pub_key, auth_secret_key) = get_new_pk_and_authenticator();

    client
        .add_account(&contract.clone(), Some(seed), &auth_secret_key, false)
        .await
        .unwrap();

    // Print the procedure root hash
    let get_add_export = component
        .library()
        .exports()
        .find(|export| export.name.as_str() == "add_u256")
        .unwrap();

    let get_add_mast_id = component.library().get_export_node_id(get_add_export);

    let add_hash = component
        .library()
        .mast_forest()
        .get_node_by_id(get_add_mast_id)
        .unwrap()
        .digest()
        .to_hex();

    println!("increment_count procedure hash: {:?}", add_hash);

    // -------------------------------------------------------------------------
    // STEP 2: Call the Contract with a script
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Call Contract With Script");

    // Load the MASM script referencing the increment procedure
    let file_path = Path::new("./masm/scripts/add_u256_script.masm");
    let original_code = fs::read_to_string(file_path).unwrap();

    // Replace the placeholder with the actual procedure call
    let replaced_code = original_code.replace("{add_u256}", &add_hash);
    println!("Final script:\n{}", replaced_code);

    // Compile the script referencing our procedure
    let tx_script = client.compile_tx_script(vec![], &replaced_code).unwrap();

    // Build a transaction request with the custom script
    let tx_request = TransactionRequestBuilder::new()
        .with_custom_script(tx_script)
        .unwrap()
        .build();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(contract.id(), tx_request)
        .await
        .unwrap();

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // Submit transaction to the network
    let _ = client.submit_transaction(tx_result).await;

    // Wait, then re-sync
    tokio::time::sleep(Duration::from_secs(3)).await;
    client.sync_state().await.unwrap();

    let account = client.get_account(contract.id()).await.unwrap();
    println!(
        "contract storage: {:?}",
        account.unwrap().account().storage().get_item(0)
    );

    Ok(())
}

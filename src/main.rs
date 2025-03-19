pub mod add_u256;
pub mod utils;

use miden_client::ClientError;
use std::env;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    let args: Vec<String> = env::args().collect();
    let name = &args[1];

    match name.as_str() {
        "add_u256" => add_u256::add_u256().await,
        &_ => todo!(),
    }
}

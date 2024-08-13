#![cfg(test)]

use eyre::Result;
use tracing_test::traced_test;

use crate::jwt::get_jwt;

pub(crate) mod jwt;

pub(crate) const URL: &str = "http://localhost:9042";

#[traced_test]
#[tokio::test]
async fn test_deposite() -> Result<()> {
    let _jwt = get_jwt().await;
    //
    Ok(())
}

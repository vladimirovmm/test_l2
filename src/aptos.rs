use eyre::{ensure, Context, ContextCompat, Result};
use futures::future::try_join_all;
use reqwest::StatusCode;
use tokio::test;
use tracing::{debug, instrument};
use tracing_test::traced_test;

// ---
// profiles:
//   alice:
//     private_key: "0x170e9218f1b8ccb44f9877ce423364021756fa438207af1f594e955e3131b0fd"
//     public_key: "0xdad2adbcf857ccf8f610d0a44f19a82f74f24e1142effd03bad571a5dc86f7a2"
//     account: 5e67137f218ca70760ff0a7d792cb4286b5a80fd81c66191d5a0412e161ec0ea
//     rest_url: "http://localhost:8080"
//     faucet_url: "http://localhost:8081"
//   bob:
//     private_key: "0xcae6621dc96fbe5d09cbfe925bbfffec8a88b7765d275561f9fa03c24b660cf8"
//     public_key: "0x9eea1f4c7e33bb33c4f588ee13ee6948467fcf6a2f91eef8a0a434ae6ca9a60d"
//     account: 12ebe3e67d11259a82646bffc7caff724ab61e9cbefc2c80df255986351f135c
//     rest_url: "http://localhost:8080"
//     faucet_url: "http://localhost:8081"
//   eve:
//     private_key: "0x5f0af78aad7bfd6445c0d9b179f92c7b1d6561acc4bb4d4dcf077caa1fbc7026"
//     public_key: "0xefe6d3a3bf426c4576cdf1b5617120d53c1192043c2a37917666dfc7ba331555"
//     account: 04228e4f14a6f2f8d202f1bbe151aaadf1105d1fc3c9c0dc1804f5773c34d62b
//     rest_url: "http://localhost:8080"
//     faucet_url: "http://localhost:8081"
pub(crate) const APTOS_ACCOUNTS: [&str; 3] = [
    "5e67137f218ca70760ff0a7d792cb4286b5a80fd81c66191d5a0412e161ec0ea", // alice
    "12ebe3e67d11259a82646bffc7caff724ab61e9cbefc2c80df255986351f135c", // bob
    "04228e4f14a6f2f8d202f1bbe151aaadf1105d1fc3c9c0dc1804f5773c34d62b", // eve
];
const URL: &str = "http://localhost:8080";

// $ aptos account list --query balance --account <ACCOUNT>
// $ curl --request GET --url https://api.devnet.aptoslabs.com/v1/accounts/<__ADDRESS__>/resource/<__RESOURCE_TYPE__>
#[instrument(level = "debug")]
async fn balance(account: &str) -> Result<usize> {
    let url = format!(
        "{URL}/v1/accounts/{account}/resource/0x1::coin::CoinStore<0x1::aptos_coin::AptosCoin>"
    );
    let response = reqwest::get(&url)
        .await
        .with_context(|| format!("При обращении к {url} возникла ошибка"))?;

    let status = response.status();
    debug!("status: {:?}", response.status());

    // аккаунта не найден/не существует
    if StatusCode::NOT_FOUND == status {
        return Ok(0);
    }
    ensure!(
        status == StatusCode::OK,
        "Неудалось получить ресурсы аккаунта {account}. Url: {url:?}. Status: {status:?}"
    );

    let body = response
        .json::<serde_json::Value>()
        .await
        .with_context(|| {
            format!("При декоидровании ответа в json произошлка ошибка. Url: {url} ")
        })?;
    debug!("response: {body:#?}");

    body.get("data")
        .and_then(|data| data.get("coin"))
        .and_then(|coin| coin.get("value"))
        .and_then(|value| value.as_str())
        .with_context(|| format!("Неудалось извлечь количество монет из ответа {body:#?}"))?
        .parse()
        .with_context(|| format!("Неудалось преобразовать количество монет в usize {body:#?}"))
}

#[ignore]
#[test]
#[traced_test]
async fn test_balance() -> Result<()> {
    let tasks = APTOS_ACCOUNTS
        .iter()
        .map(|account| balance(account))
        .collect::<Vec<_>>();
    APTOS_ACCOUNTS
        .iter()
        .zip(try_join_all(tasks).await?)
        .for_each(|(account, balance)| {
            debug!("0x{account}: {balance}");
        });

    Ok(())
}

#![cfg(test)]

use std::sync::LazyLock;

use eyre::{Context, Result};
use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
use jwt_jsonrpsee::ClientLayer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing::debug;
use tracing_test::traced_test;

use crate::jwt::get_jwt;

pub(crate) mod aptos;
pub(crate) mod jwt;

type Slot = usize;

pub(crate) const URL: &str = "http://localhost:9042";
static NEXT_SLOL: LazyLock<Mutex<Slot>> = LazyLock::new(|| Mutex::new(1));

async fn next_slot() -> Slot {
    let mut slot = NEXT_SLOL.lock().await;
    *slot += 1;
    *slot
}

#[traced_test]
#[tokio::test]
async fn test_deposite() -> Result<()> {
    let jwt = get_jwt().await;
    let client = HttpClientBuilder::new()
        .set_http_middleware(tower::ServiceBuilder::new().layer(ClientLayer::new(jwt)))
        .build(URL)
        .context("Ошибка при попытки создать клиента для service-engine")?;

    let response = client
        .request::<Value, _>("engine_l2Info_v1", rpc_params![])
        .await
        .context("запрос на депозит")?;

    debug!("{}", serde_json::to_string_pretty(&response).unwrap());

    client
        .request::<Value, _>(
            "engine_applyAttributes_v1",
            rpc_params!(json!({
                "parent_payload": 0,
                "events": [],
                "max_payload_size": 1001,
            })),
        )
        .await
        .context("Пустой массив событий")
        .unwrap();

    client
        .request::<Value, _>(
            "engine_applyAttributes_v1",
            rpc_params!(json!({
                "parent_payload": 0,
                "events": [
                    {
                        "slot": next_slot().await,
                        "events":[]
                    }
                ],
                "max_payload_size": 1001,
            })),
        )
        .await
        .context("Пустой массив событий")
        .unwrap();

    let response = client
        .request::<Value, _>(
            "engine_applyAttributes_v1",
            rpc_params!(json!({
                "parent_payload": 1,
                "max_payload_size": 1001,
                "events": [
                    {
                        "slot": next_slot().await,
                        "events":[
                            {
                                "Deposit":{
                                    "account":"0x0000000000000000000000000000000000000000000000000000000000000001",
                                    "amount":1
                                }
                            }
                        ]
                    }
                ],
            })),
        )
        .await
        .context("запрос на депозит")?;
    debug!("{}", serde_json::to_string_pretty(&response).unwrap());

    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RequestEngine {
    parent_payload: Slot,
    max_payload_size: Slot,
    events: Vec<RequestDeposit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RequestDeposit {
    slot: Slot,
    events: Vec<TxDeposit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxDeposit {
    account: String,
    amount: u64,
}

#![cfg(test)]

use std::sync::LazyLock;

use aptos::APTOS_ACCOUNTS;
use eyre::{Context, Result};
use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
use jwt_jsonrpsee::ClientLayer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing::{debug, instrument};
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

#[instrument(level = "debug", skip(client))]
async fn engine_l2info_v1<T>(client: &T) -> Result<Value>
where
    T: ClientT,
{
    debug!("Получинеие информации о текущем состоянии ноды. engine_l2Info_v1: request");

    client
        .request::<Value, _>("engine_l2Info_v1", rpc_params![])
        .await
        .context("запрос на получения статуса ноды")
}

#[traced_test]
#[tokio::test]
async fn test_deposite() -> Result<()> {
    let jwt = get_jwt().await;
    let client = HttpClientBuilder::new()
        .set_http_middleware(tower::ServiceBuilder::new().layer(ClientLayer::new(jwt)))
        .build(URL)
        .context("Ошибка при попытки создать клиента для service-engine")?;

    debug!("response: {:#?}", engine_l2info_v1(&client).await?);

    debug!("Запрос с пустым массивом событий");
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

    debug!("Запрос с пустым массивом событий слота");
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

    debug!("Пример запроса через json");
    let response: Value = client
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
    debug!("response: {response:#?}");

    debug!("Запрос на пополнение нескольких аккаунтов (engine_applyAttributes_v1)");
    let response: Value = client
        .request::<Value, _>(
            "engine_applyAttributes_v1",
            rpc_params!(RequestEngine::all().await),
        )
        .await
        .context("запрос на депозит")?;
    debug!("response: {response:#?}");

    debug!("response: {:#?}", engine_l2info_v1(&client).await?);

    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RequestEngine {
    parent_payload: Slot,
    max_payload_size: Slot,
    events: Vec<RequestSlot>,
}

impl RequestEngine {
    async fn all() -> Self {
        Self {
            parent_payload: 1,
            max_payload_size: 1001,
            events: vec![RequestSlot {
                slot: next_slot().await,
                events: vec![
                    // Alice
                    RequestEvent::Deposit(TxDeposit {
                        account: APTOS_ACCOUNTS[0].into(),
                        amount: 1001,
                    }),
                    // Bob
                    RequestEvent::Deposit(TxDeposit {
                        account: APTOS_ACCOUNTS[1].into(),
                        amount: 1002,
                    }),
                    // Eve
                    RequestEvent::Deposit(TxDeposit {
                        account: APTOS_ACCOUNTS[2].into(),
                        amount: 1003,
                    }),
                    // 0x0
                    RequestEvent::Deposit(TxDeposit {
                        account: "0".repeat(64),
                        amount: 1003,
                    }),
                    // 0x1
                    RequestEvent::Deposit(TxDeposit {
                        account: format!("{:0>64}", "1"),
                        amount: 1003,
                    }),
                ],
            }],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RequestSlot {
    slot: Slot,
    events: Vec<RequestEvent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
enum RequestEvent {
    Deposit(TxDeposit),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxDeposit {
    account: String,
    amount: u64,
}

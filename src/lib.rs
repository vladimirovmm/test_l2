#![cfg(test)]

use std::{fs, path::PathBuf, str::FromStr, time::Duration};

use async_once_cell::OnceCell;
use eyre::{bail, Context, ContextCompat, Result};
use headers::authorization::{Bearer, Credentials};
use jsonrpsee::http_client::HttpClientBuilder;
use jsonrpsee::{core::client::ClientT, rpc_params};
use jwt_jsonrpsee::{Claims, ClientLayer, JwtSecret};
use rand::random;
use reqwest::{header::HeaderValue, StatusCode};
use serde_yaml::Value;
use tokio::{test, time::sleep};
use tracing::{debug, info};
use tracing_test::traced_test;

const URL: &str = "http://localhost:9042";
const JWT_PATH: &str = "engine.jwt";

static JWT: OnceCell<JwtSecret> = OnceCell::new();

async fn get_jwt() -> JwtSecret {
    *JWT.get_or_init(async {
        JwtSecret::from_str(
            &fs::read_to_string(JWT_PATH)
                .with_context(|| format!("При чтении JWT {JWT_PATH:?} произошла ошибка"))
                .unwrap(),
        )
        .context("Неудолось декодировать JWT")
        .unwrap()
    })
    .await
}

async fn req_status(token: HeaderValue) -> Result<StatusCode> {
    let status = reqwest::Client::new()
        .get(URL)
        .header(reqwest::header::AUTHORIZATION, token)
        .send()
        .await?
        .status();
    Ok(status)
}

#[traced_test]
#[tokio::test]
async fn test_deposite() -> Result<()> {
    let _jwt = get_jwt().await;
    //
    Ok(())
}

#[tokio::test]
async fn test_unath() {
    assert_eq!(
        reqwest::get(URL)
            .await
            .with_context(|| format!("Ошибка при обращении на {URL:?}"))
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "Запросы без токена не должны приниматься"
    );
}

#[tokio::test]
async fn test_auth_reqwest() -> Result<()> {
    assert_eq!(
        req_status(get_jwt().await.to_bearer()?).await?,
        reqwest::StatusCode::METHOD_NOT_ALLOWED,
        "Ожидалось что это валидный токен и метода не существует"
    );

    Ok(())
}

#[test]
async fn test_invalid_jwt() -> Result<()> {
    let token = JwtSecret::new(random()).to_bearer()?;

    assert_eq!(
        req_status(token).await?,
        reqwest::StatusCode::UNAUTHORIZED,
        "Был принят невалидный токен"
    );

    Ok(())
}

#[test]
async fn test_token_lifetime_has_expired() -> Result<()> {
    const CLAIM_EXPIRATION: u64 = 2;

    let jwt = get_jwt().await;
    let token = {
        let jwt_as_bytes =
            hex::decode(jwt.to_string()).context("Не удалось преобразовать в Vec<u8> JWT")?;
        let claim = jsonwebtoken::encode(
            &Default::default(),
            // Expires in 30 secs from now
            &Claims::with_expiration(CLAIM_EXPIRATION),
            &jsonwebtoken::EncodingKey::from_secret(&jwt_as_bytes),
        )
        .context("Неудалось создать claim")?;

        HeaderValue::from_str(&format!("{} {claim}", Bearer::SCHEME))?
    };

    assert_eq!(
        req_status(token.clone()).await?,
        reqwest::StatusCode::METHOD_NOT_ALLOWED,
        "Ожидалось что токен валидный и метода не существует"
    );
    sleep(Duration::from_secs(CLAIM_EXPIRATION + 1)).await;

    assert_eq!(
        req_status(token).await?,
        reqwest::StatusCode::UNAUTHORIZED,
        "Токен должен был истечь"
    );

    Ok(())
}

#[test]
async fn test_jsonrpsee() -> Result<()> {
    let jwt = get_jwt().await;

    fn unwrap_call_auth<T>(result: Result<T, jsonrpsee::core::ClientError>) -> Result<bool> {
        use jsonrpsee::core::ClientError::{Call, Transport};

        let result = match result.err().context("Ожидалась ошибка")? {
            Call(_) => true,
            Transport(err) => !err.to_string().contains("401"),
            err => bail!("Для этого типи нет оброботчика. {err}"),
        };
        Ok(result)
    }

    // without JWT
    let client = HttpClientBuilder::new().build(URL).unwrap();
    let response = client.request::<String, _>("hello", rpc_params![]).await;
    assert!(
        !unwrap_call_auth(response)?,
        "Сервис не должен принемать без токена"
    );

    // with JWT
    let client = HttpClientBuilder::new()
        .set_http_middleware(tower::ServiceBuilder::new().layer(ClientLayer::new(jwt)))
        .build(URL)
        .unwrap();

    let response = client.request::<String, _>("hello", rpc_params![]).await;
    assert!(
        unwrap_call_auth(response)?,
        "Токен был отправлен и был отклонён"
    );

    Ok(())
}
/// Добавить ключ в конфиг. По умолчанию он генерируется при запуске
#[ignore]
#[traced_test]
#[tokio::test]
async fn patch_node_config() -> Result<()> {
    const CONFIG_FILE_NAME: &str = "node.yaml";
    const DEFAULT_JWT_PATH: &str = "l2/test-node/engine.jwt";

    debug!("Чтение конфига из {CONFIG_FILE_NAME:?}");
    let config_str = fs::read_to_string(CONFIG_FILE_NAME).context("Неудалось открыть конфиг")?;
    let mut config: Value =
        serde_yaml::from_str(&config_str).context("При десериализации конфига произошла ошибка")?;

    debug!("Проверка на существование поля в конфиге engine_service::jwt_path");
    let engine_service = config
        .as_mapping_mut()
        .context("Не валидный конфиг. Ожидался Mapping")?
        .entry(Value::String("engine_service".into()))
        .or_insert(Value::Mapping(Default::default()))
        .as_mapping_mut()
        .context("Не валидный конфиг `engine_service`. Ожидался Mapping")?;

    if engine_service.contains_key("jwt_path") {
        debug!("JWT уже существует в конфиге");
        return Ok(());
    }

    debug!("Генерация JWT");
    let jwt = JwtSecret::new(random());
    debug!("Сохранение JWT");
    fs::write(JWT_PATH, jwt.to_string()).context("Ошибка при сохранении JWT")?;
    let full_jwt_path = PathBuf::from(JWT_PATH).canonicalize().unwrap();
    info!("Ключ сохранен в {full_jwt_path:?}");

    engine_service.insert(
        "jwt_path".to_string().into(),
        DEFAULT_JWT_PATH.to_string().into(),
    );
    debug!("Путь до JWT установлен в конфиге {DEFAULT_JWT_PATH}");

    fs::write(CONFIG_FILE_NAME, serde_yaml::to_string(&config).unwrap())
        .context("Неудалось записать конфиг в {FILE_NAME}")?;
    let full_config_path = PathBuf::from(CONFIG_FILE_NAME).canonicalize().unwrap();
    info!("Конфиг успешно сохранён в {full_config_path:?}");

    info!("Перенесите {full_config_path:?} и {full_jwt_path:?} в нужную директорию и если путь до ключа отличается измените на нужный");

    Ok(())
}

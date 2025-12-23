use google_sheets4::{api::ValueRange, Sheets};
use yup_oauth2::{ServiceAccountAuthenticator, parse_service_account_key};
use std::env;

// Всё что здесь есть это страшный костыль для сбора метрик, не кидайтесь ссаными тряпками, мы это потом выпилим нафиг.

const DEFAULT_USER: &str = "CI_CD_BOT";

pub struct MetricsClient {
    hub: Sheets<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>,
    spreadsheet_id: String,
}

impl MetricsClient {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let spreadsheet_id = "1Wk85U6XPxNsYD0dh1xscW2FjmA1gRoXIzpDlZ6qKrS4";

        let creds_json = env::var("GOOGLE_CREDENTIALS")
            .map_err(|_| "ОШИБКА: Переменная GOOGLE_CREDENTIALS не найдена. Проверьте секреты GitHub или окружение.")?;

        let secret = parse_service_account_key(creds_json)?;

        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();

        let auth = ServiceAccountAuthenticator::builder(secret).build().await?;

        let client = hyper::Client::builder().build(connector);

        let hub = Sheets::new(client, auth);

        Ok(Self {
            hub,
            spreadsheet_id: spreadsheet_id.to_string()
        })
    }

    pub async fn add_metric(&self, username: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.internal_increment(username).await
    }

    pub async fn add_default_metric(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.internal_increment(DEFAULT_USER).await
    }

    async fn internal_increment(&self, username: &str) -> Result<(), Box<dyn std::error::Error>> {
        let range = "Лист1!A:B"; 

        let response = self.hub.spreadsheets().values_get(&self.spreadsheet_id, range)
            .doit().await?;

        let rows = response.1.values.unwrap_or_default();
        let mut user_row_index = None;
        let mut current_value = 0i64;

        for (idx, row) in rows.iter().enumerate() {
            if let Some(name_val) = row.get(0).and_then(|v| v.as_str()) {
                if name_val == username {
                    user_row_index = Some(idx + 1); 
                    current_value = row.get(1)
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    break;
                }
            }
        }

        let new_value = current_value + 1;

        if let Some(row_num) = user_row_index {
            let update_range = format!("Лист1!B{}", row_num);
            let req = ValueRange {
                values: Some(vec![vec![serde_json::Value::String(new_value.to_string())]]),
                ..Default::default()
            };

            self.hub.spreadsheets().values_update(req, &self.spreadsheet_id, &update_range)
                .value_input_option("USER_ENTERED")
                .doit().await?;
        } else {
            let req = ValueRange {
                values: Some(vec![vec![
                    serde_json::Value::String(username.to_string()),
                    serde_json::Value::String("1".to_string()),
                ]]),
                ..Default::default()
            };

            self.hub.spreadsheets().values_append(req, &self.spreadsheet_id, "Лист1!A1")
                .value_input_option("USER_ENTERED")
                .doit().await?;
        }

        Ok(())
    }
}
use super::types::DepthSnapshot;

pub trait Rest {
    type Error: std::fmt::Debug + Send;

    fn get_orderbook(
        &self,
        symbol: &str,
    ) -> impl std::future::Future<Output = Result<DepthSnapshot, Self::Error>> + Send;
}

/// Binance USD-Margin API
pub struct UM;

impl UM {
    pub fn rest_url(&self) -> &str {
        "https://fapi.binance.com"
    }

    pub fn ws_url(&self) -> &str {
        "wss://fstream.binance.com"
    }
}

impl Rest for UM {
    type Error = reqwest::Error;

    async fn get_orderbook(&self, symbol: &str) -> Result<DepthSnapshot, Self::Error> {
        let client = reqwest::Client::new();
        let url = format!("{}/fapi/v1/depth", self.rest_url());

        let resp = client
            .get(&url)
            .query(&[("symbol", symbol), ("limit", "1000")])
            .send()
            .await?;

        let data = resp.json::<DepthSnapshot>().await?;
        Ok(data)
    }
}

use crate::error::MwApiError;
use awb_domain::types::*;

/// Parse a standard MediaWiki query list response into Titles
pub fn parse_list_response(resp: &serde_json::Value, list_key: &str) -> Vec<Title> {
    resp["query"][list_key]
        .as_array()
        .map(|arr| {
            arr.iter().filter_map(|item| {
                let ns = Namespace(item["ns"].as_i64()? as i32);
                let name = item["title"].as_str()?.to_string();
                Some(Title { namespace: ns, name: name.clone(), display: name })
            }).collect()
        })
        .unwrap_or_default()
}

/// Get the continuation token from a response
pub fn get_continue_token(resp: &serde_json::Value, key: &str) -> Option<String> {
    resp["continue"][key].as_str().map(String::from)
}

/// Fetch all pages from a list endpoint using continuation
pub async fn fetch_all_pages(
    client: &reqwest::Client,
    api_url: &url::Url,
    base_params: &[(&str, &str)],
    list_key: &str,
    continue_key: &str,
) -> Result<Vec<Title>, MwApiError> {
    let mut all_titles = Vec::new();
    let mut continue_token: Option<String> = None;

    loop {
        let mut params: Vec<(&str, String)> = base_params.iter()
            .map(|(k, v)| (*k, v.to_string()))
            .collect();
        params.push(("format", "json".to_string()));
        if let Some(ref token) = continue_token {
            params.push((continue_key, token.clone()));
            params.push(("continue", "-||".to_string()));
        }

        let query_params: Vec<(&str, &str)> = params.iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let resp: serde_json::Value = client.get(api_url.as_str())
            .query(&query_params)
            .send().await?
            .json().await?;

        // Check for errors
        if let Some(error) = resp.get("error") {
            let code = error["code"].as_str().unwrap_or("unknown").to_string();
            let info = error["info"].as_str().unwrap_or("").to_string();
            return Err(MwApiError::ApiError { code, info });
        }

        let titles = parse_list_response(&resp, list_key);
        all_titles.extend(titles);

        match get_continue_token(&resp, continue_key) {
            Some(token) => continue_token = Some(token),
            None => break,
        }
    }

    Ok(all_titles)
}

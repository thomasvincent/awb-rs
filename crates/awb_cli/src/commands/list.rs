use anyhow::{Context, Result};
use awb_domain::types::Title;
use awb_mw_api::list_endpoints::fetch_all_pages;
use console::style;
use url::Url;

use crate::ListSource;

pub async fn run(wiki: Url, source: ListSource, query: String, limit: usize) -> Result<()> {
    println!("{}", style("Fetching page list").bold().cyan());
    println!("Wiki: {}", wiki);
    println!("Source: {:?}", source);
    println!("Query: {}", query);
    println!();

    let titles = match source {
        ListSource::Category => fetch_category_members(&wiki, &query, limit).await?,
        ListSource::WhatLinksHere => fetch_what_links_here(&wiki, &query, limit).await?,
        ListSource::Search => fetch_search_results(&wiki, &query, limit).await?,
        ListSource::File => fetch_from_file(&query).await?,
    };

    println!("{} Found {} pages:", style("✓").green().bold(), style(titles.len()).yellow().bold());
    println!();

    for (i, title) in titles.iter().enumerate().take(limit.max(1)) {
        println!("  {}. {}", style(i + 1).dim(), title.display);
    }

    if titles.len() > limit && limit > 0 {
        println!();
        println!("  {} (showing first {} of {} total)",
            style("...").dim(),
            limit,
            titles.len()
        );
    }

    Ok(())
}

async fn fetch_category_members(api_url: &Url, category: &str, limit: usize) -> Result<Vec<Title>> {
    let client = reqwest::Client::new();
    let category_title = if category.starts_with("Category:") {
        category.to_string()
    } else {
        format!("Category:{}", category)
    };

    let base_params = [
        ("action", "query"),
        ("list", "categorymembers"),
        ("cmtitle", &category_title),
        ("cmlimit", "500"),
    ];

    let mut titles = fetch_all_pages(
        &client,
        api_url,
        &base_params,
        "categorymembers",
        "cmcontinue"
    ).await.context("Failed to fetch category members")?;

    if limit > 0 && titles.len() > limit {
        titles.truncate(limit);
    }

    Ok(titles)
}

async fn fetch_what_links_here(api_url: &Url, page: &str, limit: usize) -> Result<Vec<Title>> {
    let client = reqwest::Client::new();
    let base_params = [
        ("action", "query"),
        ("list", "backlinks"),
        ("bltitle", page),
        ("bllimit", "500"),
    ];

    let mut titles = fetch_all_pages(
        &client,
        api_url,
        &base_params,
        "backlinks",
        "blcontinue"
    ).await.context("Failed to fetch backlinks")?;

    if limit > 0 && titles.len() > limit {
        titles.truncate(limit);
    }

    Ok(titles)
}

async fn fetch_search_results(api_url: &Url, search_query: &str, limit: usize) -> Result<Vec<Title>> {
    let client = reqwest::Client::new();
    let base_params = [
        ("action", "query"),
        ("list", "search"),
        ("srsearch", search_query),
        ("srlimit", "500"),
    ];

    let mut titles = fetch_all_pages(
        &client,
        api_url,
        &base_params,
        "search",
        "sroffset"
    ).await.context("Failed to fetch search results")?;

    if limit > 0 && titles.len() > limit {
        titles.truncate(limit);
    }

    Ok(titles)
}

async fn fetch_from_file(file_path: &str) -> Result<Vec<Title>> {
    let content = tokio::fs::read_to_string(file_path)
        .await
        .context("Failed to read file")?;

    let titles: Vec<Title> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let trimmed = line.trim();
            awb_domain::types::Title::new(awb_domain::types::Namespace::MAIN, trimmed)
        })
        .collect();

    Ok(titles)
}

use std::collections::HashMap;
use std::env;
use std::process::exit;

use anyhow::{anyhow, Result};
use askama::Template;
use chrono::prelude::*;

// https://www.ncbi.nlm.nih.gov/books/NBK25497/
// https://www.ncbi.nlm.nih.gov/pmc/tools/get-metadata/
const PM_BASE_URL: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} <PMID>...", args[0]);
        exit(1);
    }

    let ids = args[1..].to_vec();

    for id in ids {
        let url =
            PM_BASE_URL.to_owned() + &format!("esummary.fcgi?db=pubmed&id={}&retmode=json", id);

        let result = match fetch_data(&url) {
            Ok(data) => data,
            Err(err) => {
                println!("Failed to fetch data for PMID {} ({}): {}", id, url, err);
                exit(1);
            }
        };

        let article_data = &result["result"][&id];

        let yml = match gen_yml(article_data) {
            Ok(yml) => yml,
            Err(err) => {
                println!("Failed to generate YAML for PMID {}: {}", id, err);
                exit(1);
            }
        };

        println!("{}", yml);
    }
}

fn fetch_data(url: &str) -> Result<serde_json::Value> {
    let response = reqwest::blocking::get(url)?;
    let meta_data = response.json::<serde_json::Value>()?;
    Ok(meta_data)
}

#[derive(Template)]
#[template(path = "bibliography.yml")]
struct BibliographyTemplate<'a> {
    authors: &'a Vec<&'a str>,
    doi: &'a str,
    pmid: &'a str,
    fulljournalname: Option<&'a str>,
    pubdate: &'a str,
    title: &'a str,
    uid: &'a str,
}

fn gen_yml(data: &serde_json::Value) -> Result<String> {
    let authors = extract_authors(data)?;
    let ids = extract_ids(data)?;
    let pubdate = extract_pubdate(data)?;

    let template = BibliographyTemplate {
        authors: &authors,
        doi: ids["doi"],
        pmid: ids["pubmed"],
        fulljournalname: data["fulljournalname"].as_str(),
        pubdate: pubdate.as_str(),
        title: data["title"].as_str().ok_or(anyhow!("no title found"))?,
        uid: data["uid"].as_str().ok_or(anyhow!("no UID found"))?,
    };

    let rendered = template.render()?;
    Ok(rendered)
}

fn extract_authors(data: &serde_json::Value) -> Result<Vec<&str>> {
    let authors = data["authors"]
        .as_array()
        .ok_or(anyhow!("no authors found"))?
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();

    Ok(authors)
}

fn extract_pubdate(data: &serde_json::Value) -> Result<String> {
    let pubdate = data["history"]
        .as_array()
        .ok_or(anyhow!("no history found"))?
        .iter()
        .find(|h| h["pubstatus"].as_str().unwrap() == "pubmed")
        .ok_or(anyhow!("no pubmed history found"))?["date"]
        .as_str()
        .unwrap();

    let pubdate_iso8861 =
        NaiveDateTime::parse_from_str(pubdate, "%Y/%m/%d %H:%M")?.format("%Y-%m-%d");

    Ok(pubdate_iso8861.to_string())
}

fn extract_ids(data: &serde_json::Value) -> Result<HashMap<&str, &str>> {
    let ids = data["articleids"]
        .as_array()
        .ok_or(anyhow!("no articleids found"))?
        .iter()
        .map(|value| {
            (
                value["idtype"].as_str().unwrap(),
                value["value"].as_str().unwrap(),
            )
        })
        .collect();

    Ok(ids)
}

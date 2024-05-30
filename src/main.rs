use clap::{ App, Arg };
use reqwest::Client;
use serde_json::{ json, Value };
use std::fs::File;
use std::io::{ BufReader, BufRead, Write };
use std::time::Duration;
use tokio::time;
use chrono::Local;
use lazy_static::lazy_static;
use toml;

lazy_static! {
    static ref CONFIG: (String, String) = load_config().expect("Failed to load config");
}

fn load_config() -> Result<(String, String), Box<dyn std::error::Error>> {
    const CONF: &str = include_str!("../config.toml");
    let config_toml: toml::Value = CONF.parse()?;
    let api_key = config_toml["settings"]["api_key"].as_str().ok_or("Missing API key")?.to_string();
    let endpoint = config_toml["settings"]["endpoint"]
        .as_str()
        .ok_or("Missing endpoint")?
        .to_string();
    Ok((api_key, endpoint))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("Translation CLI")
        .version("1.0")
        .author("Your Name")
        .about("Translate subtitle files")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("FILE")
                .help("Sets the input file")
                .required(true)
                .takes_value(true)
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Sets the output file")
                .required(true)
                .takes_value(true)
        )
        .get_matches();

    let input_file = matches.value_of("input").unwrap();
    let output_file = matches.value_of("output").unwrap();
    translate_file(input_file, output_file).await?;
    Ok(())
}

async fn translate_text(message: &str) -> Result<(String, i64), Box<dyn std::error::Error>> {
    let api_key = &CONFIG.0;
    let endpoint = &CONFIG.1;
    let client = Client::new();

    let request_body =
        json!({
        "model": "gpt-3.5-turbo",
        "messages": [
            {
                "role": "system",
                "content": "Translate the following French text into Chinese, preserving the original meaning and applying refined language as needed"
            },
            {
                "role": "user",
                "content": message
            }
        ]
    });

    println!(
        "{} ORIGIN -- {}",
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        message.trim()
    );

    let response = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(serde_json::to_vec(&request_body)?)
        .send().await?
        .json::<Value>().await?;

    let completion = response["choices"][0]["message"]["content"].as_str().unwrap_or("");
    println!(
        "{} TRANSLATED -- {}",
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        completion
    );

    let cost = response["usage"]["total_tokens"].as_i64().unwrap_or(0);
    println!("COSTS: {} Tokens\n", cost);

    Ok((completion.to_string(), cost))
}

async fn translate_file(
    input_file: &str,
    output_file: &str
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Start translating file: {}\n", input_file);

    let file = File::open(input_file)?;
    let reader = BufReader::new(file);

    let mut translated_entries = String::new();
    let mut current_entry = String::new();
    let mut entry_number = String::new();
    let mut time_code = String::new();
    let mut total_cost = 0;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            if !current_entry.is_empty() {
                let result = translate_text(&current_entry).await?;
                let (translation, cost) = result;
                total_cost += cost;
                translated_entries.push_str(&entry_number);
                translated_entries.push('\n');
                translated_entries.push_str(&time_code);
                translated_entries.push('\n');
                translated_entries.push_str(&format!("{}\n", current_entry.trim()));
                translated_entries.push_str(&format!("{}\n", translation.trim()));
                translated_entries.push('\n');
                current_entry.clear();
                time::sleep(Duration::from_secs(2)).await;
            }
        } else if line.parse::<u32>().is_ok() {
            entry_number = line.to_string();
        } else if line.contains("-->") {
            time_code = line.to_string();
        } else {
            current_entry.push_str(&line);
            current_entry.push('\n');
        }
    }

    if !current_entry.is_empty() {
        let result = translate_text(&current_entry).await?;
        let (translation, cost) = result;
        total_cost += cost;
        translated_entries.push_str(&entry_number);
        translated_entries.push('\n');
        translated_entries.push_str(&time_code);
        translated_entries.push('\n');
        translated_entries.push_str(&format!("{}\n", current_entry.trim()));
        translated_entries.push_str(&format!("{}", translation.trim()));
    }

    println!("Total token costs: {}", total_cost);
    let mut output = File::create(output_file)?;
    write!(output, "{}", translated_entries)?;
    println!("Translation completed. Output saved to: {}", output_file);
    Ok(())
}

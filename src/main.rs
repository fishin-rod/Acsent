use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use image;
use dotenv::dotenv;

use std::collections::HashMap;
use std::time::Duration;
use std::thread::sleep;
use std::env;

#[derive(Debug, Deserialize, Serialize)]
struct Message {
    _id: String,
    nonce: String,
    channel: String,
    author: String,
    content: String,
}

#[tokio::main]
pub async fn fetch(bot: &str) -> Result<(), Box<dyn std::error::Error>> {
    
    let mut header = HeaderMap::new();
    header.insert("x-bot-token", HeaderValue::from_str(bot)?);
    
    let url = "https://api.revolt.chat/channels/01GVVE0TZH01VB428E2NMF70J6/messages"; 

    loop {  
        let client = Client::new().get(url).headers(header.clone()).send().await?.text().await?;
        let message: Vec<Message> = serde_json::from_str(&client)?;
        
        let contents = message.iter().map(|x| x.content.clone()).collect::<Vec<String>>();
        
        if contents[0].to_lowercase().starts_with("!eval") {
            equation(contents[0].split("!eval").nth(1).unwrap_or(""), bot).await?;
        }
    
        sleep(Duration::from_secs(5));

        if contents[0].to_lowercase().starts_with("!quit") {
            break;
        }
    }
    Ok(())
}

pub async fn equation(equation: &str, bot: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut url = "https://latex.codecogs.com/png.latex?".to_string();
    url.push_str(&equation);

    send(&url.replace(" ", ""), bot).await?;
    
    let client = Client::new().get(url).send().await?.bytes().await?;
    
    let image = image::load_from_memory(&client)?;
    image.save("latex.png")?;

    Ok(())
}

pub async fn send(iurl: &str, bot: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut header = HeaderMap::new();
    header.insert("x-bot-token", HeaderValue::from_str(bot)?);
    
    let url = "https://api.revolt.chat/channels/01GVVE0TZH01VB428E2NMF70J6/messages"; 

    let mut map = HashMap::new();
    map.insert("content", format!("[]({})", iurl));

    let _client = Client::new().post(url).headers(header.clone()).json(&map).send().await?;
    //let body = client.text().await?;

    Ok(())
}

fn main() {
    dotenv().ok();
    let seceret = env::var("BOT_KEY").unwrap();
    
    fetch(&seceret).expect("A");
}

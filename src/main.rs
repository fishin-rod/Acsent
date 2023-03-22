use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use image;
use dotenv::dotenv;

use std::collections::HashMap;
use std::sync::mpsc::channel;
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

#[derive(Debug, Serialize)]
struct PlaygroundCode {
    channel: Channel,
    edition: Edition,
    code: String,
    #[serde(rename = "crateType")]
    crate_type: CrateType,
    mode: Mode,
    tests: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Channel {
    Stable,
    Beta,
    Nightly,
}

#[derive(Debug, Serialize)]
enum Edition {
    #[serde(rename = "2015")]
    E2015,
    #[serde(rename = "2018")]
    E2018,
    #[serde(rename = "2021")]
    E2021,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
enum CrateType {
    #[serde(rename = "bin")]
    Binary,
    #[serde(rename = "lib")]
    Library,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Debug,
    Release,
}

#[derive(Debug, Deserialize, Serialize)]
struct Code {
    success: bool,
    stdout: String,
    stderr: String,
}

#[tokio::main]
pub async fn fetch(bot: &str) -> Result<(), Box<dyn std::error::Error>> {
    
    send("Thank you for choosing to use bird bot! The bot will start in a couple of seconds.", bot, 1).await?;

    let mut header = HeaderMap::new();
    header.insert("x-bot-token", HeaderValue::from_str(bot)?);
    
    let url = "https://api.revolt.chat/channels/01GVVE0TZH01VB428E2NMF70J6/messages"; 

    loop {
    //for _x in 0..1 {  
        let client = Client::new().get(url).headers(header.clone()).send().await?.text().await?;
        let message: Vec<Message> = serde_json::from_str(&client)?;
        
        let contents = message.iter().map(|x| x.content.clone()).collect::<Vec<String>>();
        
        if contents[0].to_lowercase().starts_with("!eval") {
            equation(contents[0].split("!eval").nth(1).unwrap_or(""), bot).await?;
        }
        
        if contents[0].to_lowercase().starts_with("!comp"){
            compile(contents[0].split("!comp").nth(1).unwrap_or(""), bot).await?;
        }

        if contents[0].to_lowercase().starts_with("!help"){
            send("```# Commands: \n !eval: Evaluates a latex equation and responds with a picture of it. 
            \n !comp (stable/beta/nightly, release/debug, 2021/2018/2015): Compiles rust code and responds with the output. If the parameters are not supplied they will be defulted to their first state.", bot, 1).await?;
        }

        if contents[0].to_lowercase().starts_with("!quit") {
            send("Thanks for using bird-bot! Hope to see you soon!", bot, 1).await?;
            break;
        }

        sleep(Duration::from_secs(5));
    }
    Ok(())
}

pub async fn compile(code: &str, bot: &str) -> Result<(), Box<dyn std::error::Error>>{
    let mut split = code.splitn(2, "```rust");
    
    let left = split.next().unwrap().trim();
    let types: Vec<String> = left.trim_matches(|c| c == '('|| c == ')').split(',').map(|s| s.trim().to_string()).collect();
    
    let mut channel = Channel::Stable;
    let mut edition = Edition::E2021;
    let mut mode = Mode::Release;

    if left.len() == 3{
        channel = match types[0].to_lowercase().as_str(){
            "stable" => Channel::Stable,
            "beta" => Channel::Beta,
            "nightly" => Channel::Nightly,
            _ => Channel::Stable,
        };
        mode = match types[1].to_lowercase().as_str(){
            "debug" => Mode::Debug,
            "release" => Mode::Release,
            _ => Mode::Debug,
        };
        edition = match types[2].as_str(){
            "2021" => Edition::E2021,
            "2018" => Edition::E2018,
            "2015" => Edition::E2015,
            _ => Edition::E2021,
        };
    }

    let mut right = split.next().unwrap().trim();
    
    if right.contains("```"){
        right = right.trim_matches('`');
    }       
   
    let url = "https://play.rust-lang.org/execute";

    let play = PlaygroundCode {
        channel: channel,
        edition: edition,
        code: right.to_string(),
        crate_type: CrateType::Binary,
        mode: mode,
        tests: false,
    };

    let client = Client::new().post(url).json(&play).send().await?.text().await?;

    let response: Code = serde_json::from_str(&client)?;

    if response.success{
        send(&response.stdout, bot, 2).await?;
    }
    else{
        send(&response.stderr, bot, 2).await?;
    }

    Ok(())
}

pub async fn equation(equation: &str, bot: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut url = "https://latex.codecogs.com/png.latex?".to_string();
    url.push_str(&equation);

    send(&url.replace(" ", ""), bot, 3).await?;
    
    let client = Client::new().get(url).send().await?.bytes().await?;
    
    let image = image::load_from_memory(&client)?;
    image.save("latex.png")?;

    Ok(())
}

pub async fn send(text: &str, bot: &str, form: i32) -> Result<(), Box<dyn std::error::Error>> {
    let mut header = HeaderMap::new();
    header.insert("x-bot-token", HeaderValue::from_str(bot)?);

    let mut map = HashMap::new();

    if form == 3 {
        map.insert("content", format!("[]({})", text));
    }
    else if form == 2 {
        map.insert("content", format!("```{}```", text));
    }
    else if form == 1 {
        map.insert("content", format!("{}", text));
    }

    let url = "https://api.revolt.chat/channels/01GVVE0TZH01VB428E2NMF70J6/messages"; 

    let _client = Client::new().post(url).headers(header.clone()).json(&map).send().await?;
    //let body = client.text().await?;

    Ok(())
}

fn main(){
    dotenv().ok();
    let seceret = env::var("BOT_KEY").unwrap();

    fetch(&seceret).expect("A");
}

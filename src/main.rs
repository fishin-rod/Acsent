use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use tungstenite::{connect, Message as Messages};
use url::Url;
use serde::{Deserialize, Serialize};
use image;
use dotenv::dotenv;
use lazy_static::lazy_static;

use std::collections::HashMap;
use std::time::Duration;
use std::thread;
use std::sync::{Arc, Mutex};
use std::env::var;

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

pub async fn fetch(channel: &str, msg: &str) -> Result<(), Box<dyn std::error::Error>> {

    let mut header = HeaderMap::new();
    header.insert("x-bot-token", HeaderValue::from_str(&BOT)?);
    
    let url = format!("https://api.revolt.chat/channels/{}/messages", channel); 

    let client = Client::new().get(url).headers(header.clone()).send().await?.text().await?;
    let message: Vec<Message> = serde_json::from_str(&client)?;
        
    let contents = message.iter().map(|x| x.content.clone()).collect::<Vec<String>>();
        
    if contents[0].to_lowercase().starts_with("!eval") {
        equation(contents[0].split("!eval").nth(1).unwrap_or(""), channel, msg).await?;
    }
        
    if contents[0].to_lowercase().starts_with("!comp"){
        compile(contents[0].split("!comp").nth(1).unwrap_or(""), channel, msg).await?;
    }

    // Godbolt support

    if contents[0].to_lowercase().starts_with("!help"){
        send("```# Commands: \n !eval: Evaluates a latex equation and responds with a picture of it. 
        \n !comp (stable/beta/nightly, release/debug, 2021/2018/2015): Compiles rust code and responds with the output. If the parameters are not supplied they will be defulted to their first state.".to_string(), channel, msg).await?;
    }
 
    Ok(())
}

pub async fn compile(code: &str, chan: &str, msg: &str) -> Result<(), Box<dyn std::error::Error>>{
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
        send(format!("``` \n {}", &response.stdout), chan, msg).await?;
    }
    else{
        send(format!("``` \n {}", &response.stderr), chan, msg).await?;
    }

    Ok(())
}

pub async fn equation(equation: &str, channel: &str, msg: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut url = "https://latex.codecogs.com/png.latex?".to_string();
    url.push_str(&equation);

    send(format!("[]({})", url.replace(" ", "")), channel, msg).await?;
    
    let client = Client::new().get(url).send().await?.bytes().await?;
    
    let image = image::load_from_memory(&client)?;
    image.save("latex.png")?;

    Ok(())
}

pub async fn send(text: String, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut header = HeaderMap::new();
    header.insert("x-bot-token", HeaderValue::from_str(&BOT)?);

    let mut map = HashMap::new();

    map.insert("content", format!("{}", text));

    // Im just going to leave this here incase I ever want to do it ...
   // map.insert("replies", format!("[{}, true]", message));

    let url = format!("https://api.revolt.chat/channels/{}/messages", channel); 

    let _client = Client::new().post(url).headers(header.clone()).json(&map).send().await?;
    //let body = client.text().await?;

    Ok(())
}

#[tokio::main]
async fn socket() -> Result<(), Box<dyn std::error::Error>> {
    let url = Url::parse("wss://ws.revolt.chat?version=1&format=json").unwrap();
    let (mut socket, _) = connect(url).expect("Failed to connect");
    
    // Send auth request
    let auth_request = format!("{{\"type\": \"Authenticate\", \"token\":\"{}\"}}", *BOT);
    socket.write_message(Messages::Text(auth_request.into())).unwrap();

    let paused = Arc::new(Mutex::new(false));

    let message = r#"{"type": "Ping", "data": 0}"#;
    socket.write_message(Messages::Text(message.into())).unwrap();

    loop {
        let msg = socket.read_message().unwrap();
        
        if let Messages::Text(msg_str) = &msg {
            let parsed_msg: serde_json::Value = serde_json::from_str(msg_str).unwrap();
            
            if parsed_msg["type"] == "Pong" {
                let paused_clone = paused.clone();
                let sleeping_thread = thread::spawn(move || {
                    *paused_clone.lock().unwrap() = true;
                    thread::sleep(Duration::from_secs(15));
                    *paused_clone.lock().unwrap() = false;
                });
                while *paused.lock().unwrap() {
                    
                }
                sleeping_thread.join().unwrap();
            }
            else if parsed_msg["type"] == "Message" {
                let msg = parsed_msg["_id"].as_str().unwrap();
                let channel = parsed_msg["channel"].as_str().unwrap();
                println!("{:?}", msg);
                fetch(channel, msg).await?;
            }
        }
        println!("{:?}", msg);
    }
}

fn main(){
    dotenv().ok();

    socket().expect("A");
}

lazy_static! {
    static ref BOT: String = var("BOT_KEY").unwrap();
}

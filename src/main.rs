use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use reqwest::multipart::{Form, Part};
use tungstenite::{connect, Message as Messages};
use url::Url;
use serde::{Deserialize, Serialize};
use image;
use plotters::prelude::*;
use dotenv::dotenv;
use lazy_static::lazy_static;

use std::collections::{HashMap};
use std::fs::File;
use std::io::Read;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::thread;
use std::env::var;

#[derive(Debug, Deserialize, Serialize)]
struct Message {
    id: String,
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

#[derive(Debug, Deserialize, Serialize)]
struct Image{
    id: String,
}

async fn fetch(contents: &str, channel: &str) -> Result<(), Box<dyn std::error::Error>> {
        
    if contents.to_lowercase().starts_with("!eval") {
        equation(contents.split("!eval").nth(1).unwrap_or(""), channel).await?;
    }
        
    if contents.to_lowercase().starts_with("!comp"){
        compile(contents.split("!comp").nth(1).unwrap_or(""), channel).await?;
    }

    if contents.to_lowercase().starts_with("!graph"){
       graph(contents.split("!graph").nth(1).unwrap_or(""), channel).await?;
    }

    // Godbolt support

    if contents.to_lowercase().starts_with("!help"){
        send("```# Commands: \n !help: Displays this message.
        \n !graph (title) [(x,y), (x,y)]: Displays a scatter plot of the points provided. 
        \n *Note: If you want to use the graph command make sure when suppling coordinates to not leave a space between the x,y but to leave a space between the sets (x,y), (x,y). This is working on being resolved but for now it will do.
        \n !eval (black/white/blue/red): Evaluates a latex equation and responds with a picture of it. If parameters are not supplied it will defult to transparent. 
        \n *Note: Some colors have cropping issues on eqautions. This is a problem with the API and not me!
        \n !comp (stable/beta/nightly, release/debug, 2021/2018/2015): Compiles rust code and responds with the output. If the parameters are not supplied they will be defulted to their first state.".to_string(), channel,false).await?;
    }
 
    Ok(())
}

pub async fn compile(code: &str, chan: &str) -> Result<(), Box<dyn std::error::Error>>{
    if !code.contains("```"){
        send("Error Missing code block".to_string(), chan, false).await?;
        return Ok(());
    }

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
        send(format!("``` \n {}", &response.stdout), chan, false).await?;
    }
    else{
        send(format!("``` \n {}", &response.stderr), chan, false).await?;
    }

    Ok(())
}

async fn equation(equation: &str, channel: &str) -> Result<(), Box<dyn std::error::Error>> {
    let split: Vec<&str> = equation.splitn(2, "eval").collect();
    let mut components_vec: Vec<Vec<&str>> = vec![];
    
    for s in split.iter(){
        let s = s.trim_start_matches('[').trim_end_matches(']');
        let components: Vec<&str> = s.split_whitespace().collect();
        components_vec.push(components);
    }
    
    let mut left = "";
    #[allow(unused_assignments)]
    let mut right = "";

    if components_vec[0].len() == 2{
        left = components_vec[0][0];
        right = components_vec[0][1];
    }
    else {
        right = components_vec[0][0];
    }

    let mut url = r#"https://latex.codecogs.com/png.latex?"#.to_string();

    if !left.is_empty() {
        left = left.trim_start_matches('(').trim_end_matches(')');
        let choise = match left.to_lowercase().as_str(){
            "black" => r#"\bg_black"#,
            "white" => r#"\bg_white"#,
            "blue" => r#"\bg_blue"#,
            "red" => r#"\bg_red"#,
            _ => "",
        };
        url.push_str(choise);
    }
    
    url.push_str(right);

    send(format!("[]({})", url.replace(' ', "")), channel, false).await?;
    
    let client = Client::new().get(url).send().await?.bytes().await?;
    
    let image = image::load_from_memory(&client)?;
    image.save(r#"C:\Users\conno\Downloads\Birdy Bot\images\latex.png"#)?;

    Ok(())
}

//graph

async fn graph(data: &str, channel: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(r#"C:\Users\conno\Downloads\Birdy Bot\images\scatter_plot.png"#, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let title = data.splitn(2, ')').nth(0).unwrap_or("").trim_matches('(').to_string();
    let points = data.splitn(2, ')').nth(1).unwrap_or("");
    
    println!("{}", title.trim_start().trim_matches('('));
 
    let nums: Vec<(i32, i32)> = points.split(", ")
    .map(|s| s.trim_matches(|c| c == '[' || c == ']' || c == '(' || c == ')'))
    .map(|s| s.trim_end_matches(")]").trim_start_matches(" [("))
    .map(|s| {
        let mut v: Vec<&str> = s.split(',').collect();
        if v.len() == 1{
            v.push("0");
        }
        let x = v[0].parse().unwrap_or(0);
        let y = v[1].parse().unwrap_or(0);
        (x, y)
    }).collect();

    println!("{:?}", nums);

    let (max_x, max_y) = nums.iter().fold((0, 0), |acc, &(x, y)| {
        (acc.0.max(x), acc.1.max(y))
    });

    let mut scatter = ChartBuilder::on(&root)
        .caption(title, ("sans-serif", 20))
        .x_label_area_size(40).y_label_area_size(40)
        .margin(5)
        .build_cartesian_2d(0..max_x+2, 0..max_y+2)?;

        scatter
        .draw_series(
            nums.iter().map(|(x, y)| Circle::new((*x, *y), 5, BLACK.filled())),
        )
        .expect("Unable to draw scatter plot");

        scatter
        .configure_mesh()
        .x_desc("X Axis").y_desc("Y Axis")
        .x_labels(10).y_labels(10)
        .draw()?;

    root.present().expect("Something went very wrong for this to happen");

    send("Graph: ".to_string(), channel, true).await?;

    Ok(())
}

async fn send(text: String, channel: &str, attach: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut header = HeaderMap::new();
    header.insert("x-bot-token", HeaderValue::from_str(&BOT)?);

    if attach{
        let mut file = File::open(r#"C:\Users\conno\Downloads\Birdy Bot\images\scatter_plot.png"#)?;
        let mut image_data = Vec::new();
        file.read_to_end(&mut image_data)?;

        let form = Form::new()
        .part("image", Part::bytes(image_data).file_name("scatter_plot.png"));

        let url= "https://autumn.revolt.chat/attachments";
        let client = Client::new().post(url).headers(header.clone()).multipart(form).send().await?;
        let msg_str = client.text().await?;

        let parsed_msg: Image = serde_json::from_str(&msg_str).unwrap();
        let id = parsed_msg.id;
        
        let mut map = HashMap::new();
        map.insert("attachments", vec![id]);

        let url = format!("https://api.revolt.chat/channels/{}/messages", channel); 
        let client = Client::new().post(url).headers(header).json(&map).send().await?;
        let body = client.text().await?;

        println!("{}", body);

    }
    else {
        let mut map = HashMap::new();
        map.insert("content", format!("{}", text));

        let url = format!("https://api.revolt.chat/channels/{}/messages", channel); 
        let client = Client::new().post(url).headers(header).json(&map).send().await?;
        let body = client.text().await?;

        println!("{}", body);

    }

    Ok(())
}

#[allow(dead_code)]
async fn del(channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> { 
    let mut header = HeaderMap::new();
    header.insert("x-bot-token", HeaderValue::from_str(&BOT)?);

    let url = format!("https://api.revolt.chat/channels/{}/messages/{}", channel, message); 

    let client = Client::new().delete(url).headers(header).send().await?;
    let body = client.text().await?;
    println!("{}", body);

    Ok(())
}

#[tokio::main]
async fn socket() -> Result<(), Box<dyn std::error::Error>> {
    loop{
    let url = Url::parse("wss://ws.revolt.chat?version=1&format=json").unwrap();
    let (mut socket, _) = connect(url).expect("Failed to connect");
    
    let auth_request = format!("{{\"type\": \"Authenticate\", \"token\":\"{}\"}}", *BOT);
    socket.write_message(Messages::Text(auth_request)).unwrap();

    let paused = Arc::new(Mutex::new(false));

    let message = r#"{"type": "Ping", "data": 0}"#;
    socket.write_message(Messages::Text(message.into())).unwrap();

    'inner: loop {
        let msg = match socket.read_message(){
            Ok(msg) => msg,
            Err(e) => {
                println!("Error: {}", e);
                break 'inner;
            },
        };
        
        if let Messages::Text(msg_str) = &msg {
            let parsed_msg: serde_json::Value = serde_json::from_str(msg_str).unwrap();

            if parsed_msg["type"] == "Pong"{
                let paused_clone = paused.clone();
                let sleeping_thread = thread::spawn(move || {
                    *paused_clone.lock().unwrap() = true;
                    thread::sleep(Duration::from_secs(15));
                    *paused_clone.lock().unwrap() = false;
                });
                while *paused.lock().unwrap() {}
                sleeping_thread.join().unwrap();
            }
            else if parsed_msg["type"] == "Message" {
                //let msg = parsed_msg["_id"].as_str().unwrap();
                let channel = parsed_msg["channel"].as_str().unwrap();
                let content = parsed_msg["content"].as_str().unwrap_or("");
                fetch(content, channel).await?;
            }
        }
        println!("{:?}", msg);
    }}
}

fn main(){
    dotenv().ok();
    
    socket().expect("A");
}

lazy_static! {
    static ref BOT: String = var("BOT_KEY").unwrap();
}

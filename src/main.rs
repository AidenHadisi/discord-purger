use std::env;

use dotenv::dotenv;
use futures::{stream, StreamExt};
use reqwest::{header, Client, StatusCode};
use serde::Deserialize;
use tokio::time;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let user_id = env::var("USER_ID").expect("Expected a user ID in the environment");

    // Prompt user for channel id
    println!("Please enter the channel ID:");
    let mut channel_id = String::new();
    std::io::stdin()
        .read_line(&mut channel_id)
        .expect("Failed to read line");

    delete_messages(&token, &user_id, &channel_id.trim())
        .await
        .unwrap();
}

///Represents a discord message.
#[derive(Deserialize)]
struct Message {
    id: String,
    author: Author,
}

///Represents a discord message author.
#[derive(Deserialize)]
struct Author {
    id: String,
}

// Deletes user messages from given discord channel.
async fn delete_messages(
    token: &str,
    user_id: &str,
    channel_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = &Client::new();
    let auth_header = format!("Bot {}", token);

    let mut last_message_id: Option<String> = None;
    loop {
        let url = format!(
            "https://discord.com/api/v9/channels/{}/messages",
            channel_id
        );

        let response = client
            .get(&url)
            .header(header::AUTHORIZATION, &auth_header)
            .query(&[("limit", "100")])
            .query(&[("before", last_message_id.as_deref().unwrap_or_default())])
            .send()
            .await?;

        let messages: Vec<Message> = match response.status() {
            StatusCode::OK => response.json().await?,
            _ => {
                let status = response.status();
                let text = response.text().await?;
                return Err(format!("HTTP error {}: {}", status, text).into());
            }
        };
        last_message_id = messages.last().map(|message| message.id.clone());

        let messages_to_delete: Vec<_> = messages
            .into_iter()
            .filter(|message| message.author.id == user_id)
            .collect();

        if messages_to_delete.is_empty() {
            break;
        }

        stream::iter(messages_to_delete)
            .for_each_concurrent(10, |message| async move {
                let url = format!(
                    "https://discord.com/api/v9/channels/{}/messages/{}",
                    channel_id, message.id
                );
                let res = client
                    .delete(&url)
                    .header("Authorization", format!("Bot {}", token))
                    .send()
                    .await;

                match res {
                    Ok(_) => println!("Message {} deleted.", message.id),
                    Err(e) => eprintln!("An error occurred: {}", e),
                }
            })
            .await;

        time::sleep(time::Duration::from_secs(5)).await;
    }

    Ok(())
}

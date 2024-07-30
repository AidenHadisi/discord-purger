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

    let mut last_message_id: Option<String> = None;
    loop {
        let url = format!(
            "https://discord.com/api/v9/channels/{}/messages",
            channel_id
        );

        let mut request = client
            .get(&url)
            .header(header::AUTHORIZATION, token)
            .query(&[("limit", "100")]);

        if let Some(ref id) = last_message_id {
            request = request.query(&[("before", id)]);
        }

        let response = request.send().await?;

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
            .for_each_concurrent(2, |message| async move {
                let url = format!(
                    "https://discord.com/api/v9/channels/{}/messages/{}",
                    channel_id, message.id
                );

                match client
                    .delete(&url)
                    .header(header::AUTHORIZATION, token)
                    .send()
                    .await
                {
                    Ok(e) if e.status() == StatusCode::NO_CONTENT => {
                        println!("Deleted message {}", message.id);
                    }
                    Ok(e) => {
                        println!(
                            "Failed to delete message {}: {}",
                            message.id,
                            e.text().await.unwrap()
                        );
                    }
                    Err(err) => {
                        eprintln!("Failed to delete message: {}", err);
                    }
                }

                time::sleep(time::Duration::from_secs(1)).await;
            })
            .await;

        time::sleep(time::Duration::from_secs(5)).await;
    }

    Ok(())
}

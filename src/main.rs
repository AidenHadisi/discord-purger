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

    while let Ok(ref messages) =
        fetch_messages(client, token, channel_id, last_message_id.clone()).await
    {
        if messages.is_empty() {
            break;
        }

        last_message_id = messages.last().map(|message| message.id.clone());
        let messages_to_delete: Vec<_> = messages
            .into_iter()
            .filter(|message| message.author.id == user_id)
            .collect();

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

                time::sleep(time::Duration::from_secs(2)).await;
            })
            .await;

        time::sleep(time::Duration::from_secs(1)).await;
    }

    Ok(())
}

async fn fetch_messages(
    client: &Client,
    token: &str,
    channel_id: &str,
    last_message_id: Option<String>,
) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
    let url = format!("https://discord.com/api/v9/channels/{channel_id}/messages");

    let response = client
        .get(&url)
        .header(header::AUTHORIZATION, token)
        .query(
            &[("limit", "100")]
                .into_iter()
                .chain(last_message_id.as_ref().map(|id| ("before", id.as_str())))
                .collect::<Vec<_>>(),
        )
        // .query(&last_message_id.map(|id| [("before", id)]))
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => response.json().await.map_err(Into::into),
        status => Err(format!("HTTP error {}: {}", status, response.text().await?).into()),
    }
}

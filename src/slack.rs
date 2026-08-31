use anyhow::{Context, Result, anyhow};
use reqwest::Url;
use std::time::Duration;

pub(crate) struct SlackApp {
    webhook: Url,
    app_info: AppDetail,
}

pub(crate) struct AppDetail {
    pub(crate) message: String,
    pub(crate) description: String,
    pub(crate) version: String,
    pub(crate) image_url: Option<String>,
}

fn readable_image_id(version: &str) -> &str {
    match version.split(':').next_back() {
        Some(last) => last,
        None => version,
    }
}

impl SlackApp {
    pub(crate) fn new(
        webhook: Url,
        message: String,
        description: String,
        version: String,
        image_url: Option<String>,
    ) -> SlackApp {
        SlackApp {
            webhook,
            app_info: AppDetail {
                message,
                description,
                version,
                image_url,
            },
        }
    }

    fn compute_description(&self) -> String {
        let version = readable_image_id(&self.app_info.version);
        // Handle newline so that Slack renders it properly
        let message = self.app_info.message.replace("\\n", "\n");
        format!(
            "{} \n *Application*: {} \n *Version*: {}",
            message, self.app_info.description, version
        )
    }

    pub(crate) fn send_notification(
        &self,
        message: &anyhow::Error,
        latest_output: &str,
    ) -> Result<()> {
        let description = self.compute_description();
        let mut value = serde_json::json!(
        {
            "text": "Health check alert",
            "blocks": [
                {
                    "type": "header",
                    "text": {
                        "type": "plain_text",
                        "text": message.to_string(),
                    }
                },
                {
                    "type": "section",
                    "block_id": "section567",
                    "text": {
                        "type": "mrkdwn",
                        "text": description
                    },
                },
                {
                    "type": "divider"
                }
            ],
            "attachments": [
                {
                    "mrkdwn_in": ["text"],
                    "author_name": "Logs",
                    "text": truncate_for_slack(latest_output)
                }
            ]
        });
        if let Some(image_url) = &self.app_info.image_url {
            let object = value
                .as_object_mut()
                .context("JSON value should be an object")?;
            let blocks = object["blocks"]
                .as_array_mut()
                .context("Blocks field should be an array")?;
            let section = blocks[1]
                .as_object_mut()
                .context("Second block should be a section object")?;
            section.insert(
                "accessory".to_owned(),
                serde_json::json!(
                    {
                        "type": "image",
                        "image_url": image_url,
                        "alt_text": "Health check image".to_owned()
                    }
                ),
            );
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let response = client.post(self.webhook.clone()).json(&value).send()?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Slack notification POST request failed with code {}",
                response.status()
            ))
        }
    }
}

fn truncate_for_slack(output: &str) -> String {
    const MAX_CHARS: usize = 2_800;
    let mut chars = output.chars();
    let truncated: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{truncated}\n… output truncated")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use crate::slack::{readable_image_id, truncate_for_slack};

    #[test]
    fn guess_readable_image_id_works() {
        let image_id =
            readable_image_id("ghcr.io/fpco/some-app:d5def5afc6030dda860a79f231b295e2e412bc28");
        assert_eq!(image_id, "d5def5afc6030dda860a79f231b295e2e412bc28");

        let image_id = readable_image_id("d5def5afc6030dda860a79f231b295e2e412bc28");
        assert_eq!(image_id, "d5def5afc6030dda860a79f231b295e2e412bc28");
    }

    #[test]
    fn slack_output_is_bounded_on_character_boundaries() {
        let output = "🦀".repeat(3_000);
        let truncated = truncate_for_slack(&output);
        assert!(truncated.ends_with("… output truncated"));
        assert!(truncated.chars().count() < 3_000);
    }
}

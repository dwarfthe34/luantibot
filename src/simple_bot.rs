use luanti_bot::{Bot, BotConfig, Event};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init(); // RUST_LOG=info or debug

    let mut bot = Bot::connect(BotConfig::new("84.247.132.141:40001", "dwarfbot", "p")).await?;
    info!("Connected and authenticated");

    while let Some(event) = bot.next_event().await {
        match event {
            Event::Joined => {
                info!("Joined the server");
                bot.send_chat("test test test test test").await?;
            }

            Event::Chat { sender, text } => {
                info!("<{sender}> {text}");

                match text.trim() {
                    "!pos" => {
                        let p = bot.state.pos;
                        bot.send_chat(format!("I am at ({:.1}, {:.1}, {:.1})", p.x, p.y, p.z))
                            .await?;
                    }
                    "!hp" => {
                        bot.send_chat(format!("HP: {}", bot.state.hp)).await?;
                    }
                    "!quit" => {
                        bot.send_chat("Goodbye!").await?;
                        bot.disconnect().await?;
                        break;
                    }
                    _ => {}
                }
            }

            Event::MovePlayer { pos, .. } => {
                info!("Server moved us to ({:.1}, {:.1}, {:.1})", pos.x, pos.y, pos.z);
            }

            Event::Hp { hp } => {
                info!("HP updated: {hp}");
                if hp == 0 {
                    // Auto-respawn on death
                    // bot.respawn().await?;
                }
            }

            Event::Kicked(reason) => {
                info!("Kicked: {reason}");
                break;
            }

            Event::Disconnected => {
                info!("Disconnected");
                break;
            }

            _ => {}
        }
    }

    Ok(())
}

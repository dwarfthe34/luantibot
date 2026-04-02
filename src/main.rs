#![feature(iterator_try_collect)]

use luanti_bot::{Bot, Config, Event};
use std::time::Duration;
use tokio::time::interval;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "luanti_bot=info".into()),
        )
        .init();

    let cfg = Config {
        address:  std::env::args().nth(1).unwrap_or_else(|| "84.247.132.141:40001".into()),
        username: std::env::args().nth(2).unwrap_or_else(|| "dwarfbot".into()),
        password: std::env::args().nth(3).unwrap_or_else(|| "p".into()),
        lang:     "en".into(),
    };

    info!("Connecting to {} as {}", cfg.address, cfg.username);
    let mut bot = Bot::connect(cfg).await?;
    info!("Connected — waiting for events");

    let mut pos_tick = interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            _ = pos_tick.tick() => {
                if bot.state.joined {
                    let pos = bot.state.pos;
                    let yaw = bot.state.yaw;
                    let _ = bot.send_pos_simple(pos, yaw).await;
                }
            }

            event = bot.next_event() => {
                match event {
                    None => {
                        info!("Event channel closed");
                        break;
                    }

                    Some(Event::Joined) => {
                        info!("Joined the server!");
                        bot.send_chat("Hello! I am a headless Luanti bot.").await?;
                    }

                    Some(Event::Chat { sender, text }) => {
                        info!("<{sender}> {text}");
                        handle_chat(&mut bot, &sender, &text).await?;
                    }

                    Some(Event::MovePlayer { pos, .. }) => {
                        info!("Server moved us to ({:.1}, {:.1}, {:.1})", pos.x, pos.y, pos.z);
                    }

                    Some(Event::Hp { hp }) => {
                        info!("HP: {hp}");
                        if hp == 0 {
                            info!("Died — respawning");
                            bot.respawn().await?;
                        }
                    }

                    Some(Event::PlayerList { update_type, players }) => {
                        use mt_net::PlayerListUpdateType;
                        match update_type {
                            PlayerListUpdateType::Add    => for p in &players { info!("+ {p} joined"); },
                            PlayerListUpdateType::Remove => for p in &players { info!("- {p} left");   },
                            PlayerListUpdateType::Init   => info!("Players online: {players:?}"),
                        }
                    }

                    Some(Event::TimeOfDay { time, .. }) => {
                        info!("Time of day: {time}/24000");
                    }

                    Some(Event::Kicked(reason)) => {
                        info!("Kicked: {reason}");
                        break;
                    }

                    Some(Event::Disconnected) => {
                        info!("Disconnected");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_chat(bot: &mut Bot, sender: &str, text: &str) -> anyhow::Result<()> {
    if sender == bot.username() {
        return Ok(());
    }

    match text.trim() {
        "!pos" => {
            let p = bot.state.pos;
            bot.send_chat(format!("({:.1}, {:.1}, {:.1})", p.x, p.y, p.z)).await?;
        }
        "!hp" => {
            bot.send_chat(format!("HP: {}", bot.state.hp)).await?;
        }
        "!quit" => {
            bot.send_chat("Goodbye!").await?;
            bot.disconnect().await?;
        }
        _ => {}
    }

    Ok(())
}

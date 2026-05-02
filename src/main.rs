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
        address:  std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:30000".into()),
        username: std::env::args().nth(2).unwrap_or_else(|| "bot".into()),
        password: std::env::args().nth(3).unwrap_or_else(|| "password".into()),
        lang:     "en".into(),
    };

    info!("Connecting to {} as {}", cfg.address, cfg.username);
    let mut bot = Bot::connect(cfg).await?;
    info!("Connected — waiting for events");

    let mut phys_tick = interval(Duration::from_millis(50));
    const DT: f32 = 0.05;

    loop {
        tokio::select! {
            _ = phys_tick.tick() => {
                if bot.state.joined {
                    if let Err(e) = bot.physics_step(DT).await {
                        info!("physics_step error: {e}");
                    }
                }
            }

            event = bot.next_event() => {
                match event {
                    None => {
                        info!("Event channel closed");
                        break;
                    }

                    Some(Event::Joined) => {
                        info!("Joined the server");
                    }

                    Some(Event::Chat { sender, text }) => {
                        info!("<{sender}> {text}");
                        handle_chat(&mut bot, &sender, &text).await?;
                    }

                    Some(Event::MovePlayer { pos, .. }) => {
                        info!("Server moved us to ({:.1}, {:.1}, {:.1})", pos.x, pos.y, pos.z);
                    }

                    Some(Event::MovementParams { walk_speed, jump_speed, gravity }) => {
                        info!("Movement params — walk:{walk_speed:.1} jump:{jump_speed:.1} gravity:{gravity:.1}");
                        if !bot.state.respawned {
                            bot.state.respawned = true;
                            info!("Respawning");
                            bot.respawn().await?;
                        }
                    }

                    Some(Event::Hp { hp }) => {
                        info!("HP: {hp}");
                    }

                    Some(Event::BlockData { pos, .. }) => {
                        info!("Received block data at ({}, {}, {})", pos.x, pos.y, pos.z);
                    }

                    Some(Event::Died) => {
                        info!("Died — respawn sent automatically");
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
        "!respawn" => {
            bot.respawn().await?;
        }
        "!jump" => {
            bot.jump();
        }
        "!forward" => {
            bot.walk(true, false, false, false);
            bot.send_chat("Walking forward").await?;
        }
        "!stop" => {
            bot.stop();
            bot.send_chat("Stopped").await?;
        }
        "!quit" => {
            bot.send_chat("Goodbye!").await?;
            bot.disconnect().await?;
        }
        _ => {}
    }

    Ok(())
}